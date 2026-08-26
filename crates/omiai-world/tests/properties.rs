//! Property tests cho world loop (slice 2).
//!
//! Bất biến kiểm chứng:
//! 1. Sau mọi số bước: mọi atom trong biên lưới, energy hữu hạn trong
//!    [0, ENERGY_MAX], gene hợp lệ trong registry.
//! 2. CA population-sum (tổng giá trị ô) bảo toàn qua ca_step riêng lẻ
//!    (rotate_block hoán vị giá trị trong block).

use omiai_world::ecology::ENERGY_MAX;
use omiai_world::world_loop::{World, WorldConfig};
use proptest::prelude::*;

fn config_for(w: u16, h: u16) -> WorldConfig {
    WorldConfig {
        width: w as usize,
        height: h as usize,
        n_initial_atoms: 2,
        initial_resources: 0.05,
    }
}

proptest! {
    // seed != 0: xorshift trong substrate::random có fixed-point tại 0
    // (không dùng random() ở đây, nhưng giữ quy ước seed dương cho nhất quán).
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn world_invariants_hold(
        w in 6u16..24,
        h in 6u16..24,
        seed in 1u64..1000,
        steps in 0u8..12,
    ) {
        let mut world = World::new(config_for(w, h), seed);
        for _ in 0..steps {
            world.step();
        }
        let occupied: std::collections::BTreeSet<_> =
            world.atoms.iter().map(|a| a.pos).collect();
        prop_assert_eq!(occupied.len(), world.atoms.len(), "hai atom cùng ô");

        for atom in &world.atoms {
            prop_assert!(atom.pos.0 < w as usize && atom.pos.1 < h as usize,
                "atom ngoài lưới: {:?}", atom.pos);
            prop_assert!(atom.energy.is_finite());
            prop_assert!((0.0..=ENERGY_MAX).contains(&atom.energy),
                "energy ngoài khoảng: {}", atom.energy);
            prop_assert!(world.registry.get(atom.gene).is_some(), "gene mất tích");
        }
        prop_assert_eq!(world.step_count, steps as u64);
    }

    #[test]
    fn ca_step_preserves_cell_value_sum(
        w in 4u16..20,
        h in 4u16..20,
        seed in 1u64..1000,
    ) {
        // Lưới đa trạng thái như world dùng (num_states=4).
        let mut world = World::new(config_for(w, h), seed);
        let sum_before: u64 = world.ca.cells.iter().map(|&c| c as u64).sum();
        world.ca_step();
        let sum_after: u64 = world.ca.cells.iter().map(|&c| c as u64).sum();
        prop_assert_eq!(sum_before, sum_after,
            "rotate_block phải bảo toàn tổng giá trị ô (hoán vị)");
    }
}
