//! Propositional & first-order logic: the `Formula`/`Term` AST, and a CNF
//! normalization pipeline built from five textbook-standard passes:
//!
//! 1. [`eliminate_iff_implies`] — rewrite `->` and `<->` in terms of
//!    `¬`, `∧`, `∨`.
//! 2. [`to_nnf`] — push negations inward (De Morgan, quantifier duals,
//!    double-negation elimination) to reach negation-normal form.
//! 3. [`skolemize`] — replace existentially quantified variables with
//!    Skolem functions of the enclosing universally quantified variables
//!    (or Skolem constants, if there are none).
//! 4. [`drop_universal_quantifiers`] — strip the now-implicit universal
//!    quantifiers, leaving a quantifier-free matrix.
//! 5. [`distribute_cnf`] — distribute `∨` over `∧` to reach conjunctive
//!    normal form, then flatten into a clause list.
//!
//! [`normalize_cnf`] runs all five passes and returns `Vec<Vec<Literal>>`
//! (a conjunction of clauses, each a disjunction of literals) — the
//! representation the Part 2 resolution/CDCL provers will consume.
//!
//! [`evaluate`] evaluates a *ground* (variable-free, quantifier-free)
//! formula against a truth-value assignment for its atoms. Evaluating
//! quantified formulas requires a finite domain model and belongs to
//! `knowledge::reasoning` once that lands (Part 2) — see [`LogicError`].

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::substitution::Substitution;

/// A first-order term: a variable, a constant, or a function applied to
/// other terms (which also covers Skolem functions/constants generated
/// during [`skolemize`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Term {
    Var(String),
    Const(String),
    Func(String, Vec<Term>),
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Var(v) => write!(f, "{v}"),
            Term::Const(c) => write!(f, "{c}"),
            Term::Func(name, args) => {
                write!(f, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
        }
    }
}

/// A propositional / first-order formula.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Formula {
    True,
    False,
    /// A predicate applied to terms. A zero-arity atom (`args` empty) is
    /// an ordinary propositional variable.
    Atom(String, Vec<Term>),
    Not(Box<Formula>),
    And(Box<Formula>, Box<Formula>),
    Or(Box<Formula>, Box<Formula>),
    Implies(Box<Formula>, Box<Formula>),
    Iff(Box<Formula>, Box<Formula>),
    ForAll(String, Box<Formula>),
    Exists(String, Box<Formula>),
}

impl Formula {
    pub fn atom(name: impl Into<String>, args: Vec<Term>) -> Self {
        Formula::Atom(name.into(), args)
    }

    pub fn prop(name: impl Into<String>) -> Self {
        Formula::Atom(name.into(), Vec::new())
    }
}

/// A single literal in clause form: a (possibly negated) atom.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Literal {
    pub negated: bool,
    pub predicate: String,
    pub args: Vec<Term>,
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negated {
            write!(f, "\u{00ac}")?;
        }
        write!(f, "{}(", self.predicate)?;
        for (i, a) in self.args.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{a}")?;
        }
        write!(f, ")")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicError {
    /// The formula couldn't be flattened into clause form (this indicates
    /// a bug in the normalization pipeline if it happens after
    /// `normalize_cnf`'s own steps 1-4 ran).
    NotInClauseForm,
    /// `evaluate` hit a quantifier. Ground/propositional evaluation can't
    /// handle `∀`/`∃` without a finite domain model.
    UnboundQuantifierInEvaluation,
    /// `evaluate` needed a truth value for an atom the caller didn't
    /// provide.
    UnknownAtom(String),
}

impl fmt::Display for LogicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicError::NotInClauseForm => write!(f, "formula is not in clause (CNF) form"),
            LogicError::UnboundQuantifierInEvaluation => write!(
                f,
                "evaluate() cannot handle quantifiers without a finite domain model; \
                 model-based evaluation belongs to knowledge::reasoning"
            ),
            LogicError::UnknownAtom(name) => {
                write!(f, "no truth value provided for atom `{name}`")
            }
        }
    }
}

impl std::error::Error for LogicError {}

