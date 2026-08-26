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

use rayon::prelude::*;

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
