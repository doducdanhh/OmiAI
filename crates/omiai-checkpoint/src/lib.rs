//! Versioned directory-format checkpoints for long-running training/
//! evolution sessions.
//!
//! Design (spec §2, docs/format-spec/checkpoint-v1.md): each checkpoint is
//! a *directory* `step_XXXXXXXX/` with a `manifest.json` (format version,
//! git commit, step, timestamp, full RNG state, per-file BLAKE3 hashes).
//! Writes are atomic: tmp dir → fsync files → rename. A sliding window
//! keeps the N most recent plus permanent milestones every K steps.
//!
//! This slice ships the [`Checkpointable`] trait, atomic-write/hash
//! helpers, and the first round-trip implementation for the world CA grid.

pub mod legacy;

pub use legacy::*;