/// Collect the free (unbound) variables of a formula.
pub fn free_variables(formula: &Formula) -> BTreeSet<String> {
    fn term_vars(t: &Term, out: &mut BTreeSet<String>) {
        match t {
            Term::Var(v) => {
                out.insert(v.clone());
            }
            Term::Const(_) => {}
            Term::Func(_, args) => {
                for a in args {
                    term_vars(a, out);
                }
            }
        }
    }

    fn go(f: &Formula, bound: &mut Vec<String>, out: &mut BTreeSet<String>) {
        match f {
            Formula::True | Formula::False => {}
            Formula::Atom(_, args) => {
                let mut vs = BTreeSet::new();
                for a in args {
                    term_vars(a, &mut vs);
                }
                for v in vs {
                    if !bound.contains(&v) {
                        out.insert(v);
                    }
                }
            }
            Formula::Not(inner) => go(inner, bound, out),
            Formula::And(a, b)
            | Formula::Or(a, b)
            | Formula::Implies(a, b)
            | Formula::Iff(a, b) => {
                go(a, bound, out);
                go(b, bound, out);
            }
            Formula::ForAll(v, body) | Formula::Exists(v, body) => {
                bound.push(v.clone());
                go(body, bound, out);
                bound.pop();
            }
        }
    }

    let mut bound = Vec::new();
    let mut out = BTreeSet::new();
    go(formula, &mut bound, &mut out);
    out
}

/// Pass 1: rewrite `a -> b` as `¬a ∨ b` and `a <-> b` as `(¬a ∨ b) ∧ (¬b ∨ a)`.
pub fn eliminate_iff_implies(f: &Formula) -> Formula {
    match f {
        Formula::True | Formula::False | Formula::Atom(..) => f.clone(),
        Formula::Not(a) => Formula::Not(Box::new(eliminate_iff_implies(a))),
        Formula::And(a, b) => Formula::And(
            Box::new(eliminate_iff_implies(a)),
            Box::new(eliminate_iff_implies(b)),
        ),
        Formula::Or(a, b) => Formula::Or(
            Box::new(eliminate_iff_implies(a)),
            Box::new(eliminate_iff_implies(b)),
        ),
        Formula::Implies(a, b) => {
            let a2 = eliminate_iff_implies(a);
            let b2 = eliminate_iff_implies(b);
            Formula::Or(Box::new(Formula::Not(Box::new(a2))), Box::new(b2))
        }
        Formula::Iff(a, b) => {
            let a2 = eliminate_iff_implies(a);
            let b2 = eliminate_iff_implies(b);
            let left = Formula::Or(
                Box::new(Formula::Not(Box::new(a2.clone()))),
                Box::new(b2.clone()),
            );
            let right = Formula::Or(Box::new(Formula::Not(Box::new(b2))), Box::new(a2));
            Formula::And(Box::new(left), Box::new(right))
        }
        Formula::ForAll(v, body) => {
            Formula::ForAll(v.clone(), Box::new(eliminate_iff_implies(body)))
        }
        Formula::Exists(v, body) => {
            Formula::Exists(v.clone(), Box::new(eliminate_iff_implies(body)))
        }
    }
}

/// Pass 2: push negations inward to reach negation-normal form (NNF).
/// Handles `Implies`/`Iff` too, so it's safe to call standalone even if
/// [`eliminate_iff_implies`] hasn't run first.
pub fn to_nnf(f: &Formula) -> Formula {
    match f {
        Formula::True | Formula::False | Formula::Atom(..) => f.clone(),
        Formula::Not(inner) => match inner.as_ref() {
            Formula::True => Formula::False,
            Formula::False => Formula::True,
            Formula::Atom(..) => f.clone(),
            Formula::Not(a) => to_nnf(a),
            Formula::And(a, b) => Formula::Or(
                Box::new(to_nnf(&Formula::Not(a.clone()))),
                Box::new(to_nnf(&Formula::Not(b.clone()))),
            ),
            Formula::Or(a, b) => Formula::And(
                Box::new(to_nnf(&Formula::Not(a.clone()))),
                Box::new(to_nnf(&Formula::Not(b.clone()))),
            ),
            Formula::Implies(a, b) => Formula::And(
                Box::new(to_nnf(a)),
                Box::new(to_nnf(&Formula::Not(b.clone()))),
            ),
            Formula::Iff(a, b) => {
                let l = Formula::And(
                    Box::new(to_nnf(a)),
                    Box::new(to_nnf(&Formula::Not(b.clone()))),
                );
                let r = Formula::And(
                    Box::new(to_nnf(&Formula::Not(a.clone()))),
                    Box::new(to_nnf(b)),
                );
                Formula::Or(Box::new(l), Box::new(r))
            }
            Formula::ForAll(v, body) => {
                Formula::Exists(v.clone(), Box::new(to_nnf(&Formula::Not(body.clone()))))
            }
            Formula::Exists(v, body) => {
                Formula::ForAll(v.clone(), Box::new(to_nnf(&Formula::Not(body.clone()))))
            }
        },
        Formula::And(a, b) => Formula::And(Box::new(to_nnf(a)), Box::new(to_nnf(b))),
        Formula::Or(a, b) => Formula::Or(Box::new(to_nnf(a)), Box::new(to_nnf(b))),
        Formula::Implies(a, b) => to_nnf(&eliminate_iff_implies(&Formula::Implies(
            a.clone(),
            b.clone(),
        ))),
        Formula::Iff(a, b) => to_nnf(&eliminate_iff_implies(&Formula::Iff(a.clone(), b.clone()))),
        Formula::ForAll(v, body) => Formula::ForAll(v.clone(), Box::new(to_nnf(body))),
        Formula::Exists(v, body) => Formula::Exists(v.clone(), Box::new(to_nnf(body))),
    }
}

