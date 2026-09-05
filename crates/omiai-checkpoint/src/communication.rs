//! `Checkpointable` implementations for communication types: `Vocabulary`,
//! `BenefitCounters`, `ConventionTracker`, `PromotedConvention`.
//!
//! These are simple CBOR-serializable structs, so the implementations are
//! straightforward wrappers around `ciborium`.

use std::path::Path;

use omiai_world::communication::{
    Vocabulary, BenefitCounters, ConventionTracker, PromotedConvention,
    N_SYMBOLS, N_STATE_CLASSES, N_SIGNAL_VALUES, Symbol, StateClass, SignalValue
};
use serde::{Deserialize, Serialize};

use crate::error::CheckpointError;
use crate::fsutil::{write_atomic, read_file};
use crate::traits::Checkpointable;
use crate::world_bundle::{VOCABULARY_FILE, CONVENTIONS_FILE, COMM_DIR};

impl Checkpointable for Vocabulary {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let world_dir = dir.join("world");
        std::fs::create_dir_all(&world_dir).map_err(|e| CheckpointError::Io {
            path: world_dir.clone(),
            source: e,
        })?;
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(&world_dir, VOCABULARY_FILE, &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let world_dir = dir.join("world");
        let bytes = read_file(&world_dir.join(VOCABULARY_FILE))?;
        ciborium::de::from_reader(&bytes[..]).map_err(|e| CheckpointError::Cbor(e.to_string()))
    }
}

impl Checkpointable for BenefitCounters {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let comm_dir = dir.join(COMM_DIR);
        std::fs::create_dir_all(&comm_dir).map_err(|e| CheckpointError::Io {
            path: comm_dir.clone(),
            source: e,
        })?;
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(&comm_dir, "benefit.cbor", &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let comm_dir = dir.join(COMM_DIR);
        let bytes = read_file(&comm_dir.join("benefit.cbor"))?;
        ciborium::de::from_reader(&bytes[..]).map_err(|e| CheckpointError::Cbor(e.to_string()))
    }
}

impl Checkpointable for PromotedConvention {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let comm_dir = dir.join(COMM_DIR);
        std::fs::create_dir_all(&comm_dir).map_err(|e| CheckpointError::Io {
            path: comm_dir.clone(),
            source: e,
        })?;
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(&comm_dir, "promoted_convention.cbor", &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let comm_dir = dir.join(COMM_DIR);
        let bytes = read_file(&comm_dir.join("promoted_convention.cbor"))?;
        ciborium::de::from_reader(&bytes[..]).map_err(|e| CheckpointError::Cbor(e.to_string()))
    }
}

impl Checkpointable for ConventionTracker {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let comm_dir = dir.join(COMM_DIR);
        std::fs::create_dir_all(&comm_dir).map_err(|e| CheckpointError::Io {
            path: comm_dir.clone(),
            source: e,
        })?;
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(&comm_dir, CONVENTIONS_FILE, &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let comm_dir = dir.join(COMM_DIR);
        let bytes = read_file(&comm_dir.join(CONVENTIONS_FILE))?;
        ciborium::de::from_reader(&bytes[..]).map_err(|e| CheckpointError::Cbor(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omiai_world::communication::{SignalValue, StateClass};
    use tempfile::tempdir;

    #[test]
    fn vocabulary_roundtrip() {
        let dir = tempdir().unwrap();
        let mut vocab = Vocabulary::default();
        vocab.record(SignalValue::Sym(0), StateClass::North);
        vocab.record(SignalValue::Sym(1), StateClass::East);
        vocab.record(SignalValue::Silent, StateClass::None);

        vocab.save(dir.path()).unwrap();
        let loaded = Vocabulary::load(dir.path()).unwrap();
        assert_eq!(loaded.joint, vocab.joint);
        assert_eq!(loaded.total, vocab.total);
    }

    #[test]
    fn benefit_counters_roundtrip() {
        let dir = tempdir().unwrap();
        let mut benefit = BenefitCounters::default();
        benefit.record(&[true, false, false, false], true);
        benefit.record(&[false, true, false, false], false);
        benefit.record(&[false, false, false, false], true);

        benefit.save(dir.path()).unwrap();
        let loaded = BenefitCounters::load(dir.path()).unwrap();
        assert_eq!(loaded.heard_steps, benefit.heard_steps);
        assert_eq!(loaded.heard_feeds, benefit.heard_feeds);
        assert_eq!(loaded.quiet_steps, benefit.quiet_steps);
        assert_eq!(loaded.quiet_feeds, benefit.quiet_feeds);
    }

    #[test]
    fn convention_tracker_roundtrip() {
        let dir = tempdir().unwrap();
        let mut tracker = ConventionTracker::default();
        // Add a promoted convention directly to the promoted vec
        tracker.promoted.push(PromotedConvention {
            symbol: 0,
            meaning_col: StateClass::North as u8,
            epoch: 0,
            streak: 1,
            precision_hits: 1,
            precision_total: 2,
            heard_steps: 1,
            heard_feeds: 1,
            quiet_steps: 0,
            quiet_feeds: 0,
        });

        tracker.save(dir.path()).unwrap();
        let loaded = ConventionTracker::load(dir.path()).unwrap();
        assert_eq!(loaded.promoted.len(), 1);
        assert_eq!(loaded.promoted[0].symbol, 0);
    }
}