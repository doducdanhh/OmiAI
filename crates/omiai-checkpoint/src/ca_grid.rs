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
//! 20..          body: row-major cells, ONE BYTE per cell (raw state value,
//!               0..num_states — supports resource states 2/3 của World)
//! ```
//!
//! Note: the slice-1 plan called this a "16-byte header", but the listed
//! fields sum to 20; the field list wins (the u32 reserved word is kept
//! for forward compatibility).
//!
//! Slice-1 shipped a bit-packed body ("bit set = live"); slice-2 requires
//! bit-exact resume of multi-state worlds (resource cells 2/3), so the
//! body is now one byte per cell. The magic is unchanged — v1 readers of
//! the old format must reject via the body-length check.
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
        let bytes = encode_ca(self)?;
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
        decode_ca(&bytes)
    }
}

/// Serialize a CA into the `ca_grid.bin` byte layout.
pub(crate) fn encode_ca(ca: &CellularAutomaton) -> Result<Vec<u8>, CheckpointError> {
    let w = ca.width;
    let h = ca.height;
    if w > u16::MAX as usize || h > u16::MAX as usize {
        return Err(CheckpointError::GridTooLarge {
            path: Path::new(GRID_FILE).to_path_buf(),
        });
    }
    let body_len = w * h;
    let mut out = Vec::with_capacity(HEADER_LEN + body_len);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    out.push(ca.num_states);
    out.push(0u8); // flags
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved

    // Row-major, một byte mỗi cell (giữ nguyên giá trị 0..num_states).
    for &c in &ca.cells {
        debug_assert!(c < ca.num_states || c == 0);
        out.push(c);
    }
    // Store phase in flags byte (index 15 = after magic 10 + width 2 + height 2 + num_states 1)
    out[15] = ca.phase();
    Ok(out)
}

/// Reconstruct a CA from `ca_grid.bin` bytes.
pub(crate) fn decode_ca(bytes: &[u8]) -> Result<CellularAutomaton, CheckpointError> {
    let path = Path::new(GRID_FILE).to_path_buf();
    if bytes.len() < HEADER_LEN || &bytes[..10] != MAGIC {
        return Err(CheckpointError::BadMagic { path });
    }
    let w = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    let h = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    let num_states = bytes[14];
    let body = &bytes[HEADER_LEN..];
    let expected_len = w * h;
    if body.len() != expected_len {
        return Err(CheckpointError::Corrupt {
            path,
            expected: format!("body len {expected_len}"),
            actual: format!("body len {}", body.len()),
        });
    }
    // `phase` and `block_cache` are private simulation bookkeeping;
    // reconstruct via the public constructor, then restore the decoded
    // cells directly (the public `set` clamps values into
    // `0..num_states`, which would corrupt resource states 2/3).
    if body.iter().any(|&c| c >= num_states && num_states > 0) {
        return Err(CheckpointError::Corrupt {
            path,
            expected: format!("cells in 0..{num_states}"),
            actual: "cell value out of range".to_string(),
        });
    }
    let mut ca = CellularAutomaton::new(w, h, num_states);
    ca.cells = body.to_vec();
    ca.set_phase(bytes[15]); // flags byte stores phase in bit 0 (index 15 = magic 10 + width 2 + height 2 + num_states 1 + flags 1 = 16 bytes header? Wait: MAGIC 10 + width 2 + height 2 + num_states 1 = 15, so flags is index 15)
    Ok(ca)
}
