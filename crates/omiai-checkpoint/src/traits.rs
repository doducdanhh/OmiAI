//! The `Checkpointable` trait: save/load a self-contained checkpoint-v1
//! directory. Implementations live next to the types they persist
//! (orphan-rule friendly) — e.g. `CellularAutomaton`'s impl lives in
//! `omiai_checkpoint`, not `omiai_world`.

use std::path::Path;

pub trait Checkpointable: Sized {
    type Error;
    /// Persist this object as a checkpoint directory.
    fn save(&self, dir: &Path) -> Result<(), Self::Error>;
    /// Reconstruct from a checkpoint directory, verifying integrity.
    fn load(dir: &Path) -> Result<Self, Self::Error>;
}
