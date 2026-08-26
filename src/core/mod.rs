//! Core reasoning engine: symbolic logic, automated theorem proving,
//! constraint satisfaction, and answer-set programming.
//!
//! # Modules
//! - [`logic_engine`] — propositional & first-order `Formula`/`Term` AST,
//!   CNF normalization (NNF → Skolemization → clause form), evaluation.
//! - [`substitution`] — variable substitutions over terms and formulas.
//! - [`unification`] — Robinson first-order unification with occurs check.
//! - [`higher_order_unification`] — Huet higher-order unification (typed λ).
//! - [`ltl`] — Linear Temporal Logic (LTL) with tableau satisfiability.
//! - [`modal`] — Modal logic K (Kripke semantics, model checking, validity).
//! - [`inference`] — Resolution, DPLL, CDCL.
//! - [`prover`] — [`prover::TheoremProver`] and minimal DPLL(T).
//! - [`csp_solver`] — AC-3 + backtracking with forward checking.
//! - [`asp_solver`] — Answer Set Programming (stable models).

pub mod asp_solver;
pub mod csp_solver;
pub mod higher_order_unification;
pub mod inference;
pub mod logic_engine;
pub mod ltl;
pub mod modal;
pub mod prover;
pub mod substitution;
pub mod unification;
