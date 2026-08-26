//! Metacognition: introspection over proofs, Active Inference
//! self-improvement loop, hierarchical goal systems, autopoiesis.
//!
//! # Modules
//! - [`introspection`] — compact proof explanations.
//! - [`self_improvement`] — the Active Inference meta-engine.
//! - [`autopoiesis`] — self-production loop coupling GP + knowledge graph.
//! - [`goal_system`] — hierarchical goals.
//! - [`continual_learning`] — accumulate knowledge across sessions.

#![allow(dead_code)]

pub mod autopoiesis;
pub mod continual_learning;
pub mod goal_system;
pub mod introspection;
pub mod self_improvement;
