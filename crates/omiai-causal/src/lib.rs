//! Causal reasoning: structural causal models, Pearl's do-calculus,
//! interventions, confounding detection, Invariant Causal Prediction.
//!
//! # Modules
//! - [`dag`] — DAG utilities.
//! - [`scm`] — structural causal model simulation.
//! - [`do_calculus`] — do-operator derivations.
//! - [`intervention`] — graph surgery under interventions.
//! - [`confounding`] — backdoor/frontdoor helpers.
//! - [`icp`] — invariant causal prediction.

#![allow(dead_code)]

pub mod confounding;
pub mod dag;
pub mod do_calculus;
pub mod icp;
pub mod intervention;
pub mod scm;
pub mod utils_stats;
