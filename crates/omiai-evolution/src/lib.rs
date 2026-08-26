//! Evolutionary computation: genetic programming over CGP genomes,
//! selection strategies, crossover and mutation operators, fitness.
//!
//! # Modules
//! - [`genetic_programming`] — Cartesian Genetic Programming islands.
//! - [`genetic`] — classic GA loop.
//! - [`crossover`], [`mutation`], [`selection`], [`fitness`].
//!
//! Slice-2+ note: will be extended to evolve logic `Formula` trees directly.

#![allow(dead_code)]

pub mod crossover;
pub mod fitness;
pub mod genetic;
pub mod genetic_programming;
pub mod mutation;
pub mod selection;
