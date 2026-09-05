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

use serde::{Deserialize, Serialize};

use crate::error::CheckpointError;
use crate::fsutil::{hash_file, write_atomic, read_file};
use crate::manifest::{FileRecord, Manifest};
use crate::traits::Checkpointable;

const MAGIC: &[u8; 10] = b"OMICAGRID\0";
/// magic(10) + width(2) + height(2) + num_states(1) + flags(1) + reserved(4)
const HEADER_LEN: usize = 20;
const GRID_FILE: &str = "grid.bin";

/// Cellular Automaton grid state (minimal subset needed for checkpointing)
/// This mirrors the persistent state subset of `omiai_world::CellularAutomaton`.
#[derive(Debug, Clone)]
pub struct CellularAutomaton {
    pub width: usize,
    pub height: usize,
    /// Cell states in `0..num_states`
    pub cells: Vec<u8>,
    pub num_states: u8,
    /// Margolus partition phase (0 or 1)
    pub phase: u8,
}

impl CellularAutomaton {
    pub fn new(width: usize, height: usize, num_states: u8) -> Self {
        Self {
            width,
            height,
            cells: vec![0; width * height],
            num_states,
            phase: 0,
        }
    }

    pub fn random(width: usize, height: usize, density: f64, seed: u64) -> Self {
        use rand::RngCore;
        use rand_chacha::ChaCha8Rng;
        use rand_chacha::rand_core::SeedableRng;
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut cells = Vec::with_capacity(width * height);
        for _ in 0..width * height {
            cells.push(if (rng.next_u32() as f64) / (u32::MAX as f64) < density { 1 } else { 0 });
        }
        Self {
            width,
            height,
            cells,
            num_states: 2,
            phase: 0,
        }
    }

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|&&c| c > 0).count()
    }

    pub fn step(&mut self) {
        // Simple Margolus step (conserves population)
        let w = self.width;
        let h = self.height;
        let mut new_cells = self.cells.clone();
        for y in 0..h {
            for x in 0..w {
                // Margolus partition
                let bx = (x + self.phase as usize) & !1;
                let by = (y + self.phase as usize) & !1;
                if bx + 1 < w && by + 1 < h {
                    let idx00 = by * w + bx;
                    let idx01 = by * w + bx + 1;
                    let idx10 = (by + 1) * w + bx;
                    let idx11 = (by + 1) * w + bx + 1;
                    // Rotate 2x2 block
                    let c00 = self.cells[idx00];
                    let c01 = self.cells[idx01];
                    let c10 = self.cells[idx10];
                    let c11 = self.cells[idx11];
                    new_cells[idx00] = c10;
                    new_cells[idx01] = c00;
                    new_cells[idx10] = c11;
                    new_cells[idx11] = c01;
                }
            }
        }
        self.cells = new_cells;
        self.phase ^= 1;
    }
}

impl From<&omiai_world::CellularAutomaton> for CellularAutomaton {
    fn from(ca: &omiai_world::CellularAutomaton) -> Self {
        Self {
            width: ca.width,
            height: ca.height,
            cells: ca.cells.clone(),
            num_states: ca.num_states,
            phase: ca.phase(),
        }
    }
}

impl From<CellularAutomaton> for omiai_world::CellularAutomaton {
    fn from(ca: CellularAutomaton) -> Self {
        use std::collections::HashMap;
        let mut ca_world = Self::new(ca.width, ca.height, ca.num_states);
        ca_world.cells = ca.cells;
        ca_world.set_phase(ca.phase);
        ca_world
    }
}

/// Encode CA into the `ca_grid.bin` byte layout.
pub fn encode_ca(ca: &CellularAutomaton) -> Result<Vec<u8>, CheckpointError> {
    let w = ca.width;
    let h = ca.height;
    if w > u16::MAX as usize || h > u16::MAX as usize {
        return Err(CheckpointError::GridTooLarge {
            path: Path::new("grid.bin").to_path_buf(),
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

    // Row-major, one byte per cell (preserving values 0..num_states).
    for &c in &ca.cells {
        debug_assert!(c < ca.num_states || c == 0);
        out.push(c);
    }
    // Store phase in flags byte (index 15 = after magic 10 + width 2 + height 2 + num_states 1)
    out[15] = ca.phase;
    Ok(out)
}

/// Decode CA from the `ca_grid.bin` byte layout.
pub fn decode_ca(bytes: &[u8]) -> Result<CellularAutomaton, CheckpointError> {
    if bytes.len() < HEADER_LEN {
        return Err(CheckpointError::Corrupt {
            path: Path::new("grid.bin").to_path_buf(),
            expected: format!("at least {} bytes", HEADER_LEN),
            actual: format!("{} bytes", bytes.len()),
        });
    }
    if &bytes[0..10] != MAGIC {
        return Err(CheckpointError::Corrupt {
            path: Path::new("grid.bin").to_path_buf(),
            expected: "magic OMICAGRID\\0".to_string(),
            actual: format!("{:?}", &bytes[0..10]),
        });
    }
    let w = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    let h = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    let num_states = bytes[14];
    let flags = bytes[15];
    let _reserved = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);

    let expected_body = w * h;
    let body = &bytes[HEADER_LEN..];
    if body.len() != expected_body {
        return Err(CheckpointError::Corrupt {
            path: Path::new("grid.bin").to_path_buf(),
            expected: format!("body len {expected_body}"),
            actual: format!("body len {}", body.len()),
        });
    }
    // Validate cells are in range
    if body.iter().any(|&c| c >= num_states && num_states > 0) {
        return Err(CheckpointError::Corrupt {
            path: Path::new("grid.bin").to_path_buf(),
            expected: format!("cells in 0..{num_states}"),
            actual: "cell value out of range".to_string(),
        });
    }
    let mut ca = CellularAutomaton {
        width: w,
        height: h,
        cells: body.to_vec(),
        num_states,
        phase: flags,
    };
    Ok(ca)
}

impl Checkpointable for CellularAutomaton {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let bytes = encode_ca(self)?;
        write_atomic(dir, GRID_FILE, &bytes)?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let grid_path = dir.join(GRID_FILE);
        let bytes = read_file(&grid_path)?;
        decode_ca(&bytes)
    }
}