/// Generates fresh Skolem function/constant names for [`skolemize`].
struct SkolemContext {
    counter: usize,
}

impl SkolemContext {
    fn new() -> Self {
        Self { counter: 0 }
    }

    fn fresh_name(&mut self) -> String {
        self.counter += 1;
        format!("sk{}", self.counter)
    }
}

/// Pass 3: replace each existentially quantified variable with a Skolem
/// term — a fresh function of the universally quantified variables
/// currently in scope, or a fresh constant if none are in scope. Expects
/// (but does not require) NNF input, since NNF guarantees all quantifiers
/// have positive polarity.
pub fn skolemize(f: &Formula) -> Formula {
    let mut ctx = SkolemContext::new();
    let mut universal_scope: Vec<String> = Vec::new();
    skolemize_rec(f, &mut universal_scope, &mut ctx)
}

fn skolemize_rec(
    f: &Formula,
    universal_scope: &mut Vec<String>,
    ctx: &mut SkolemContext,
) -> Formula {
    match f {
        Formula::True | Formula::False | Formula::Atom(..) => f.clone(),
        Formula::Not(inner) => Formula::Not(Box::new(skolemize_rec(inner, universal_scope, ctx))),
        Formula::And(a, b) => Formula::And(
            Box::new(skolemize_rec(a, universal_scope, ctx)),
            Box::new(skolemize_rec(b, universal_scope, ctx)),
        ),
        Formula::Or(a, b) => Formula::Or(
            Box::new(skolemize_rec(a, universal_scope, ctx)),
            Box::new(skolemize_rec(b, universal_scope, ctx)),
        ),
        Formula::Implies(a, b) => Formula::Implies(
            Box::new(skolemize_rec(a, universal_scope, ctx)),
            Box::new(skolemize_rec(b, universal_scope, ctx)),
        ),
        Formula::Iff(a, b) => Formula::Iff(
            Box::new(skolemize_rec(a, universal_scope, ctx)),
            Box::new(skolemize_rec(b, universal_scope, ctx)),
        ),
        Formula::ForAll(v, body) => {
            universal_scope.push(v.clone());
            let body2 = skolemize_rec(body, universal_scope, ctx);
            universal_scope.pop();
            Formula::ForAll(v.clone(), Box::new(body2))
        }
        Formula::Exists(v, body) => {
            let skolem_term = if universal_scope.is_empty() {
                Term::Const(ctx.fresh_name())
            } else {
                let args = universal_scope
                    .iter()
                    .map(|n| Term::Var(n.clone()))
                    .collect();
                Term::Func(ctx.fresh_name(), args)
            };
            let mut subst = Substitution::new();
            subst.bind(v.clone(), skolem_term);
            let body_substituted = subst.apply_formula(body);
            // The existential is now eliminated: recurse directly into the
            // substituted body without re-wrapping in `Exists`.
            skolemize_rec(&body_substituted, universal_scope, ctx)
        }
    }
}

