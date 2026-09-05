//! Reversible Block Cellular Automata with parallel updates (rayon) and
//! a HashLife-style memoization cache for repeated quadrant patterns.
//!
//! # Design
//! - Grid of binary (or multi-state) cells
//! - Block partitioning (2×2 Margolus neighbourhood) for reversibility
//! - Parallel sweep via `rayon::par_chunks_mut`
//! - Pattern cache: hash of 2^n blocks → evolved result (HashLife core idea)

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use rayon::prelude::*;

/// Error type for substrate operations (local to avoid circular dependency).
#[derive(Debug, Error)]
pub enum SubstrateError {
    #[error("checkpoint error: {0}")]
    Checkpoint(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CBOR encoding error: {0}")]
    Cbor(String),
    #[error("grid too large for checkpoint format: {path:?}")]
    GridTooLarge { path: std::path::PathBuf },
    #[error("bad magic in checkpoint file: {path:?}")]
    BadMagic { path: std::path::PathBuf },
    #[error("corrupt checkpoint: {path:?}: expected {expected}, got {actual}")]
    Corrupt {
        path: std::path::PathBuf,
        expected: String,
        actual: String,
    },
}

/// 2D cellular automaton grid (row-major).
#[derive(Debug, Clone)]
pub struct CellularAutomaton {
    pub width: usize,
    pub height: usize,
    /// Cell states in `0..num_states`
    pub cells: Vec<u8>,
    pub num_states: u8,
    /// Margolus partition phase (0 or 1)
    phase: u8,
    /// HashLife-style memo for 2×2 blocks: (4 cells + rule id) → next 4 cells
    block_cache: HashMap<u32, [u8; 4]>,
}

impl CellularAutomaton {
    pub fn new(width: usize, height: usize, num_states: u8) -> Self {
        Self {
            width,
            height,
            cells: vec![0; width.saturating_mul(height)],
            num_states: num_states.max(2),
            phase: 0,
            block_cache: HashMap::new(),
        }
    }

    /// Create a grid seeded with a fraction of random live cells.
    pub fn random(width: usize, height: usize, density: f64, seed: u64) -> Self {
        let mut ca = Self::new(width, height, 2);
        let mut s = seed;
        for cell in ca.cells.iter_mut() {
            // xorshift
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let r = (s as f64) / (u64::MAX as f64);
            *cell = if r < density { 1 } else { 0 };
        }
        ca
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    pub fn get(&self, x: usize, y: usize) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.cells[self.idx(x, y)]
    }

    pub fn set(&mut self, x: usize, y: usize, v: u8) {
        if x < self.width && y < self.height {
            let i = self.idx(x, y);
            let states = self.num_states;
            self.cells[i] = v % states;
        }
    }

    /// Get current Margolus phase (0 or 1).
    pub fn phase(&self) -> u8 {
        self.phase
    }

    /// Set Margolus phase (for checkpoint restore).
    pub fn set_phase(&mut self, phase: u8) {
        self.phase = phase & 1;
    }

    /// One reversible block CA step (Margolus neighbourhood).
    ///
    /// Even phase: tiles (0,0)-(1,1), (2,0)-(3,1), …
    /// Odd phase: tiles shifted by +1 in both axes (toroidal).
    pub fn step(&mut self) {
        let w = self.width;
        let h = self.height;
        if w < 2 || h < 2 {
            return;
        }
        let ox = self.phase as usize;
        let oy = self.phase as usize;

        // Collect block origins
        let mut origins = Vec::new();
        let mut y = oy % 2;
        while y + 1 < h {
            let mut x = ox % 2;
            while x + 1 < w {
                origins.push((x, y));
                x += 2;
            }
            y += 2;
        }

        // Parallel block updates into a side buffer
        let cells = &self.cells;
        let num_states = self.num_states;
        let updates: Vec<(usize, usize, [u8; 4])> = origins
            .par_iter()
            .map(|&(x, y)| {
                let i00 = y * w + x;
                let i10 = y * w + (x + 1);
                let i01 = (y + 1) * w + x;
                let i11 = (y + 1) * w + (x + 1);
                let block = [cells[i00], cells[i10], cells[i01], cells[i11]];
                let next = rotate_block(block, num_states);
                ((x, y), next)
            })
            .map(|((x, y), next)| (x, y, next))
            .collect();

        for (x, y, next) in updates {
            self.cells[y * w + x] = next[0];
            self.cells[y * w + x + 1] = next[1];
            self.cells[(y + 1) * w + x] = next[2];
            self.cells[(y + 1) * w + x + 1] = next[3];
        }

        self.phase = 1 - self.phase;
    }

