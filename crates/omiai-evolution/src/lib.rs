//! Evolutionary computation: genetic programming over CGP genomes,
//! selection strategies, crossover and mutation operators, fitness.
//!
//! # Modules
//! - [`genetic_programming`] — Cartesian Genetic Programming islands.
//! - [`genetic`] — classic GA loop.
//! - [`crossover`], [`mutation`], [`selection`], [`fitness`].
//! - [`formula_gp`] — Genetic Programming trực tiếp trên cây cú pháp Formula (logic AST).
//!
//! Slice-2+ note: extended to evolve logic `Formula` trees directly.

#![allow(dead_code)]

pub mod crossover;
pub mod fitness;
pub mod formula_gp;
pub mod genetic;
pub mod genetic_programming;
pub mod ltl_formula_gp;
pub mod mutation;
pub mod selection;
