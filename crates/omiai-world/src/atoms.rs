//! Atom: đơn vị sống trên lưới — vị trí, năng lượng, gene (con trỏ Formula).
//!
//! Atom KHÔNG sở hữu Formula; gene chỉ là [`FormulaId`] trỏ vào
//! [`FormulaRegistry`](crate::registry::FormulaRegistry) của World.

use serde::{Deserialize, Serialize};

use crate::ecology::{ENERGY_MAX, ENERGY_PER_RESOURCE_UNIT, METABOLIC_COST, REPRODUCE_THRESHOLD};
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
    /// Gene tiếng nói: 0 arm (câm) hoặc đúng `N_SYMBOLS` arm.
    ///
    /// `#[serde(default)]` là hợp đồng tương thích ngược với `atoms.cbor`
    /// của slice 2 (không có khoá này) — atom cũ hồi sinh thành câm, spec
    /// §6.4. Bỏ attribute này = checkpoint slice 2 hết đọc được.
    #[serde(default)]
    pub voice: Vec<FormulaId>,
}

impl Atom {
    /// Atom không phát được ký hiệu nào.
    pub fn is_mute(&self) -> bool {
        self.voice.is_empty()
    }

    /// Bất biến arity: rỗng (câm) hoặc đủ `N_SYMBOLS` arm.
    pub fn voice_is_valid(&self) -> bool {
        self.voice.is_empty() || self.voice.len() == crate::communication::N_SYMBOLS
    }

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

    /// Đã đủ năng lượng để sinh sản chưa (không thay đổi trạng thái).
    ///
    /// Caller phải kiểm cái này TRƯỚC khi tìm ô đặt con, rồi mới gọi
    /// [`Atom::split_energy`] — nếu split trước mà sau đó không có ô trống,
    /// năng lượng cha bốc hơi mà chẳng sinh được ai.
    pub fn can_split(&self) -> bool {
        self.energy >= REPRODUCE_THRESHOLD
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
        Atom {
            pos: (x, y),
            energy,
            gene: FormulaId::from_slot(0),
            age: 0,
            voice: Vec::new(),
        }
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
    fn can_split_agrees_with_split_energy() {
        // can_split là cửa kiểm không-thay-đổi-trạng-thái cho split_energy:
        // hai hàm phải luôn đồng ý, và can_split không được trừ năng lượng.
        for energy in [
            0.0,
            REPRODUCE_THRESHOLD - 0.01,
            REPRODUCE_THRESHOLD,
            ENERGY_MAX,
        ] {
            let mut a = atom_at(0, 0, energy);
            let expected = a.can_split();
            assert_eq!(a.energy, energy, "can_split không được đổi năng lượng");
            assert_eq!(a.split_energy().is_some(), expected);
        }
    }

    #[test]
    fn first_free_neighbor_scans_n_esw_order() {
        let bounds = |_: usize, _: usize| true;
        let empty = |_: usize, _: usize| false;
        // Tất cả trống → chọn N trước.
        assert_eq!(first_free_neighbor((2, 2), &bounds, &empty), Some((2, 1)));

        // N và E bị chiếm → chọn S.
        let occ = |x: usize, y: usize| (x == 2 && y == 1) || (x == 3 && y == 2);
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

    #[test]
    fn slice2_atom_cbor_deserializes_to_mute() {
        // Bản ghi CBOR đúng hình dạng slice 2: KHÔNG có khoá `voice`.
        // Đây là hợp đồng tương thích ngược của spec §6.4 — nếu ai đó bỏ
        // #[serde(default)] thì checkpoint slice 2 hết đọc được, và test này
        // là chỗ duy nhất phát hiện ra trước khi người dùng mất dữ liệu.
        #[derive(serde::Serialize)]
        struct Slice2Atom {
            pos: (usize, usize),
            energy: f64,
            gene: FormulaId,
            age: u64,
        }
        let old = Slice2Atom {
            pos: (3, 4),
            energy: 0.75,
            gene: FormulaId::from_slot(0),
            age: 9,
        };
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&old, &mut buf).unwrap();

        let back: Atom = ciborium::de::from_reader(&buf[..]).unwrap();
        assert_eq!(back.pos, (3, 4));
        assert_eq!(back.age, 9);
        assert!(
            back.voice.is_empty(),
            "atom slice 2 phải hồi sinh thành câm"
        );
        assert!(back.is_mute());
        assert!(back.voice_is_valid());
    }

    #[test]
    fn voice_is_valid_only_for_empty_or_full_arity() {
        let mut a = atom_at(0, 0, 0.5);
        assert!(a.voice_is_valid()); // rỗng
        a.voice = vec![FormulaId::from_slot(0); crate::communication::N_SYMBOLS];
        assert!(a.voice_is_valid() && !a.is_mute());
        a.voice = vec![FormulaId::from_slot(0); crate::communication::N_SYMBOLS - 1];
        assert!(!a.voice_is_valid(), "arity thiếu = ký hiệu không tồn tại");
    }

    #[test]
    fn atom_with_voice_round_trips_cbor() {
        let mut a = atom_at(1, 2, 0.5);
        a.voice = (0..crate::communication::N_SYMBOLS)
            .map(|i| FormulaId::from_slot(i as u32))
            .collect();
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&a, &mut buf).unwrap();
        let back: Atom = ciborium::de::from_reader(&buf[..]).unwrap();
        assert_eq!(back, a);
    }
}