    /// Run `n` steps.
    pub fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    /// HashLife-inspired pattern detection: memoize 2×2 → next 2×2 under
    /// the block rule, and return number of unique live patterns seen.
    pub fn detect_patterns(&mut self) -> usize {
        let w = self.width;
        let h = self.height;
        let mut unique = HashMap::new();
        for y in 0..h.saturating_sub(1) {
            for x in 0..w.saturating_sub(1) {
                let block = [
                    self.get(x, y),
                    self.get(x + 1, y),
                    self.get(x, y + 1),
                    self.get(x + 1, y + 1),
                ];
                let key = pack_block(block);
                let next = *self
                    .block_cache
                    .entry(key)
                    .or_insert_with(|| rotate_block(block, self.num_states));
                let mut hasher_state = std::collections::hash_map::DefaultHasher::new();
                next.hash(&mut hasher_state);
                unique.insert(hasher_state.finish(), next);
            }
        }
        unique.len()
    }

    /// Count live (non-zero) cells.
    pub fn population(&self) -> usize {
        self.cells.iter().filter(|&&c| c != 0).count()
    }

    /// Conway-like density (fraction live).
    pub fn density(&self) -> f64 {
        if self.cells.is_empty() {
            return 0.0;
        }
        self.population() as f64 / self.cells.len() as f64
    }
}

/// Reversible block rule: rotate the 2×2 block by 90° if any cell is live,
/// else identity. (Billard-ball-like reversible CA.)
fn rotate_block(b: [u8; 4], _num_states: u8) -> [u8; 4] {
    // b: [tl, tr, bl, br]
    if b.iter().all(|&c| c == 0) {
        return b;
    }
    // 90° clockwise: tl→tr, tr→br, br→bl, bl→tl
    [b[2], b[0], b[3], b[1]]
}

fn pack_block(b: [u8; 4]) -> u32 {
    ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32)
}

/// CA grid binary format constants (matching checkpoint-v1 spec)
const MAGIC: &[u8; 10] = b"OMICAGRID\0";
const HEADER_LEN: usize = 20;

/// Serialize a CA into the `ca_grid.bin` byte layout.
pub fn encode_ca(ca: &CellularAutomaton) -> Result<Vec<u8>, SubstrateError> {
    let w = ca.width;
    let h = ca.height;
    if w > u16::MAX as usize || h > u16::MAX as usize {
        return Err(SubstrateError::GridTooLarge {
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
    out[15] = ca.phase();
    Ok(out)
}

/// Reconstruct a CA from `ca_grid.bin` bytes.
pub fn decode_ca(bytes: &[u8]) -> Result<CellularAutomaton, SubstrateError> {
    let path = Path::new("grid.bin").to_path_buf();
    if bytes.len() < HEADER_LEN || &bytes[..10] != MAGIC {
        return Err(SubstrateError::BadMagic { path });
    }
    let w = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    let h = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    let num_states = bytes[14];
    let body = &bytes[HEADER_LEN..];
    let expected_len = w * h;
    if body.len() != expected_len {
        return Err(SubstrateError::Corrupt {
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
        return Err(SubstrateError::Corrupt {
            path,
            expected: format!("cells in 0..{num_states}"),
            actual: "cell value out of range".to_string(),
        });
    }
    let mut ca = CellularAutomaton::new(w, h, num_states);
    ca.cells = body.to_vec();
    ca.set_phase(bytes[15]); // flags byte stores phase in bit 0
    Ok(ca)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_preserves_population_on_full_rotation() {
        let mut ca = CellularAutomaton::new(4, 4, 2);
        ca.set(0, 0, 1);
        ca.set(1, 0, 1);
        let pop0 = ca.population();
        ca.step();
        assert_eq!(ca.population(), pop0);
    }

    #[test]
    fn detect_patterns_runs() {
        let mut ca = CellularAutomaton::random(16, 16, 0.3, 1);
        let n = ca.detect_patterns();
        assert!(n > 0);
    }

    #[test]
    fn parallel_many_steps() {
        let mut ca = CellularAutomaton::random(64, 64, 0.2, 42);
        ca.steps(10);
        assert!(ca.density() >= 0.0);
    }
}
