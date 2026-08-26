//! `Checkpointable` for `CellularAutomaton`: the `ca_grid.bin` payload of
//! checkpoint-v1 (format-spec/checkpoint-v1.md §3).
//!
//! Binary layout, little-endian throughout:
//! ```text
//! offset  size  field
//! 0       10    magic "OMICAGRID\0"
//! 10      2     width      u16
//! 12      2     height     u16
//! 14      1     num_states u8
//! 15      1     flags      u8   (=0)
//! 16      4     reserved   u32  (=0)
//! 20..          body: bit-packed row-major LSB-first cells
//! ```
//!
//! Note: the slice-1 plan called this a "16-byte header", but the listed
//! fields sum to 20; the field list wins (the u32 reserved word is kept
//! for forward compatibility).
//!
//! Only the grid is persistent state: `phase` and `block_cache` are
//! private simulation bookkeeping in `omiai_world` and reset on load.

use std::path::Path;

use omiai_world::substrate::CellularAutomaton;

use crate::error::CheckpointError;
use crate::fsutil::{hash_file, write_atomic};
use crate::manifest::{FileRecord, Manifest};
use crate::traits::Checkpointable;

const MAGIC: &[u8; 10] = b"OMICAGRID\0";
/// magic(10) + width(2) + height(2) + num_states(1) + flags(1) + reserved(4)
const HEADER_LEN: usize = 20;
const GRID_FILE: &str = "grid.bin";

impl Checkpointable for CellularAutomaton {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let bytes = encode(self)?;
        write_atomic(dir, GRID_FILE, &bytes)?;
        let hash = hash_file(&dir.join(GRID_FILE))?;
        Manifest::write(
            dir,
            &[FileRecord {
                path: GRID_FILE.to_string(),
                blake3: hash,
            }],
        )
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let manifest = Manifest::read(dir)?;
        if manifest.format_version != crate::manifest::FORMAT_VERSION_V1 {
            return Err(CheckpointError::MissingField(format!(
                "unsupported format_version {}",
                manifest.format_version
            )));
        }
        let path = dir.join(GRID_FILE);
        let bytes = std::fs::read(&path).map_err(|source| CheckpointError::Io {
            path: path.clone(),
            source,
        })?;
        // Verify integrity against the manifest before decoding.
        let record = manifest
            .files
            .iter()
            .find(|f| f.path == GRID_FILE)
            .ok_or_else(|| CheckpointError::MissingField(GRID_FILE.to_string()))?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(&bytes);
        let actual = hasher.finalize().to_hex().to_string();
        if actual != record.blake3 {
            return Err(CheckpointError::Corrupt {
                path,
                expected: record.blake3.clone(),
                actual,
            });
        }
        decode(&bytes)
    }
}

/// Serialize a CA into the `ca_grid.bin` byte layout.
fn encode(ca: &CellularAutomaton) -> Result<Vec<u8>, CheckpointError> {
    let w = ca.width;
    let h = ca.height;
    if w > u16::MAX as usize || h > u16::MAX as usize {
        return Err(CheckpointError::GridTooLarge {
            path: Path::new(GRID_FILE).to_path_buf(),
        });
    }
    let body_len = w * h;
    let mut out = Vec::with_capacity(HEADER_LEN + body_len.div_ceil(8));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    out.push(ca.num_states);
    out.push(0u8); // flags
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved

    // Bit-packed row-major LSB-first.
    let mut acc = 0u8;
    let mut bit = 0u32;
    for &c in &ca.cells {
        if c != 0 {
            acc |= 1 << bit;
        }
        bit += 1;
        if bit == 8 {
            out.push(acc);
            acc = 0;
            bit = 0;
        }
    }
    if bit > 0 {
        out.push(acc);
    }
    Ok(out)
}

/// Reconstruct a CA from `ca_grid.bin` bytes.
fn decode(bytes: &[u8]) -> Result<CellularAutomaton, CheckpointError> {
    let path = Path::new(GRID_FILE).to_path_buf();
    if bytes.len() < HEADER_LEN || &bytes[..10] != MAGIC {
        return Err(CheckpointError::BadMagic { path });
    }
    let w = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    let h = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    let num_states = bytes[14];
    let body = &bytes[HEADER_LEN..];
    let expected_len = (w * h).div_ceil(8);
    if body.len() < expected_len {
        return Err(CheckpointError::Corrupt {
            path,
            expected: format!("body len {expected_len}"),
            actual: format!("body len {}", body.len()),
        });
    }
    // `phase` and `block_cache` are private simulation bookkeeping;
    // reconstruct via the public constructor, then restore the decoded
    // cells directly (the public `set` clamps values into
    // `0..num_states`, which would corrupt multi-state grids whose
    // stored bits mean "live").
    let mut ca = CellularAutomaton::new(w, h, num_states);
    let n = w * h;
    let mut cells = vec![0u8; n];
    for (i, cell) in cells.iter_mut().enumerate() {
        if body[i / 8] >> (i % 8) & 1 == 1 {
            *cell = 1;
        }
    }
    ca.cells = cells;
    Ok(ca)
}
