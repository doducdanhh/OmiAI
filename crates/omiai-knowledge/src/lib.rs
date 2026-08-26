//! Knowledge graphs & ontologies.
//!
//! # Modules
//! - [`graph`] — concept/relation graph over petgraph.
//! - [`reasoning`] — forward/backward chaining.
//! - [`abduction`] — minimal-explanation search on top of core DPLL.
//! - [`ontology`] — class hierarchy utilities.
//! - [`triple`] — RDF-style triple store.
//! - [`discocat`] — DisCoCat / pregroup grammar reduction.
//! - [`sparql_like`] — small SPARQL-like query engine.

#![allow(dead_code)]

use omiai_core as core;

pub mod abduction;
pub mod discocat;
pub mod graph;
pub mod ontology;
pub mod reasoning;
pub mod sparql_like;
pub mod triple;
