//! FormulaRegistry: kho genome dùng chung cho mọi atom (ADR-0004, Cách 1).
//!
//! Genome là [`LtlFormula`]; atom chỉ giữ handle [`FormulaId`] nên nhiều
//! atom có thể chia sẻ một gene. Registry sống trong `World`, không global.
//!
//! Bất biến slice-2: KHÔNG có remove — arena luôn đặc, thứ tự insertion ==
//! thứ tự slot, nhờ đó serialize là `Vec<Genome>` theo thứ tự và load chỉ
//! cần insert lại tuần tự. (GC/refcount genome là việc lát sau — giới hạn
//! đã biết: genome chết chủ vẫn nằm lại registry.)

use generational_arena::Arena;
use omiai_core::ltl::LtlFormula;
use serde::{Deserialize, Serialize};

/// Một genome: công thức LTL điều khiển hành vi atom + fitness cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    pub formula: LtlFormula,
    /// Cache kết quả đánh giá; `None` = chưa đánh giá.
    pub fitness: Option<f64>,
}

/// Handle generational tới genome trong registry.
///
/// Serialize/Deserialize dưới dạng **u32 slot-index** (yêu cầu spec §1.1:
/// atom lưu slot để map về id mới sau load). Hợp lệ nhờ bất biến
/// không-remove: generation luôn 0, slot == vị trí insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormulaId(generational_arena::Index);

impl FormulaId {
    /// Slot index (ổn định vì arena không bao giờ có lỗ hổng ở slice này).
    pub fn slot(self) -> u32 {
        let (idx, _gen) = self.0.into_raw_parts();
        idx as u32
    }

    /// Dựng lại handle từ slot index (generation luôn 0 khi không remove).
    pub fn from_slot(slot: u32) -> Self {
        Self(generational_arena::Index::from_raw_parts(slot as usize, 0))
    }
}

impl Serialize for FormulaId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.slot().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FormulaId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from_slot(u32::deserialize(deserializer)?))
    }
}

/// Kho genome dùng chung.
#[derive(Debug, Default)]
pub struct FormulaRegistry {
    arena: Arena<Genome>,
}

impl FormulaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, genome: Genome) -> FormulaId {
        FormulaId(self.arena.insert(genome))
    }

    pub fn get(&self, id: FormulaId) -> Option<&Genome> {
        self.arena.get(id.0)
    }

    pub fn get_mut(&mut self, id: FormulaId) -> Option<&mut Genome> {
        self.arena.get_mut(id.0)
    }

    pub fn len(&self) -> usize {
        self.arena.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Bản sao toàn bộ genome theo thứ tự slot (dùng cho checkpoint).
    pub fn genomes_in_order(&self) -> Vec<Genome> {
        self.arena.iter().map(|(_, g)| g.clone()).collect()
    }

    /// Tái tạo registry từ danh sách theo thứ tự slot (dùng cho checkpoint).
    ///
    /// Bất biến: `genomes[i]` phải ứng slot i — chỉ đúng khi danh sách đến
    /// từ `genomes_in_order` của registry chưa từng remove.
    pub fn from_genomes_in_order(genomes: Vec<Genome>) -> Self {
        let mut reg = Self::new();
        for g in genomes {
            reg.insert(g);
        }
        reg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome(f: LtlFormula) -> Genome {
        Genome {
            formula: f,
            fitness: None,
        }
    }

    #[test]
    fn insert_get_round_trip() {
        let mut reg = FormulaRegistry::new();
        let g = genome(LtlFormula::atom("res"));
        let id = reg.insert(g.clone());
        assert_eq!(reg.get(id), Some(&g));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn shared_gene_between_atoms_is_same_id() {
        let mut reg = FormulaRegistry::new();
        let id = reg.insert(genome(LtlFormula::atom("open")));
        // Hai "atom" cùng trỏ một id — đọc ra cùng genome, không nhân bản.
        let a = reg.get(id).unwrap();
        let b = reg.get(id).unwrap();
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn get_mut_updates_in_place() {
        let mut reg = FormulaRegistry::new();
        let id = reg.insert(genome(LtlFormula::atom("old")));
        reg.get_mut(id).unwrap().formula = LtlFormula::atom("new");
        assert_eq!(reg.get(id).unwrap().formula, LtlFormula::atom("new"));
    }

    #[test]
    fn formula_id_serializes_as_slot_index() {
        // u32 slot qua serde (JSON đại diện cho mọi format).
        let id = FormulaId::from_slot(7);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "7");
        let back: FormulaId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn order_preserved_for_checkpoint_round_trip() {
        let mut reg = FormulaRegistry::new();
        let formulas = [
            LtlFormula::atom("a"),
            LtlFormula::and(LtlFormula::atom("b"), LtlFormula::atom("c")),
            LtlFormula::g(LtlFormula::atom("d")),
        ];
        for f in &formulas {
            reg.insert(genome(f.clone()));
        }
        let dumped = reg.genomes_in_order();
        assert_eq!(dumped.len(), 3);
        let rebuilt = FormulaRegistry::from_genomes_in_order(dumped);
        assert_eq!(rebuilt.genomes_in_order(), reg.genomes_in_order());
        // Handle cũ vẫn hợp lệ trên registry dựng lại (slot khớp).
        let id = FormulaId::from_slot(1);
        assert_eq!(
            rebuilt.get(id).unwrap().formula,
            LtlFormula::and(LtlFormula::atom("b"), LtlFormula::atom("c"))
        );
    }

    #[test]
    fn slot_round_trip() {
        let mut reg = FormulaRegistry::new();
        let id = reg.insert(genome(LtlFormula::atom("x")));
        assert_eq!(FormulaId::slot(id), 0);
        let id2 = reg.insert(genome(LtlFormula::atom("y")));
        assert_eq!(FormulaId::slot(id2), 1);
        assert_eq!(FormulaId::from_slot(1), id2);
    }
}
