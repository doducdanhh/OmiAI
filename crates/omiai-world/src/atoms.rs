//! Atom: đơn vị sống trên lưới — vị trí, năng lượng, gene (con trỏ Formula).
//!
//! Atom KHÔNG sở hữu Formula; gene chỉ là [`FormulaId`] trỏ vào
//! [`FormulaRegistry`](crate::registry::FormulaRegistry) của World.

use serde::{Deserialize, Serialize};

use crate::ecology::{
    ENERGY_MAX, ENERGY_PER_RESOURCE_UNIT, METABOLIC_COST, REPRODUCE_THRESHOLD,
};
use crate::registry::FormulaId;

/// Một thực thể sống.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atom {
    /// Ô lưới đang chiếm (cột, hàng).
    pub pos: (usize, usize),
    /// Năng lượng, clamp [0, ENERGY_MAX].
    pub energy: f64,
    /// Gene — handle vào FormulaRegistry của World.
    pub gene: FormulaId,
    /// Số bước đã sống.
    pub age: u64,
}

impl Atom {
    /// Trừ metabolic cost. Trả về `false` nếu atom chết (energy ≤ 0).
    pub fn metabolize(&mut self) -> bool {
        self.energy -= METABOLIC_COST;
        if self.energy <= 0.0 {
            self.energy = 0.0;
            return false;
        }
        true
    }

    /// Ăn tài nguyên: giá trị ô ≥ 2 quy đổi thành năng lượng, clamp max.
    /// (Caller chịu trách nhiệm xoá tài nguyên khỏi lưới.)
    pub fn feed(&mut self, cell_value: u8) {
        debug_assert!(cell_value >= 2, "feed chỉ dùng cho ô tài nguyên");
        self.energy =
            (self.energy + (cell_value as f64) * ENERGY_PER_RESOURCE_UNIT).min(ENERGY_MAX);
    }

    /// Sinh sản: cha giữ nửa năng lượng, trả về nửa cho con.
    /// Trả về `None` nếu chưa đạt ngưỡng sinh sản.
    pub fn split_energy(&mut self) -> Option<f64> {
        if self.energy < REPRODUCE_THRESHOLD {
            return None;
        }
        let child = self.energy / 2.0;
        self.energy -= child;
        Some(child)
    }
}

/// Ô kề trống đầu tiên theo thứ tự quét cố định N, E, S, W.
///
/// `in_bounds(x, y)` do caller cung cấp (biết w/h); `occupied(x, y)` tra
/// tập hợp vị trí atom đang sống. Trả về toạ độ (x, y) của ô tìm được.
pub fn first_free_neighbor(
    pos: (usize, usize),
    in_bounds: &dyn Fn(usize, usize) -> bool,
    occupied: &dyn Fn(usize, usize) -> bool,
) -> Option<(usize, usize)> {
    // N, E, S, W — dùng isize để lùi biên an toàn.
    const OFFSETS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    let (px, py) = (pos.0 as isize, pos.1 as isize);
    for (dx, dy) in OFFSETS {
        let (x, y) = (px + dx, py + dy);
        if x < 0 || y < 0 {
            continue;
        }
        let (x, y) = (x as usize, y as usize);
        if in_bounds(x, y) && !occupied(x, y) {
            return Some((x, y));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom_at(x: usize, y: usize, energy: f64) -> Atom {
        Atom { pos: (x, y), energy, gene: FormulaId::from_slot(0), age: 0 }
    }

    #[test]
    fn metabolize_kills_starved_atom() {
        let mut a = atom_at(0, 0, METABOLIC_COST - 0.01);
        assert!(!a.metabolize());
        assert_eq!(a.energy, 0.0);
    }

    #[test]
    fn metabolize_survives_with_leftover() {
        let mut a = atom_at(0, 0, METABOLIC_COST + 0.01);
        assert!(a.metabolize());
        assert!((a.energy - 0.01).abs() < 1e-12);
    }

    #[test]
    fn feed_adds_resource_energy_clamped() {
        let mut a = atom_at(0, 0, 0.5);
        a.feed(2); // 0.5 + 2*0.2 = 0.9
        assert!((a.energy - 0.9).abs() < 1e-12);
        a.feed(3); // 0.9 + 0.6 = 1.5 → clamp 1.0
        assert!((a.energy - ENERGY_MAX).abs() < 1e-12);
    }

    #[test]
    fn split_only_above_threshold_and_halves() {
        let mut a = atom_at(0, 0, 0.5);
        assert!(a.split_energy().is_none()); // dưới ngưỡng

        let mut b = atom_at(0, 0, REPRODUCE_THRESHOLD);
        let child = b.split_energy().unwrap();
        assert!((child - REPRODUCE_THRESHOLD / 2.0).abs() < 1e-12);
        assert!((b.energy - REPRODUCE_THRESHOLD / 2.0).abs() < 1e-12);
    }

    #[test]
    fn first_free_neighbor_scans_n_esw_order() {
        let bounds = |_: usize, _: usize| true;
        let empty = |_: usize, _: usize| false;
        // Tất cả trống → chọn N trước.
        assert_eq!(first_free_neighbor((2, 2), &bounds, &empty), Some((2, 1)));

        // N và E bị chiếm → chọn S.
        let occ =
            |x: usize, y: usize| (x == 2 && y == 1) || (x == 3 && y == 2);
        assert_eq!(first_free_neighbor((2, 2), &bounds, &occ), Some((2, 3)));
    }

    #[test]
    fn first_free_neighbor_respects_bounds() {
        let bounds = |x: usize, y: usize| x < 3 && y < 3;
        let empty = |_: usize, _: usize| false;
        // (0,0): N ngoài biên, E=(1,0) trống.
        assert_eq!(first_free_neighbor((0, 0), &bounds, &empty), Some((1, 0)));
    }

    #[test]
    fn atom_serialization_round_trip() {
        let a = atom_at(3, 4, 0.75);
        let bytes = serde_json::to_vec(&a).unwrap();
        let back: Atom = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, a);
    }
}
