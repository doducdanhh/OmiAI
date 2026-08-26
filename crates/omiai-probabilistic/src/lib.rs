//! Probabilistic reasoning: exact and approximate inference.
//!
//! # Modules
//! - [`bayesian`] — Bayesian networks with CPTs, variable elimination.
//! - [`gibbs`] — Gibbs sampling.
//! - [`hmc`] — Hamiltonian Monte Carlo.
//! - [`junction_tree`] — exact inference via junction tree algorithm.
//! - [`markov`] — Markov chains.
//! - [`mean_field`] — variational mean-field.
//! - [`mcts`] — Monte Carlo Tree Search over [`mcts::GameState`].
//! - [`puct_mcts`] — PUCT (AlphaZero-style) search.
//! - [`kolmogorov`] — algorithmic probability utilities.
//! - [`solomonoff`] — Solomonoff induction sketch.

#![allow(dead_code)]

pub mod bayesian;
pub mod gibbs;
pub mod hmc;
pub mod junction_tree;
pub mod kolmogorov;
pub mod markov;
pub mod mean_field;
pub mod mcts;
pub mod puct_mcts;
pub mod solomonoff;
