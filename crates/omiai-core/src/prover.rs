//! SAT solving (CDCL / DPLL via [`super::inference`]) and a high-level
//! [`TheoremProver`] that normalizes premises + negated goal to CNF and
//! runs resolution refutation (first-order) or CDCL (propositional).
//!
//! # SMT sketch
//! [`dpll_t_satisfiable`] implements a minimal DPLL(T) loop for linear
//! arithmetic equalities/inequalities over `f64`, used as a theory layer
//! on top of the boolean skeleton.

use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use super::inference::{self, ProofResult};
use super::logic_engine::{self, Formula, Literal};

/// Timed proof report for CLI / introspection.
#[derive(Debug, Clone)]
pub struct ProofReport {
    pub result: ProofResult,
    pub elapsed_ms: f64,
    pub clause_count: usize,
    pub signature_size: usize,
}

/// Public entry point: premises + goal → proof by contradiction.
#[derive(Debug, Default, Clone)]
pub struct TheoremProver {
    /// Max clauses generated during resolution before giving up.
    pub max_clauses: usize,
}

impl TheoremProver {
    pub fn new() -> Self {
        Self {
            max_clauses: 10_000,
        }
    }

    /// Raise the clause budget for harder proof searches.
    pub fn with_max_clauses(mut self, max_clauses: usize) -> Self {
        self.max_clauses = max_clauses;
        self
    }

    /// Negate the goal, conjoin with premises, CNF-normalize, and refute.
    ///
    /// # Example
    /// ```
    /// use omiai_core::logic_engine::Formula;
    /// use omiai_core::prover::TheoremProver;
    /// use omiai_core::inference::ProofResult;
    ///
    /// let p = Formula::prop("P");
    /// let q = Formula::prop("Q");
    /// let imp = Formula::Implies(Box::new(p.clone()), Box::new(q.clone()));
    /// let prover = TheoremProver::new();
    /// let r = prover.prove(&[imp, p], &q);
    /// assert!(matches!(r, ProofResult::Proved { .. }));
    /// ```
    pub fn prove(&self, premises: &[Formula], goal: &Formula) -> ProofResult {
        self.prove_timed(premises, goal).result
    }

    /// Same as [`Self::prove`] but returns timing and clause statistics.
    pub fn prove_timed(&self, premises: &[Formula], goal: &Formula) -> ProofReport {
        let start = Instant::now();
        let mut clauses: Vec<Vec<Literal>> = Vec::new();

        for prem in premises {
            match logic_engine::normalize_cnf(prem) {
                Ok(cs) => clauses.extend(cs),
                Err(_) => {
                    return ProofReport {
                        result: ProofResult::Unknown,
                        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                        clause_count: 0,
                        signature_size: 0,
                    };
                }
            }
        }

        let neg_goal = Formula::Not(Box::new(goal.clone()));
        match logic_engine::normalize_cnf(&neg_goal) {
            Ok(cs) => clauses.extend(cs),
            Err(_) => {
                return ProofReport {
                    result: ProofResult::Unknown,
                    elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                    clause_count: clauses.len(),
                    signature_size: signature_size(&clauses),
                };
            }
        }

        let clause_count = clauses.len();
        let signature_size = signature_size(&clauses);
        let result = inference::resolution_refute_bounded(&clauses, self.max_clauses);
        ProofReport {
            result,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
            clause_count,
            signature_size,
        }
    }
}

fn signature_size(clauses: &[Vec<Literal>]) -> usize {
    clauses
        .iter()
        .flat_map(|clause| clause.iter().map(|literal| literal.predicate.as_str()))
        .collect::<BTreeSet<_>>()
        .len()
}

/// Theory atom for the `T` in DPLL(T).
#[derive(Debug, Clone)]
pub enum TheoryLiteral {
    /// Pure boolean literal (skeleton).
    Boolean(Literal),
    /// Linear constraint: `var op value` where op ∈ {=, <, >, <=, >=}.
    LinearArithmetic { var: String, op: ArithOp, rhs: f64 },
}

/// Comparison operators for linear-arithmetic theory literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Default, Clone)]
struct TheoryBounds {
    lo: HashMap<String, f64>,
    hi: HashMap<String, f64>,
    eq: HashMap<String, f64>,
}

