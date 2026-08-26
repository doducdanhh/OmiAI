//! Minimal load-and-infer runtime for exported `model.omiai` bundles:
//! `load(path)` + `step(input) -> output`, nothing else.
//!
//! Constraint: this crate must NEVER depend on training/evolution code,
//! so it can compile to native lib, cdylib (FFI), wasm32-wasi, and
//! wasm32-unknown-unknown targets.
//!
//! **Scaffold** — implemented in the final slice.
#![allow(dead_code)]
