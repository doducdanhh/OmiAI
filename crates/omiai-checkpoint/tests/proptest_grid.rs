//! Property tests: grid round-trip preserves state; Margolus step preserves
//! population and the round-trip preserves it too.

use omiai_checkpoint::Checkpointable;
use omiai_world::substrate::CellularAutomaton;
use proptest::prelude::*;

proptest! {
    // NOTE: seed=0 is a fixed point of the xorshift inside
    // `CellularAutomaton::random` (every cell becomes live), so seeds are
    // drawn from 1.. to exercise real variation.
    #[test]
    fn roundtrip_preserves_cells(w in 1usize..64, h in 1usize..64, seed in 1u64..) {
        let dir = tempfile::tempdir()?;
        let ca = CellularAutomaton::random(w, h, 0.3, seed);
        let snap = ca.clone();
        ca.save(dir.path())?;
        let back = CellularAutomaton::load(dir.path())?;
        prop_assert_eq!(back.cells, snap.cells);
    }

    #[test]
    fn population_preserved_by_step_and_roundtrip(w in 2usize..32, h in 2usize..32, seed in 1u64..) {
        let dir = tempfile::tempdir()?;
        let mut ca = CellularAutomaton::random(w, h, 0.3, seed);
        let p0 = ca.population();
        ca.step();
        prop_assert_eq!(ca.population(), p0); // Margolus rotation conserves population
        ca.save(dir.path())?;
        let back = CellularAutomaton::load(dir.path())?;
        prop_assert_eq!(back.population(), p0);
    }
}