impl TheoryBounds {
    fn push(&mut self, var: String, op: ArithOp, rhs: f64) {
        match op {
            ArithOp::Eq => {
                self.eq.insert(var, rhs);
            }
            ArithOp::Lt => {
                let entry = self.hi.entry(var).or_insert(f64::INFINITY);
                *entry = entry.min(rhs - f64::EPSILON);
            }
            ArithOp::Le => {
                let entry = self.hi.entry(var).or_insert(f64::INFINITY);
                *entry = entry.min(rhs);
            }
            ArithOp::Gt => {
                let entry = self.lo.entry(var).or_insert(f64::NEG_INFINITY);
                *entry = entry.max(rhs + f64::EPSILON);
            }
            ArithOp::Ge => {
                let entry = self.lo.entry(var).or_insert(f64::NEG_INFINITY);
                *entry = entry.max(rhs);
            }
        }
    }

    fn is_consistent(&self) -> bool {
        let vars: BTreeSet<String> = self
            .lo
            .keys()
            .chain(self.hi.keys())
            .chain(self.eq.keys())
            .cloned()
            .collect();
        for var in vars {
            if let Some(&e) = self.eq.get(&var) {
                if let Some(&l) = self.lo.get(&var)
                    && e < l - f64::EPSILON {
                        return false;
                    }
                if let Some(&h) = self.hi.get(&var)
                    && e > h + f64::EPSILON {
                        return false;
                    }
            }
            if let (Some(&l), Some(&h)) = (self.lo.get(&var), self.hi.get(&var))
                && l > h + f64::EPSILON {
                    return false;
                }
        }
        true
    }
}

/// Minimal DPLL(T): SAT over the boolean skeleton, then check arithmetic
/// consistency of the forced theory literals under a simple bound store.
///
/// Full Nelson–Oppen / CDCL(T) is out of scope; this covers ground LRA
/// unit constraints used by causal / planning layers.
pub fn dpll_t_satisfiable(literals: &[TheoryLiteral]) -> bool {
    let mut bool_clauses: Vec<Vec<Literal>> = Vec::new();
    let mut bounds = TheoryBounds::default();

    for lit in literals {
        match lit {
            TheoryLiteral::Boolean(l) => bool_clauses.push(vec![l.clone()]),
            TheoryLiteral::LinearArithmetic { var, op, rhs } => bounds.push(var.clone(), *op, *rhs),
        }
    }

    if !bool_clauses.is_empty() && !inference::dpll_satisfiable(&bool_clauses) {
        return false;
    }

    bounds.is_consistent()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic_engine::Term;

    #[test]
    fn prove_modus_ponens() {
        let p = Formula::prop("P");
        let q = Formula::prop("Q");
        let imp = Formula::Implies(Box::new(p.clone()), Box::new(q.clone()));
        let prover = TheoremProver::new();
        let report = prover.prove_timed(&[imp, p], &q);
        assert!(matches!(report.result, ProofResult::Proved { .. }));
        assert!(report.signature_size > 0);
    }

    #[test]
    fn prove_socrates() {
        let human_x = Formula::atom("Human", vec![Term::Var("x".into())]);
        let mortal_x = Formula::atom("Mortal", vec![Term::Var("x".into())]);
        let rule = Formula::ForAll(
            "x".into(),
            Box::new(Formula::Implies(Box::new(human_x), Box::new(mortal_x))),
        );
        let fact = Formula::atom("Human", vec![Term::Const("socrates".into())]);
        let goal = Formula::atom("Mortal", vec![Term::Const("socrates".into())]);
        let prover = TheoremProver::new();
        let report = prover.prove_timed(&[rule, fact], &goal);
        assert!(
            matches!(report.result, ProofResult::Proved { .. }),
            "expected proved, got {:?}",
            report.result
        );
    }

    #[test]
    fn dpll_t_consistent_bounds() {
        let lits = vec![
            TheoryLiteral::LinearArithmetic {
                var: "x".into(),
                op: ArithOp::Ge,
                rhs: 0.0,
            },
            TheoryLiteral::LinearArithmetic {
                var: "x".into(),
                op: ArithOp::Le,
                rhs: 1.0,
            },
        ];
        assert!(dpll_t_satisfiable(&lits));
    }

    #[test]
    fn dpll_t_inconsistent_bounds() {
        let lits = vec![
            TheoryLiteral::LinearArithmetic {
                var: "x".into(),
                op: ArithOp::Ge,
                rhs: 2.0,
            },
            TheoryLiteral::LinearArithmetic {
                var: "x".into(),
                op: ArithOp::Le,
                rhs: 1.0,
            },
        ];
        assert!(!dpll_t_satisfiable(&lits));
    }
}
