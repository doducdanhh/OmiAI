//! OmiAI — a zero-training, self-bootstrapping reasoning system built on
//! symbolic AI, knowledge graphs, evolutionary computation, reservoir
//! computing, cellular automata, causal inference, active inference, and
//! neuro-symbolic search.
//!
//! No deep learning, no GPU, no training datasets: every module is either
//! a proven algorithm from theoretical computer science or an explicit,
//! inspectable data structure.
//!
//! # Eight pillars
//! 1. **Symbolic AI** — [`core`] (logic, unification, DPLL/CDCL, resolution)
//! 2. **Knowledge graphs** — [`knowledge`] (petgraph, ontology, SPARQL-like)
//! 3. **Evolution** — [`evolution`] (GA, CGP, island model)
//! 4. **Reservoir / LSM** — [`neuro`] (ESN + RLS, liquid state, CA)
//! 5. **Cellular automata** — [`neuro::cellular`]
//! 6. **Causal reasoning** — [`causal`] (SCM, do-calculus)
//! 7. **Active inference** — [`meta::self_improvement`]
//! 8. **Neuro-symbolic search** — [`probabilistic::mcts`] + logical solvers
//!
//! # Example
//! ```
//! use omiai::core::logic_engine::Formula;
//! use omiai::core::prover::TheoremProver;
//! use omiai::core::inference::ProofResult;
//!
//! let p = Formula::prop("P");
//! let q = Formula::prop("Q");
//! let imp = Formula::Implies(Box::new(p.clone()), Box::new(q.clone()));
//! let result = TheoremProver::new().prove(&[imp, p], &q);
//! assert!(matches!(result, ProofResult::Proved { .. }));
//! ```

#![allow(dead_code)]

pub mod causal;
pub mod core;
pub mod evolution;
pub mod io;
pub mod knowledge;
pub mod memory;
pub mod meta;
pub mod neuro;
pub mod persistence;
pub mod probabilistic;
pub mod utils;

pub use crate::io::chat::{ChatEngine, ChatRequest, ChatResponse};
pub use crate::io::nlp_parser::{DetectedLanguage, NlpParser, ParseIntent};
pub use crate::meta::introspection::ConversationMemory;

pub use crate::core::inference::ProofResult;
pub use crate::core::logic_engine::{Formula, Literal, LogicError, Term};
pub use crate::core::prover::TheoremProver;
