//! Simulated living world (artificial life / open-ended evolution).
//!
//! # Modules
//! - [`substrate`] — cellular-automata grid (moved from `neuro::cellular`,
//!   ADR-0002) with pluggable `Rule` trait to come in a later slice.
//!
//! Later slices add: `atoms` (energy + lattice coordinate + gene pointing
//! at a `Formula`), `agents` (policies as evolved Formulae), `communication`
//! (Lewis signaling-game emergent vocabulary), `world_loop` (central step
//! loop: CA step → energy/fitness → evolution → knowledge → beliefs).

#![allow(dead_code)]

pub mod atoms;
pub mod ecology;
pub mod registry;
pub mod substrate;
