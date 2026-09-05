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

use std::collections::BTreeMap;
use std::path::Path;

use omiai_knowledge::graph::{Concept, KnowledgeGraph};
use omiai_world::{
    World, WorldConfig,
    atoms::Atom,
    registry::{FormulaRegistry, Genome, FormulaId},
};
use crate::ca_grid::CellularAutomaton;
use omiai_world::communication::{N_SYMBOLS, N_STATE_CLASSES};
use rand_chacha::{rand_core::SeedableRng, ChaCha8Rng};
use serde::{Deserialize, Serialize};

use crate::ca_grid::{encode_ca, decode_ca};
use crate::error::CheckpointError;
use crate::fsutil::{hash_file, write_atomic, read_file as fs_read_file};
use crate::manifest::{FileRecord, Manifest, FORMAT_VERSION_V1};
use crate::traits::Checkpointable;

// Constants for file names
pub const WORLD_DIR: &str = "world";
pub const GRID_FILE: &str = "grid.bin";
pub const ATOMS_FILE: &str = "atoms.cbor";
pub const REGISTRY_FILE: &str = "registry.cbor";
pub const RNG_FILE: &str = "rng_state.bin";
pub const AIRWAVE_FILE: &str = "airwave.cbor";
pub const VOCABULARY_FILE: &str = "vocabulary.cbor";
pub const COMM_DIR: &str = "communication";
pub const CONVENTIONS_FILE: &str = "conventions.cbor";
pub const KNOWLEDGE_DIR: &str = "knowledge_graph";
pub const GRAPH_FILE: &str = "graph.cbor";

#[derive(Debug, Serialize, Deserialize)]
pub struct AtomsFile {
    step_count: u64,
    atoms: Vec<Atom>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryFile {
    genomes: Vec<Genome>,
}

/// Dạng file của `KnowledgeGraph`, vốn không có `Serialize`.
///
/// Concept theo thứ tự chèn (`concept_ids()` là `IndexMap`) và relation theo
/// thứ tự cạnh, nên load dựng lại đúng cùng một cấu trúc — điều kiện để
/// round-trip so được bằng `relations()`.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GraphFile {
    pub concepts: Vec<Concept>,
    pub relations: Vec<(String, String, String)>,
}

impl GraphFile {
    fn from_graph(graph: &KnowledgeGraph) -> Self {
        Self {
            concepts: graph.concept_ids().filter_map(|id| graph.get(id).cloned()).collect(),
            relations: graph.relations(),
        }
    }