/// Pass 4: strip the (now-implicit) universal quantifiers left after
/// Skolemization, wherever they appear in the tree, leaving a
/// quantifier-free matrix. Any `Exists` encountered here indicates
/// `skolemize` wasn't run first; it's stripped too, defensively.
pub fn drop_universal_quantifiers(f: &Formula) -> Formula {
    match f {
        Formula::True | Formula::False | Formula::Atom(..) => f.clone(),
        Formula::Not(inner) => Formula::Not(Box::new(drop_universal_quantifiers(inner))),
        Formula::And(a, b) => Formula::And(
            Box::new(drop_universal_quantifiers(a)),
            Box::new(drop_universal_quantifiers(b)),
        ),
        Formula::Or(a, b) => Formula::Or(
            Box::new(drop_universal_quantifiers(a)),
            Box::new(drop_universal_quantifiers(b)),
        ),
        Formula::Implies(a, b) => Formula::Implies(
            Box::new(drop_universal_quantifiers(a)),
            Box::new(drop_universal_quantifiers(b)),
        ),
        Formula::Iff(a, b) => Formula::Iff(
            Box::new(drop_universal_quantifiers(a)),
            Box::new(drop_universal_quantifiers(b)),
        ),
        Formula::ForAll(_, body) => drop_universal_quantifiers(body),
        Formula::Exists(_, body) => drop_universal_quantifiers(body),
    }
}

/// Pass 5: distribute `∨` over `∧` (standard CNF distribution law) so the
/// tree becomes a conjunction of disjunctions of literals.
pub fn distribute_cnf(f: &Formula) -> Formula {
    match f {
        Formula::Or(a, b) => {
            let a2 = distribute_cnf(a);
            let b2 = distribute_cnf(b);
            distribute_or(&a2, &b2)
        }
        Formula::And(a, b) => {
            Formula::And(Box::new(distribute_cnf(a)), Box::new(distribute_cnf(b)))
        }
        _ => f.clone(),
    }
}

fn distribute_or(a: &Formula, b: &Formula) -> Formula {
    match (a, b) {
        (Formula::And(a1, a2), _) => Formula::And(
            Box::new(distribute_or(a1, b)),
            Box::new(distribute_or(a2, b)),
        ),
        (_, Formula::And(b1, b2)) => Formula::And(
            Box::new(distribute_or(a, b1)),
            Box::new(distribute_or(a, b2)),
        ),
        _ => Formula::Or(Box::new(a.clone()), Box::new(b.clone())),
    }
}

/// Flatten a quantifier-free CNF-shaped formula tree into `Vec<Vec<Literal>>`.
pub fn formula_to_clauses(f: &Formula) -> Result<Vec<Vec<Literal>>, LogicError> {
    fn collect_and(f: &Formula, out: &mut Vec<Formula>) {
        match f {
            Formula::And(a, b) => {
                collect_and(a, out);
                collect_and(b, out);
            }
            other => out.push(other.clone()),
        }
    }

    fn collect_or(f: &Formula, out: &mut Vec<Literal>) -> Result<(), LogicError> {
        match f {
            Formula::Or(a, b) => {
                collect_or(a, out)?;
                collect_or(b, out)
            }
            Formula::Atom(name, args) => {
                out.push(Literal {
                    negated: false,
                    predicate: name.clone(),
                    args: args.clone(),
                });
                Ok(())
            }
            Formula::Not(inner) => match inner.as_ref() {
                Formula::Atom(name, args) => {
                    out.push(Literal {
                        negated: true,
                        predicate: name.clone(),
                        args: args.clone(),
                    });
                    Ok(())
                }
                _ => Err(LogicError::NotInClauseForm),
            },
            Formula::True | Formula::False => Ok(()),
            _ => Err(LogicError::NotInClauseForm),
        }
    }

    let mut conjuncts = Vec::new();
    collect_and(f, &mut conjuncts);
    let mut clauses = Vec::new();
    for c in conjuncts {
        let mut lits = Vec::new();
        collect_or(&c, &mut lits)?;
        clauses.push(lits);
    }
    Ok(clauses)
}

/// Run the full CNF normalization pipeline (passes 1-5) and return the
/// resulting clause set.
pub fn normalize_cnf(f: &Formula) -> Result<Vec<Vec<Literal>>, LogicError> {
    let step1 = eliminate_iff_implies(f);
    let step2 = to_nnf(&step1);
    let step3 = skolemize(&step2);
    let step4 = drop_universal_quantifiers(&step3);
    let step5 = distribute_cnf(&step4);
    formula_to_clauses(&step5)
}

fn atom_key(name: &str, args: &[Term]) -> String {
    if args.is_empty() {
        name.to_string()
    } else {
        let rendered: Vec<String> = args.iter().map(|t| t.to_string()).collect();
        format!("{name}({})", rendered.join(", "))
    }
}

