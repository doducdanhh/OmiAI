//! `Checkpointable` cho `World`: bundle 4 file `world/*` của checkpoint-v1.
//!
//! Layout trong thư mục checkpoint:
//! ```text
//! world/grid.bin       — lưới CA (format ca_grid, xem ca_grid.rs)
//! world/atoms.cbor     — {step_count, atoms[]}
//! world/registry.cbor  — {genomes[]} theo thứ tự slot
//! world/rng_state.bin  — u64 LE seed + u64 LE stream + u128 LE word_pos
//! ```
//!
//! RNG tái tạo: `ChaCha8Rng::seed_from_u64(seed)` → `set_stream(stream)`
//! → `set_word_pos(word_pos)` (ADR-0006).

use std::path::Path;

use omiai_world::atoms::Atom;
use omiai_world::communication::Vocabulary;
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

#[derive(Debug, Serialize, Deserialize)]
struct AtomsFile {
    step_count: u64,
    atoms: Vec<Atom>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryFile {
    genomes: Vec<Genome>,
}

fn cbor_error(e: ciborium::ser::Error<std::io::Error>) -> CheckpointError {
    CheckpointError::Cbor(e.to_string())
}

fn de_cbor_error(e: ciborium::de::Error<std::io::Error>) -> CheckpointError {
    CheckpointError::Cbor(e.to_string())
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
        let mut atoms_buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(&atoms, &mut atoms_buf)
            .map_err(cbor_error)?;
        write_atomic(&world_dir, ATOMS_FILE, atoms_buf.get_ref())?;

        // 3. registry (thứ tự slot — bất biến không-remove, xem registry.rs)
        let registry = RegistryFile { genomes: self.registry.genomes_in_order() };
        let mut reg_buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(&registry, &mut reg_buf)
            .map_err(cbor_error)?;
        write_atomic(&world_dir, REGISTRY_FILE, reg_buf.get_ref())?;

        // 4. rng
        write_atomic(&world_dir, RNG_FILE, &encode_rng(self))?;

        // 5. airwave
        let mut airwave_buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(&self.airwave, &mut airwave_buf)
            .map_err(cbor_error)?;
        write_atomic(&world_dir, AIRWAVE_FILE, airwave_buf.get_ref())?;

        // 6. vocabulary
        let mut vocab_buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(&self.vocabulary, &mut vocab_buf)
            .map_err(cbor_error)?;
        write_atomic(&world_dir, VOCABULARY_FILE, vocab_buf.get_ref())?;

        // 7. manifest với hash cả 6 file
        let mut records = Vec::with_capacity(6);
        for name in [GRID_FILE, ATOMS_FILE, REGISTRY_FILE, RNG_FILE, AIRWAVE_FILE, VOCABULARY_FILE] {
            let blake3 = hash_file(&world_dir.join(name))?;
            records.push(FileRecord {
                path: format!("{WORLD_DIR}/{name}"),
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
        let ca = decode_ca(&std::fs::read(&grid_path).map_err(|source| {
            CheckpointError::Io { path: grid_path.clone(), source }
        })?)?;

        // atoms
        let atoms_path = world_dir.join(ATOMS_FILE);
        let atoms_bytes =
            std::fs::read(&atoms_path).map_err(|source| {
                CheckpointError::Io { path: atoms_path.clone(), source }
            })?;
        let atoms_file: AtomsFile = ciborium::de::from_reader(&atoms_bytes[..])
            .map_err(de_cbor_error)?;

        let n_cells = ca.width * ca.height;

        // registry
        let reg_path = world_dir.join(REGISTRY_FILE);
        let reg_bytes = std::fs::read(&reg_path).map_err(|source| {
            CheckpointError::Io { path: reg_path.clone(), source }
        })?;
        let registry_file: RegistryFile =
            ciborium::de::from_reader(&reg_bytes[..]).map_err(de_cbor_error)?;

        // rng
        let rng_path = world_dir.join(RNG_FILE);
        let rng_bytes = std::fs::read(&rng_path).map_err(|source| {
            CheckpointError::Io { path: rng_path.clone(), source }
        })?;
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
        let airwave_bytes = std::fs::read(&airwave_path).map_err(|source| {
            CheckpointError::Io { path: airwave_path.clone(), source }
        })?;
        let airwave: Vec<Option<u8>> = ciborium::de::from_reader(&airwave_bytes[..])
            .map_err(de_cbor_error)?;

        // vocabulary
        let vocab_path = world_dir.join(VOCABULARY_FILE);
        let vocab_bytes = std::fs::read(&vocab_path).map_err(|source| {
            CheckpointError::Io { path: vocab_path.clone(), source }
        })?;
        let vocabulary: Vocabulary = ciborium::de::from_reader(&vocab_bytes[..])
            .map_err(de_cbor_error)?;

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
        })
    }
}
