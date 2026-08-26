//! Reservoir computing: zero-training recurrent substrates with a trained
//! linear readout only (RLS) — the natural fit for CPU-only, 8GB-RAM
//! hardware (no backprop through the network, no GPU).
//!
//! # Modules
//! - [`reservoir`] — echo-state network + RLS readout.
//! - [`liquid_state`] — liquid state machines.
//! - [`weights`] — random/sparse matrix helpers, spectral normalization.
//!
//! Note: cellular automata moved to `omiai-world::substrate` (ADR-0002).

#![allow(dead_code)]

pub mod liquid_state;
pub mod reservoir;
pub mod weights;
