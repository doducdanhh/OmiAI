//! Memory systems for a reasoning agent.
//!
//! # Modules
//! - [`episodic`] — time-stamped episodes of interaction.
//! - [`semantic`] — long-term semantic facts.
//! - [`working`] — bounded working memory.
//! - [`procedural`] — skills/procedures.

#![allow(dead_code)]

pub mod episodic;
pub mod procedural;
pub mod semantic;
pub mod working;