    fn into_graph(self, _path: &Path) -> Result<KnowledgeGraph, CheckpointError> {
        let mut graph = KnowledgeGraph::new();
        // Add concepts first
        for concept in self.concepts {
            graph.add_concept(concept);
        }
        // Then add relations
        for (from, to, kind) in self.relations {
            graph.add_relation(&from, &to, kind).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
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
fn read_required_file(path: &Path) -> Result<Vec<u8>, CheckpointError> {
    std::fs::read(path).map_err(|source| CheckpointError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Đọc payload optional: không có file ⇒ `None` (checkpoint cũ), có file mà
/// đọc lỗi ⇒ lỗi thật (đừng biến hỏng đĩa thành "mặc định rỗng").
pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, CheckpointError> {
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

/// Get RNG state as hex for manifest
fn encode_rng_hex(world: &World) -> String {
    let mut rng = world.rng.clone();
    hex::encode(&rng.get_word_pos().to_le_bytes())
}

/// Tái tạo generator đúng vị trí trong dãy.
pub fn restore_rng(seed: u64, stream: u64, word_pos: u128) -> ChaCha8Rng {
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
        let ca_for_checkpoint: crate::ca_grid::CellularAutomaton = (&self.ca).into();
        let grid_bytes = encode_ca(&ca_for_checkpoint)?;
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

        // Manifest (checksums) - paths are relative to checkpoint dir root
        let files = vec![
            FileRecord { path: format!("{}/{}", WORLD_DIR, GRID_FILE), blake3: hash_file(&world_dir.join(GRID_FILE))? },
            FileRecord { path: format!("{}/{}", WORLD_DIR, ATOMS_FILE), blake3: hash_file(&world_dir.join(ATOMS_FILE))? },
            FileRecord { path: format!("{}/{}", WORLD_DIR, REGISTRY_FILE), blake3: hash_file(&world_dir.join(REGISTRY_FILE))? },
            FileRecord { path: format!("{}/{}", WORLD_DIR, RNG_FILE), blake3: hash_file(&world_dir.join(RNG_FILE))? },
            FileRecord { path: format!("{}/{}", WORLD_DIR, AIRWAVE_FILE), blake3: hash_file(&world_dir.join(AIRWAVE_FILE))? },
            FileRecord { path: format!("{}/{}", WORLD_DIR, VOCABULARY_FILE), blake3: hash_file(&world_dir.join(VOCABULARY_FILE))? },
        ];
        let mut files = files;
        if comm_dir.join(CONVENTIONS_FILE).exists() {
            files.push(FileRecord {
                path: format!("{}/{}", COMM_DIR, CONVENTIONS_FILE),
                blake3: hash_file(&comm_dir.join(CONVENTIONS_FILE))?,
            });
        }
        if kg_dir.join(GRAPH_FILE).exists() {
            files.push(FileRecord {
                path: format!("{}/{}", KNOWLEDGE_DIR, GRAPH_FILE),
                blake3: hash_file(&kg_dir.join(GRAPH_FILE))?,
            });
        }
        let manifest = Manifest {
            format_version: FORMAT_VERSION_V1,
            git_commit: option_env!("OMIAI_GIT_COMMIT").map(str::to_string),
            step: self.step_count,
            timestamp_utc: chrono::Utc::now().to_rfc3339(),
            rng_seed: self.rng_seed,
            rng_state_hex: encode_rng_hex(self),
            files,
        };
        write_atomic(dir, "manifest.json", &serde_json::to_vec(&manifest)?)?;

        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let world_dir = dir.join(WORLD_DIR);

        // 1. grid
        let grid_bytes = read_required_file(&world_dir.join(GRID_FILE))?;
        let ca_checkpoint: crate::ca_grid::CellularAutomaton = decode_ca(&grid_bytes)?;
        let ca: omiai_world::CellularAutomaton = ca_checkpoint.into();

        // 2. atoms (+ step_count)
        let atoms_bytes = read_required_file(&world_dir.join(ATOMS_FILE))?;
        let atoms_file: AtomsFile = ciborium::de::from_reader(&atoms_bytes[..]).map_err(de_cbor_error)?;
        let step_count = atoms_file.step_count;
        let atoms = atoms_file.atoms;

        // 3. registry
        let registry_bytes = read_required_file(&world_dir.join(REGISTRY_FILE))?;
        let registry_file: RegistryFile = ciborium::de::from_reader(&registry_bytes[..]).map_err(de_cbor_error)?;
        let n_genomes = registry_file.genomes.len();
        let registry = FormulaRegistry::from_genomes_in_order(registry_file.genomes);

        // 4. rng
        let rng_bytes = read_required_file(&world_dir.join(RNG_FILE))?;
        let seed = u64::from_le_bytes(rng_bytes[0..8].try_into().map_err(|_| CheckpointError::Corrupt {
            path: world_dir.join(RNG_FILE),
            expected: "8 bytes seed".into(),
            actual: format!("{} bytes", rng_bytes.len()),
        })?);
        let stream = u64::from_le_bytes(rng_bytes[8..16].try_into().map_err(|_| CheckpointError::Corrupt {
            path: world_dir.join(RNG_FILE),
            expected: "8 bytes stream".into(),
            actual: format!("{} bytes", rng_bytes.len()),
        })?);
        let word_pos = u128::from_le_bytes(rng_bytes[16..32].try_into().map_err(|_| CheckpointError::Corrupt {
            path: world_dir.join(RNG_FILE),
            expected: "16 bytes word_pos".into(),
            actual: format!("{} bytes", rng_bytes.len()),
        })?);
        let rng = restore_rng(seed, stream, word_pos);

        // 5. airwave
        let airwave_bytes = read_required_file(&world_dir.join(AIRWAVE_FILE))?;
        let airwave: Vec<Option<u8>> = ciborium::de::from_reader(&airwave_bytes[..]).map_err(de_cbor_error)?;

        // 6. vocabulary
        let vocab_bytes = read_required_file(&world_dir.join(VOCABULARY_FILE))?;
        let vocabulary: omiai_world::communication::Vocabulary = ciborium::de::from_reader(&vocab_bytes[..]).map_err(de_cbor_error)?;

        // 7. conventions tracker — OPTIONAL (slice 5)
        let conventions: omiai_world::communication::ConventionTracker = match read_optional(&dir.join(COMM_DIR).join(CONVENTIONS_FILE))? {
            Some(bytes) => ciborium::de::from_reader(&bytes[..]).map_err(de_cbor_error)?,
            None => omiai_world::communication::ConventionTracker::default(),
        };

        // 8. knowledge graph — OPTIONAL (slice 5)
        let knowledge: KnowledgeGraph = match read_optional(&dir.join(KNOWLEDGE_DIR).join(GRAPH_FILE))? {
            Some(bytes) => {
                let file: GraphFile = ciborium::de::from_reader(&bytes[..]).map_err(de_cbor_error)?;
                file.into_graph(&dir.join(KNOWLEDGE_DIR).join(GRAPH_FILE))?
            }
            None => KnowledgeGraph::new(),
        };

        // Cross-payload consistency: atom must be inside grid and gene must
        // point to an existing slot. If not checked here, world resume "works"
        // then atoms silently freeze (`registry.get` → None → `continue` in
        // agent_act) — exactly the silent failure §4 spec forbids.
        let n_cells = ca.width * ca.height;
        for atom in &atoms {
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
                path: world_dir.join(AIRWAVE_FILE),
                expected: format!("airwave length = {n_cells}"),
                actual: format!("{} elements", airwave.len()),
            });
        }

        Ok(World {
            ca,
            registry,
            atoms,
            rng,
            rng_seed: seed,
            rng_stream: stream,
            step_count,
            airwave,
            vocabulary,
            conventions,
            knowledge,
        })
    }
}