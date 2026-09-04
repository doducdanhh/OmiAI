//! `Checkpointable` implementations for communication types: `Vocabulary`,
//! `BenefitCounters`, `ConventionTracker`, `PromotedConvention`.
//!
//! These are simple CBOR-serializable structs, so the implementations are
//! straightforward wrappers around `ciborium`.

use std::path::Path;

use omiai_world::communication::{BenefitCounters, ConventionTracker, PromotedConvention, Vocabulary};

use crate::error::CheckpointError;
use crate::fsutil::write_atomic;
use crate::traits::Checkpointable;

const VOCAB_FILE: &str = "vocabulary.cbor";
const BENEFIT_FILE: &str = "benefit.cbor";
const CONVENTIONS_FILE: &str = "conventions.cbor";

impl Checkpointable for Vocabulary {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(dir, VOCAB_FILE, &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let bytes = std::fs::read(dir.join(VOCAB_FILE)).map_err(|source| CheckpointError::Io {
            path: dir.join(VOCAB_FILE),
            source,
        })?;
        ciborium::de::from_reader(&bytes[..]).map_err(|e| CheckpointError::Cbor(e.to_string()))
    }
}

impl Checkpointable for BenefitCounters {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(dir, BENEFIT_FILE, &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let bytes = std::fs::read(dir.join(BENEFIT_FILE)).map_err(|source| CheckpointError::Io {
            path: dir.join(BENEFIT_FILE),
            source,
        })?;
        ciborium::de::from_reader(&bytes[..]).map_err(|e| CheckpointError::Cbor(e.to_string()))
    }
}

impl Checkpointable for PromotedConvention {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(dir, CONVENTIONS_FILE, &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let bytes = std::fs::read(dir.join(CONVENTIONS_FILE)).map_err(|source| CheckpointError::Io {
            path: dir.join(CONVENTIONS_FILE),
            source,
        })?;
        ciborium::de::from_reader(&bytes[..]).map_err(|e| CheckpointError::Cbor(e.to_string()))
    }
}

impl Checkpointable for ConventionTracker {
    type Error = CheckpointError;

    fn save(&self, dir: &Path) -> Result<(), CheckpointError> {
        let mut buf = std::io::Cursor::new(Vec::new());
        ciborium::ser::into_writer(self, &mut buf).map_err(|e| CheckpointError::Cbor(e.to_string()))?;
        write_atomic(dir, CONVENTIONS_FILE, &buf.into_inner())?;
        Ok(())
    }

    fn load(dir: &Path) -> Result<Self, CheckpointError> {
        let bytes = std::fs::read(dir.join(CONVENTIONS_FILE)).map_err(|source| CheckpointError::Io {
            path: dir.join(CONVENTIONS_FILE),
            source,
        })?;
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
        tracker.epoch_index = 5;
        tracker.steps_in_epoch = 32;
        tracker.record_signal(SignalValue::Sym(0), StateClass::North);
        tracker.record_benefit(&[true, false, false, false], true);

        tracker.save(dir.path()).unwrap();
        let loaded = ConventionTracker::load(dir.path()).unwrap();
        assert_eq!(loaded.epoch_index, tracker.epoch_index);
        assert_eq!(loaded.steps_in_epoch, tracker.steps_in_epoch);
        assert_eq!(loaded.epoch_vocab.joint, tracker.epoch_vocab.joint);
        assert_eq!(loaded.benefit.heard_steps, tracker.benefit.heard_steps);
    }
}