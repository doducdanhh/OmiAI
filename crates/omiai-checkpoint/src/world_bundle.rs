//! `Checkpointable` cho `World`: bundle các file payload của checkpoint-v1.
//!
//! Layout trong thư mục checkpoint:
//! ```text
//! world/grid.bin                   — lưới CA (format ca_grid, xem ca_grid.rs)
//! world/atoms.cbor                 — {step_count, atoms[]}
//! world/registry.cbor              — {genomes[]} theo thứ tự slot
//! world/rng_state.bin              — u64 LE seed + u64 LE stream + u128 LE word_pos
//! world/airwave.cbor               — Vec<Option<Symbol>> theo ô lưới
//! world/vocabulary.cbor            — bảng đồng xuất hiện tích luỹ toàn run
//! communication/conventions.cbor   — ConventionTracker (slice 5, OPTIONAL lúc load)
//! knowledge_graph/graph.cbor       — {concepts[], relations[]} (slice 5, OPTIONAL lúc load)
//! ```
//!
//! Hai payload cuối là **optional lúc load**: checkpoint ghi bởi slice 2/3/4
//! không có chúng và vẫn phải đọc được (tracker rỗng + graph rỗng). Schema cũ
//! là tập con hợp lệ của schema mới ⇒ KHÔNG bump `format_version` (spec slice 5
//! §6, checkpoint-v1 §6).
//!
//! RNG tái tạo: `ChaCha8Rng::seed_from_u64(seed)` → `set_stream(stream)`
//! → `set_word_pos(word_pos)` (ADR-0006).

use std::path::Path;

use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_world::atoms::Atom;
use omiai_world::communication::{ConventionTracker, Vocabulary};
use omiai_world::registry::{FormulaRegistry, Genome};
use omiai_world::world_loop::World;
use rand_chacha::{rand_core::SeedableRng, ChaCha8Rng};
use serde::{Deserialize, Serialize};

use crate::ca_grid::{decode_ca, encode_ca};
use crate::error::CheckpointError;
use crate::fsutil::{hash_file, write_atomic};
use crate::manifest::{FileRecord, Manifest};
use crate::traits::Checkpointable;

const WORLD_DIR: &str = "world";
const GRID_FILE: &str = "grid.bin";
const ATOMS_FILE: &str = "atoms.cbor";
const REGISTRY_FILE: &str = "registry.cbor";
const RNG_FILE: &str = "rng_state.bin";
const AIRWAVE_FILE: &str = "airwave.cbor";
const VOCABULARY_FILE: &str = "vocabulary.cbor";
const COMM_DIR: &str = "communication";
const CONVENTIONS_FILE: &str = "conventions.cbor";
const KNOWLEDGE_DIR: &str = "knowledge_graph";
const GRAPH_FILE: &str = "graph.cbor";

#[derive(Debug, Serialize, Deserialize)]
struct AtomsFile {
    step_count: u64,
    atoms: Vec<Atom>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryFile {
    genomes: Vec<Genome>,
}

/// Dạng file của `KnowledgeGraph`, vốn không có `Serialize`.
///
/// Concept theo thứ tự chèn (`concept_ids()` là `IndexMap`) và relation theo
/// thứ tự cạnh, nên load dựng lại đúng cùng một cấu trúc — điều kiện để
/// round-trip so được bằng `relations()`.
#[derive(Debug, Serialize, Deserialize, Default)]
struct GraphFile {
    concepts: Vec<Concept>,
    relations: Vec<(String, String, String)>,
}

impl GraphFile {
    fn from_graph(graph: &KnowledgeGraph) -> Self {
        let ids: Vec<String> = graph.concept_ids().map(str::to_string).collect();
        Self {
            concepts: ids
                .iter()
                .map(|id| {
                    graph
                        .get(id)
                        .expect("id vừa lấy từ chính graph")
                        .clone()
                })
                .collect(),
            relations: graph.relations(),
        }
    }

