//! Inference bundle packaging (`model.omiai`): a single tar+zstd archive
//! with a versioned manifest declaring schema version, present pillars,
//! and entry-point signatures.
//!
//! **Scaffold** — implemented in a later slice, after all pillars above it
//! are stable and benchmarked (spec: bundle/runtime là bước cuối).
#![allow(dead_code)]
