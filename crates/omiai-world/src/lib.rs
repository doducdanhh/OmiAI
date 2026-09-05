//! Simulated living world (artificial life / open-ended evolution).
//!
//! # Modules
//! - [`substrate`] — cellular-automata grid (moved from `neuro::cellular`,
//!   ADR-0002) with pluggable `Rule` trait to come in a later slice.
//! - [`atoms`] — energy + lattice coordinate + gene pointing at a `Formula`.
//! - [`agents`] — policies decoded from evolved Formulae.
//! - [`communication`] — Lewis signaling-game emergent vocabulary: symbol
//!   alphabet, voice-gene decoding, mutual-information measurement.
//! - [`ecology`] — metabolism/feeding/reproduction constants and rules.
//! - [`registry`] — generational arena of genome Formulae.
//! - [`world_loop`] — central step loop over the fixed phase order.

#![allow(dead_code)]

pub mod agents;
pub mod atoms;
pub mod communication;
pub mod ecology;
pub mod registry;
pub mod substrate;
pub mod world_loop;

pub use world_loop::World;
pub use world_loop::WorldConfig;
pub use substrate::CellularAutomaton;