/// Evaluate a ground (variable-free), quantifier-free formula against a
/// truth-value assignment keyed by atom (e.g. `"Rains"` or
/// `"Loves(alice, bob)"`, matching [`Term`]'s `Display` rendering).
pub fn evaluate(f: &Formula, valuation: &HashMap<String, bool>) -> Result<bool, LogicError> {
    match f {
        Formula::True => Ok(true),
        Formula::False => Ok(false),
        Formula::Atom(name, args) => {
            let key = atom_key(name, args);
            valuation
                .get(&key)
                .copied()
                .ok_or(LogicError::UnknownAtom(key))
        }
        Formula::Not(a) => Ok(!evaluate(a, valuation)?),
        Formula::And(a, b) => Ok(evaluate(a, valuation)? && evaluate(b, valuation)?),
        Formula::Or(a, b) => Ok(evaluate(a, valuation)? || evaluate(b, valuation)?),
        Formula::Implies(a, b) => Ok(!evaluate(a, valuation)? || evaluate(b, valuation)?),
        Formula::Iff(a, b) => Ok(evaluate(a, valuation)? == evaluate(b, valuation)?),
        Formula::ForAll(..) | Formula::Exists(..) => Err(LogicError::UnboundQuantifierInEvaluation),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_variables_excludes_bound() {
        // ∀x P(x, y) — x is bound, y is free.
        let f = Formula::ForAll(
            "x".into(),
            Box::new(Formula::atom(
                "P",
                vec![Term::Var("x".into()), Term::Var("y".into())],
            )),
        );
        let vars = free_variables(&f);
        assert_eq!(vars.len(), 1);
        assert!(vars.contains("y"));
    }

    #[test]
    fn cnf_of_implication() {
        // P -> Q  ==  ¬P ∨ Q
        let f = Formula::Implies(Box::new(Formula::prop("P")), Box::new(Formula::prop("Q")));
        let clauses = normalize_cnf(&f).unwrap();
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0].len(), 2);
        assert!(clauses[0].iter().any(|l| l.negated && l.predicate == "P"));
        assert!(clauses[0].iter().any(|l| !l.negated && l.predicate == "Q"));
    }

    #[test]
    fn cnf_of_conjunction_splits_into_two_clauses() {
        let f = Formula::And(Box::new(Formula::prop("P")), Box::new(Formula::prop("Q")));
        let clauses = normalize_cnf(&f).unwrap();
        assert_eq!(clauses.len(), 2);
    }

    #[test]
    fn skolemize_existential_under_universal_produces_function_of_universal_var() {
        // ∀x ∃y P(x, y)  ->  P(x, sk1(x))  (x remains implicitly universal)
        let inner = Formula::atom("P", vec![Term::Var("x".into()), Term::Var("y".into())]);
        let exists_y = Formula::Exists("y".into(), Box::new(inner));
        let forall_x = Formula::ForAll("x".into(), Box::new(exists_y));

        let clauses = normalize_cnf(&forall_x).unwrap();
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0].len(), 1);
        let lit = &clauses[0][0];
        assert_eq!(lit.predicate, "P");
        assert_eq!(lit.args[0], Term::Var("x".into()));
        assert_eq!(
            lit.args[1],
            Term::Func("sk1".into(), vec![Term::Var("x".into())])
        );
    }

    #[test]
    fn skolemize_existential_with_no_enclosing_universal_produces_constant() {
        // ∃y Human(y) -> Human(sk1)
        let f = Formula::Exists(
            "y".into(),
            Box::new(Formula::atom("Human", vec![Term::Var("y".into())])),
        );
        let clauses = normalize_cnf(&f).unwrap();
        assert_eq!(clauses[0][0].args[0], Term::Const("sk1".into()));
    }

    #[test]
    fn evaluate_propositional_formula() {
        let f = Formula::And(
            Box::new(Formula::prop("P")),
            Box::new(Formula::Not(Box::new(Formula::prop("Q")))),
        );
        let mut val = HashMap::new();
        val.insert("P".to_string(), true);
        val.insert("Q".to_string(), false);
        assert_eq!(evaluate(&f, &val).unwrap(), true);
    }

    #[test]
    fn evaluate_rejects_quantifiers() {
        let f = Formula::ForAll("x".into(), Box::new(Formula::prop("P")));
        assert_eq!(
            evaluate(&f, &HashMap::new()),
            Err(LogicError::UnboundQuantifierInEvaluation)
        );
    }
}