    /// Dựng lại graph, từ chối file hỏng thay vì bỏ qua âm thầm.
    fn into_graph(self, path: &Path) -> Result<KnowledgeGraph, CheckpointError> {
        let mut graph = KnowledgeGraph::new();
        for concept in self.concepts {
            let id = concept.id.clone();
            if !graph.add_concept(concept) {
                return Err(CheckpointError::Corrupt {
                    path: path.to_path_buf(),
                    expected: "concept id duy nhất".to_string(),
                    actual: format!("id `{id}` trùng"),
                });
            }
        }
        for (from, to, kind) in self.relations {
            graph.add_relation(&from, &to, kind.clone()).map_err(|e| {
                CheckpointError::Corrupt {
                    path: path.to_path_buf(),
                    expected: "quan hệ trỏ vào concept có thật".to_string(),
                    actual: format!("{from} --{kind}--> {to}: {e}"),
                }
            })?;
        }
        Ok(graph)
    }
}

fn cbor_error(e: ciborium::ser::Error<std::io::Error>) -> CheckpointError {
    CheckpointError::Cbor(e.to_string())
}

fn de_cbor_error(e: ciborium::de::Error<std::io::Error>) -> CheckpointError {
    CheckpointError::Cbor(e.to_string())
}

/// Serialize CBOR vào buffer — cùng một đường cho mọi payload có cấu trúc.
fn to_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, CheckpointError> {
    let mut buf = std::io::Cursor::new(Vec::new());
    ciborium::ser::into_writer(value, &mut buf).map_err(cbor_error)?;
    Ok(buf.into_inner())
}

/// Đọc file bắt buộc.
fn read_file(path: &Path) -> Result<Vec<u8>, CheckpointError> {
    std::fs::read(path).map_err(|source| CheckpointError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Đọc payload optional: không có file ⇒ `None` (checkpoint cũ), có file mà
/// đọc lỗi ⇒ lỗi thật (đừng biến hỏng đĩa thành "mặc định rỗng").
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, CheckpointError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(CheckpointError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// RNG state: seed + stream + word_pos (ADR-0006) — 32 byte.
fn encode_rng(world: &World) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 8 + 16);
    out.extend_from_slice(&world.rng_seed.to_le_bytes());
    out.extend_from_slice(&world.rng_stream.to_le_bytes());
    out.extend_from_slice(&world.rng.get_word_pos().to_le_bytes());
    out
}

/// Tái tạo generator đúng vị trí trong dãy.
fn restore_rng(seed: u64, stream: u64, word_pos: u128) -> ChaCha8Rng {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    rng.set_stream(stream);
    rng.set_word_pos(word_pos);
    rng
}

impl Checkpointable for World {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let world_dir = dir.join(WORLD_DIR);
        std::fs::create_dir_all(&world_dir).map_err(|source| {
            CheckpointError::Io { path: world_dir.clone(), source }
        })?;

        // 1. grid
        let grid_bytes = encode_ca(&self.ca)?;
        write_atomic(&world_dir, GRID_FILE, &grid_bytes)?;

        // 2. atoms (+ step_count)
        let atoms =
            AtomsFile { step_count: self.step_count, atoms: self.atoms.clone() };
        write_atomic(&world_dir, ATOMS_FILE, &to_cbor(&atoms)?)?;

        // 3. registry (thứ tự slot — bất biến không-remove, xem registry.rs)
        let registry = RegistryFile { genomes: self.registry.genomes_in_order() };
        write_atomic(&world_dir, REGISTRY_FILE, &to_cbor(&registry)?)?;

        // 4. rng
        write_atomic(&world_dir, RNG_FILE, &encode_rng(self))?;

        // 5. airwave
        write_atomic(&world_dir, AIRWAVE_FILE, &to_cbor(&self.airwave)?)?;

        // 6. vocabulary
        write_atomic(&world_dir, VOCABULARY_FILE, &to_cbor(&self.vocabulary)?)?;

        // 7. conventions tracker (slice 5)
        let comm_dir = dir.join(COMM_DIR);
        std::fs::create_dir_all(&comm_dir).map_err(|source| CheckpointError::Io {
            path: comm_dir.clone(),
            source,
        })?;
        write_atomic(&comm_dir, CONVENTIONS_FILE, &to_cbor(&self.conventions)?)?;

        // 8. knowledge graph đã đề bạt (slice 5)
        let kg_dir = dir.join(KNOWLEDGE_DIR);
        std::fs::create_dir_all(&kg_dir).map_err(|source| CheckpointError::Io {
            path: kg_dir.clone(),
            source,
        })?;
        let graph_file = GraphFile::from_graph(&self.knowledge);
        write_atomic(&kg_dir, GRAPH_FILE, &to_cbor(&graph_file)?)?;

        // 9. manifest với hash cả 8 file
        let mut records = Vec::with_capacity(8);
        for (subdir, name) in [
            (WORLD_DIR, GRID_FILE),
            (WORLD_DIR, ATOMS_FILE),
            (WORLD_DIR, REGISTRY_FILE),
            (WORLD_DIR, RNG_FILE),
            (WORLD_DIR, AIRWAVE_FILE),
            (WORLD_DIR, VOCABULARY_FILE),
            (COMM_DIR, CONVENTIONS_FILE),
            (KNOWLEDGE_DIR, GRAPH_FILE),
        ] {
            let blake3 = hash_file(&dir.join(subdir).join(name))?;
            records.push(FileRecord {
                path: format!("{subdir}/{name}"),
                blake3,
            });
        }
        Manifest::write(dir, &records)
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let world_dir = dir.join(WORLD_DIR);

        // Verify manifest + hash trước khi tin bất kỳ file nào.
        let manifest = Manifest::read(dir)?;
        if manifest.format_version != crate::manifest::FORMAT_VERSION_V1 {
            return Err(CheckpointError::Corrupt {
                path: dir.join(crate::manifest::MANIFEST_NAME),
                expected: format!(
                    "format_version {}",
                    crate::manifest::FORMAT_VERSION_V1
                ),
                actual: manifest.format_version.to_string(),
            });
        }
        for record in &manifest.files {
            let path = dir.join(&record.path);
            let actual = hash_file(&path)?;
            if actual != record.blake3 {
                return Err(CheckpointError::Corrupt {
                    path,
                    expected: record.blake3.clone(),
                    actual,
                });
            }
        }

        // grid
        let grid_path = world_dir.join(GRID_FILE);
        let ca = decode_ca(&read_file(&grid_path)?)?;

        // atoms
        let atoms_bytes = read_file(&world_dir.join(ATOMS_FILE))?;
        let atoms_file: AtomsFile = ciborium::de::from_reader(&atoms_bytes[..])
            .map_err(de_cbor_error)?;

        let n_cells = ca.width * ca.height;

        // registry
        let reg_bytes = read_file(&world_dir.join(REGISTRY_FILE))?;
        let registry_file: RegistryFile =
            ciborium::de::from_reader(&reg_bytes[..]).map_err(de_cbor_error)?;

        // rng
        let rng_path = world_dir.join(RNG_FILE);
        let rng_bytes = read_file(&rng_path)?;
        if rng_bytes.len() != 32 {
            return Err(CheckpointError::Corrupt {
                path: rng_path,
                expected: "32-byte rng state".to_string(),
                actual: format!("{} bytes", rng_bytes.len()),
            });
        }
        let seed = u64::from_le_bytes(rng_bytes[0..8].try_into().expect("8 bytes"));
        let stream =
            u64::from_le_bytes(rng_bytes[8..16].try_into().expect("8 bytes"));
        let word_pos =
            u128::from_le_bytes(rng_bytes[16..32].try_into().expect("16 bytes"));

        // airwave
        let airwave_path = world_dir.join(AIRWAVE_FILE);
        let airwave_bytes = read_file(&airwave_path)?;
        let airwave: Vec<Option<u8>> = ciborium::de::from_reader(&airwave_bytes[..])
            .map_err(de_cbor_error)?;

        // vocabulary
        let vocab_bytes = read_file(&world_dir.join(VOCABULARY_FILE))?;
        let vocabulary: Vocabulary = ciborium::de::from_reader(&vocab_bytes[..])
            .map_err(de_cbor_error)?;

        // conventions tracker — OPTIONAL: checkpoint slice 2/3/4 không có.
        let conventions = match read_optional(&dir.join(COMM_DIR).join(CONVENTIONS_FILE))? {
            Some(bytes) => {
                ciborium::de::from_reader(&bytes[..]).map_err(de_cbor_error)?
            }
            None => ConventionTracker::default(),
        };

        // knowledge graph — OPTIONAL, cùng lý do.
        let graph_path = dir.join(KNOWLEDGE_DIR).join(GRAPH_FILE);
        let knowledge = match read_optional(&graph_path)? {
            Some(bytes) => {
                let file: GraphFile = ciborium::de::from_reader(&bytes[..])
                    .map_err(de_cbor_error)?;
                file.into_graph(&graph_path)?
            }
            None => KnowledgeGraph::new(),
        };

        // Nhất quán liên-payload: atom phải nằm trong lưới và gene phải trỏ
        // vào slot có thật. Nếu không kiểm ở đây, world resume "thành công"
        // rồi atom im lặng bất động (`registry.get` → None → `continue` trong
        // agent_act) — đúng kiểu hỏng âm thầm mà §4 spec cấm.
        let n_genomes = registry_file.genomes.len();
        for atom in &atoms_file.atoms {
            if atom.pos.0 >= ca.width || atom.pos.1 >= ca.height {
                return Err(CheckpointError::Corrupt {
                    path: world_dir.join(ATOMS_FILE),
                    expected: format!("pos < ({}, {})", ca.width, ca.height),
                    actual: format!("atom at {:?}", atom.pos),
                });
            }
            if (atom.gene.slot() as usize) >= n_genomes {
                return Err(CheckpointError::Corrupt {
                    path: world_dir.join(ATOMS_FILE),
                    expected: format!("gene slot < {n_genomes}"),
                    actual: format!("slot {}", atom.gene.slot()),
                });
            }
        }

        // Validate airwave length matches grid
        if airwave.len() != n_cells {
            return Err(CheckpointError::Corrupt {
                path: airwave_path,
                expected: format!("airwave length = {n_cells}"),
                actual: format!("{} elements", airwave.len()),
            });
        }

        Ok(World {
            ca,
            registry: FormulaRegistry::from_genomes_in_order(registry_file.genomes),
            atoms: atoms_file.atoms,
            rng: restore_rng(seed, stream, word_pos),
            rng_seed: seed,
            rng_stream: stream,
            step_count: atoms_file.step_count,
            airwave,
            vocabulary,
            conventions,
            knowledge,
        })
    }
}
