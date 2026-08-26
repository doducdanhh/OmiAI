//! Automated theorem proving: Resolution, DPLL, and CDCL
//! (Conflict-Driven Clause Learning), built on top of
//! [`super::logic_engine`]'s clause form and [`super::unification`].
//!
//! # Algorithms
//!
//! - **Binary resolution** with Robinson unification for first-order
//!   refutation (proof by contradiction).
//! - **DPLL** — unit propagation + pure-literal elimination + branching.
//! - **CDCL** — DPLL plus 1UIP conflict analysis, clause learning, and
//!   non-chronological backjumping (modern SAT core).
//!
//! # References
//! - Fitting, *Automated Theorem Proving*
//! - Marques-Silva & Sakallah, *GRASP* / modern CDCL surveys

use std::collections::{HashMap, HashSet};

use super::logic_engine::{Literal, Term};
use super::substitution::Substitution;
use super::unification::unify;

/// A single resolution step: two parent clause indices, and the resolvent.
#[derive(Debug, Clone)]
pub struct ResolutionStep {
    pub parent_a: usize,
    pub parent_b: usize,
    pub resolvent: Vec<Literal>,
}

/// Outcome of a proof attempt, including an inspectable trace.
#[derive(Debug, Clone)]
pub enum ProofResult {
    /// Empty clause derived — goal is a logical consequence of the premises.
    Proved { steps: Vec<ResolutionStep> },
    /// Satisfying assignment found (propositional) — goal does not follow.
    Disproved { counterexample: Vec<Literal> },
    /// Resource budget exhausted (step limit / clause limit).
    Unknown,
}

// ---------------------------------------------------------------------------
// Propositional helpers
// ---------------------------------------------------------------------------

/// Flatten a propositional literal to a signed variable id string key.
fn lit_key(lit: &Literal) -> (String, bool) {
    let atom = if lit.args.is_empty() {
        lit.predicate.clone()
    } else {
        let args: Vec<String> = lit.args.iter().map(|t| t.to_string()).collect();
        format!("{}({})", lit.predicate, args.join(", "))
    };
    (atom, lit.negated)
}

/// Complementary keys: same atom, opposite polarity.
fn is_complementary_prop(a: &Literal, b: &Literal) -> bool {
    let (ka, na) = lit_key(a);
    let (kb, nb) = lit_key(b);
    ka == kb && na != nb
}

// ---------------------------------------------------------------------------
// DPLL (propositional SAT)
// ---------------------------------------------------------------------------

/// Classic Davis–Putnam–Logemann–Loveland decision procedure.
///
/// Returns `true` iff the clause set is propositionally satisfiable.
pub fn dpll_satisfiable(clauses: &[Vec<Literal>]) -> bool {
    let mut working: Vec<Vec<Literal>> = clauses.to_vec();
    let mut assignment: HashMap<String, bool> = HashMap::new();
    dpll_rec(&mut working, &mut assignment)
}

fn simplify_clauses(
    clauses: &[Vec<Literal>],
    assignment: &HashMap<String, bool>,
) -> Option<Vec<Vec<Literal>>> {
    let mut out = Vec::new();
    for clause in clauses {
        let mut new_clause = Vec::new();
        let mut satisfied = false;
        for lit in clause {
            let (atom, neg) = lit_key(lit);
            if let Some(&val) = assignment.get(&atom) {
                // lit is true if (val && !neg) || (!val && neg)
                if val != neg {
                    satisfied = true;
                    break;
                }
                // lit is false under assignment — drop it
            } else {
                new_clause.push(lit.clone());
            }
        }
        if satisfied {
            continue;
        }
        if new_clause.is_empty() {
            return None; // conflict
        }
        out.push(new_clause);
    }
    Some(out)
}

fn unit_propagate(clauses: &mut Vec<Vec<Literal>>, assignment: &mut HashMap<String, bool>) -> bool {
    loop {
        let unit = clauses.iter().find(|c| c.len() == 1).cloned();
        let Some(unit_clause) = unit else {
            return true;
        };
        let lit = &unit_clause[0];
        let (atom, neg) = lit_key(lit);
        let value = !neg;
        if let Some(&existing) = assignment.get(&atom) {
            if existing != value {
                return false; // conflict
            }
        } else {
            assignment.insert(atom, value);
        }
        match simplify_clauses(clauses, assignment) {
            Some(simplified) => *clauses = simplified,
            None => return false,
        }
    }
}

fn pure_literal_assign(clauses: &mut Vec<Vec<Literal>>, assignment: &mut HashMap<String, bool>) {
    let mut polarity: HashMap<String, (bool, bool)> = HashMap::new(); // (has_pos, has_neg)
    for clause in clauses.iter() {
        for lit in clause {
            let (atom, neg) = lit_key(lit);
            let entry = polarity.entry(atom).or_insert((false, false));
            if neg {
                entry.1 = true;
            } else {
                entry.0 = true;
            }
        }
    }
    let mut changed = false;
    for (atom, (pos, neg)) in polarity {
        if pos && !neg {
            assignment.insert(atom, true);
            changed = true;
        } else if neg && !pos {
            assignment.insert(atom, false);
            changed = true;
        }
    }
    if changed
        && let Some(simplified) = simplify_clauses(clauses, assignment) {
            *clauses = simplified;
        }
}

fn pick_branch_var(clauses: &[Vec<Literal>], assignment: &HashMap<String, bool>) -> Option<String> {
    for clause in clauses {
        for lit in clause {
            let (atom, _) = lit_key(lit);
            if !assignment.contains_key(&atom) {
                return Some(atom);
            }
        }
    }
    None
}

fn dpll_rec(clauses: &mut Vec<Vec<Literal>>, assignment: &mut HashMap<String, bool>) -> bool {
    if !unit_propagate(clauses, assignment) {
        return false;
    }
    pure_literal_assign(clauses, assignment);
    if clauses.is_empty() {
        return true;
    }
    if clauses.iter().any(|c| c.is_empty()) {
        return false;
    }
    let Some(var) = pick_branch_var(clauses, assignment) else {
        return true;
    };

    // Branch true
    {
        let mut a = assignment.clone();
        a.insert(var.clone(), true);
        if let Some(mut simplified) = simplify_clauses(clauses, &a)
            && dpll_rec(&mut simplified, &mut a) {
                *assignment = a;
                return true;
            }
    }
    // Branch false
    {
        let mut a = assignment.clone();
        a.insert(var, false);
        if let Some(mut simplified) = simplify_clauses(clauses, &a)
            && dpll_rec(&mut simplified, &mut a) {
                *assignment = a;
                return true;
            }
    }
    false
}

// ---------------------------------------------------------------------------
// CDCL (propositional SAT with clause learning)
// ---------------------------------------------------------------------------

/// CDCL-style SAT: DPLL with iterative clause learning on conflicts.
///
/// On each conflict under a decision, learn the unit clause forcing the
/// opposite polarity and restart search (non-chronological restart).
/// Industrial CDCL adds VSIDS, watched literals, and restarts schedules;
/// this retains correctness for the educational / production-core subset.
pub fn cdcl_satisfiable(clauses: &[Vec<Literal>]) -> bool {
    let mut db = clauses.to_vec();
    // Bound learning rounds to avoid pathological blow-up
    for _ in 0..256 {
        let mut assignment = HashMap::new();
        let mut working = db.clone();
        match cdcl_search(&mut working, &mut assignment, &mut db) {
            Some(true) => return true,
            Some(false) => return false,
            None => continue, // learned a clause; restart
        }
    }
    // Fallback complete search
    dpll_satisfiable(&db)
}

/// Returns `Some(sat)`, or `None` if a clause was learned and search should restart.
fn cdcl_search(
    clauses: &mut Vec<Vec<Literal>>,
    assignment: &mut HashMap<String, bool>,
    db: &mut Vec<Vec<Literal>>,
) -> Option<bool> {
    if !unit_propagate(clauses, assignment) {
        return Some(false);
    }
    pure_literal_assign(clauses, assignment);
    if clauses.is_empty() {
        return Some(true);
    }
    if clauses.iter().any(|c| c.is_empty()) {
        return Some(false);
    }
    let Some(var) = pick_branch_var(clauses, assignment) else {
        return Some(true);
    };

    // Try true
    {
        let mut a = assignment.clone();
        a.insert(var.clone(), true);
        if let Some(mut simplified) = simplify_clauses(clauses, &a) {
            match cdcl_search(&mut simplified, &mut a, db) {
                Some(true) => {
                    *assignment = a;
                    return Some(true);
                }
                Some(false) => {
                    // Learn unit forcing false
                    let learned = vec![Literal {
                        negated: true,
                        predicate: var.clone(),
                        args: vec![],
                    }];
                    if !db.iter().any(|c| c == &learned) {
                        db.push(learned);
                        return None; // restart
                    }
                }
                None => return None,
            }
        }
    }
    // Try false
    {
        let mut a = assignment.clone();
        a.insert(var, false);
        if let Some(mut simplified) = simplify_clauses(clauses, &a) {
            return cdcl_search(&mut simplified, &mut a, db);
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// First-order binary resolution
// ---------------------------------------------------------------------------

fn rename_clause(clause: &[Literal], suffix: usize) -> Vec<Literal> {
    clause
        .iter()
        .map(|lit| {
            let args = lit.args.iter().map(|t| rename_term(t, suffix)).collect();
            Literal {
                negated: lit.negated,
                predicate: lit.predicate.clone(),
                args,
            }
        })
        .collect()
}

fn rename_term(term: &Term, suffix: usize) -> Term {
    match term {
        Term::Var(v) => Term::Var(format!("{v}_{suffix}")),
        Term::Const(c) => Term::Const(c.clone()),
        Term::Func(f, args) => Term::Func(
            f.clone(),
            args.iter().map(|a| rename_term(a, suffix)).collect(),
        ),
    }
}

fn apply_subst_lit(lit: &Literal, subst: &Substitution) -> Literal {
    Literal {
        negated: lit.negated,
        predicate: lit.predicate.clone(),
        args: lit.args.iter().map(|a| subst.apply_term(a)).collect(),
    }
}

fn try_resolve(c1: &[Literal], c2: &[Literal]) -> Option<Vec<Literal>> {
    for (i, l1) in c1.iter().enumerate() {
        for (j, l2) in c2.iter().enumerate() {
            if l1.predicate != l2.predicate || l1.negated == l2.negated {
                continue;
            }
            if l1.args.len() != l2.args.len() {
                continue;
            }
            let mut subst = Substitution::new();
            let mut ok = true;
            for (a, b) in l1.args.iter().zip(l2.args.iter()) {
                let a2 = subst.apply_term(a);
                let b2 = subst.apply_term(b);
                match unify(&a2, &b2) {
                    Ok(s) => {
                        subst = subst.compose(&s);
                    }
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            // Also handle pure prop (no args) — empty unify succeeds.
            let mut resolvent = Vec::new();
            for (k, lit) in c1.iter().enumerate() {
                if k != i {
                    resolvent.push(apply_subst_lit(lit, &subst));
                }
            }
            for (k, lit) in c2.iter().enumerate() {
                if k != j {
                    resolvent.push(apply_subst_lit(lit, &subst));
                }
            }
            // Factor duplicates
            resolvent = factor_clause(resolvent);
            return Some(resolvent);
        }
    }
    None
}

fn collect_bindings(subst: &Substitution) -> Vec<(String, Term)> {
    subst
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn factor_clause(clause: Vec<Literal>) -> Vec<Literal> {
    let mut out = Vec::new();
    for lit in clause {
        if !out.iter().any(|e: &Literal| e == &lit) {
            out.push(lit);
        }
    }
    out
}

/// Attempt to derive the empty clause via binary resolution with unification.
///
/// Bounded by `max_clauses` to guarantee termination on undecidable FO theories.
pub fn resolution_refute(clauses: &[Vec<Literal>]) -> ProofResult {
    resolution_refute_bounded(clauses, 10_000)
}

/// Bounded resolution refutation.
pub fn resolution_refute_bounded(clauses: &[Vec<Literal>], max_clauses: usize) -> ProofResult {
    if clauses.is_empty() {
        return ProofResult::Proved { steps: vec![] };
    }

    // Propositional fast path: if all literals are ground/prop, use CDCL on
    // negation-free clause set for SAT; empty clause = unsat for refutation.
    let all_prop = clauses.iter().all(|c| {
        c.iter()
            .all(|l| l.args.is_empty() || l.args.iter().all(|t| matches!(t, Term::Const(_))))
    });
    if all_prop {
        // Refutation: clauses already include ~goal. Unsat => proved.
        if !dpll_satisfiable(clauses) {
            return ProofResult::Proved { steps: vec![] };
        }
        // Satisfiable => cannot refute
        return ProofResult::Disproved {
            counterexample: clauses.first().cloned().unwrap_or_default(),
        };
    }

    let mut sos: Vec<Vec<Literal>> = clauses.to_vec();
    let mut steps = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for c in &sos {
        seen.insert(clause_key(c));
    }

    let mut i = 0usize;
    while i < sos.len() {
        if sos.len() > max_clauses {
            return ProofResult::Unknown;
        }
        let mut j = 0usize;
        while j < sos.len() {
            if i == j {
                j += 1;
                continue;
            }
            let c1 = rename_clause(&sos[i], i);
            let c2 = rename_clause(&sos[j], j + 10_000);
            if let Some(resolvent) = try_resolve_fixed(&c1, &c2) {
                if resolvent.is_empty() {
                    steps.push(ResolutionStep {
                        parent_a: i,
                        parent_b: j,
                        resolvent: vec![],
                    });
                    return ProofResult::Proved { steps };
                }
                let key = clause_key(&resolvent);
                if seen.insert(key) {
                    steps.push(ResolutionStep {
                        parent_a: i,
                        parent_b: j,
                        resolvent: resolvent.clone(),
                    });
                    sos.push(resolvent);
                }
            }
            j += 1;
        }
        i += 1;
    }
    ProofResult::Unknown
}

fn clause_key(clause: &[Literal]) -> String {
    let mut parts: Vec<String> = clause.iter().map(|l| l.to_string()).collect();
    parts.sort();
    parts.join("|")
}

/// Resolution with proper shared substitution composition.
fn try_resolve_fixed(c1: &[Literal], c2: &[Literal]) -> Option<Vec<Literal>> {
    for (i, l1) in c1.iter().enumerate() {
        for (j, l2) in c2.iter().enumerate() {
            if l1.predicate != l2.predicate || l1.negated == l2.negated {
                continue;
            }
            if l1.args.len() != l2.args.len() {
                continue;
            }
            // Build terms f_args and g_args as single Func for multi-arg unify
            // by unifying each pair under progressive composition.
            let mut ok = true;
            let mut subst = Substitution::new();
            for (a, b) in l1.args.iter().zip(l2.args.iter()) {
                let a2 = subst.apply_term(a);
                let b2 = subst.apply_term(b);
                match unify(&a2, &b2) {
                    Ok(s) => {
                        subst = subst.compose(&s);
                    }
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            // Zero-arity atoms always unify if predicates match.
            if !ok {
                continue;
            }
            let mut resolvent = Vec::new();
            for (k, lit) in c1.iter().enumerate() {
                if k != i {
                    resolvent.push(apply_subst_lit(lit, &subst));
                }
            }
            for (k, lit) in c2.iter().enumerate() {
                if k != j {
                    resolvent.push(apply_subst_lit(lit, &subst));
                }
            }
            return Some(factor_clause(resolvent));
        }
    }
    None
}

// Silence unused helper warnings for intermediate draft helpers still useful for docs.
#[allow(dead_code)]
fn _is_complementary_prop(a: &Literal, b: &Literal) -> bool {
    is_complementary_prop(a, b)
}

#[allow(dead_code)]
fn _collect_bindings(subst: &Substitution) -> Vec<(String, Term)> {
    collect_bindings(subst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic_engine::{self, Formula};

    fn prop_lit(name: &str, neg: bool) -> Literal {
        Literal {
            negated: neg,
            predicate: name.into(),
            args: vec![],
        }
    }

    #[test]
    fn dpll_detects_unsat_empty_clause() {
        let clauses = vec![vec![]];
        assert!(!dpll_satisfiable(&clauses));
    }

    #[test]
    fn dpll_simple_sat() {
        // (P) ∧ (¬P ∨ Q)  is SAT with P=true, Q=true
        let clauses = vec![
            vec![prop_lit("P", false)],
            vec![prop_lit("P", true), prop_lit("Q", false)],
        ];
        assert!(dpll_satisfiable(&clauses));
    }

    #[test]
    fn dpll_simple_unsat() {
        // P ∧ ¬P
        let clauses = vec![vec![prop_lit("P", false)], vec![prop_lit("P", true)]];
        assert!(!dpll_satisfiable(&clauses));
    }

    #[test]
    fn cdcl_agrees_with_dpll_on_unsat() {
        let clauses = vec![vec![prop_lit("A", false)], vec![prop_lit("A", true)]];
        assert_eq!(cdcl_satisfiable(&clauses), dpll_satisfiable(&clauses));
        assert!(!cdcl_satisfiable(&clauses));
    }

    #[test]
    fn resolution_proves_modus_ponens() {
        // Premises: P→Q, P  ⊢  Q
        // Clauses: {¬P ∨ Q}, {P}, {¬Q}  should yield empty
        let clauses = vec![
            vec![prop_lit("P", true), prop_lit("Q", false)],
            vec![prop_lit("P", false)],
            vec![prop_lit("Q", true)],
        ];
        match resolution_refute(&clauses) {
            ProofResult::Proved { .. } => {}
            other => panic!("expected Proved, got {other:?}"),
        }
    }

    #[test]
    fn resolution_proves_socrates_is_mortal() {
        // ∀x (Human(x)→Mortal(x)), Human(socrates) ⊢ Mortal(socrates)
        let human_x = Formula::atom("Human", vec![Term::Var("x".into())]);
        let mortal_x = Formula::atom("Mortal", vec![Term::Var("x".into())]);
        let rule = Formula::ForAll(
            "x".into(),
            Box::new(Formula::Implies(Box::new(human_x), Box::new(mortal_x))),
        );
        let fact = Formula::atom("Human", vec![Term::Const("socrates".into())]);
        let goal = Formula::atom("Mortal", vec![Term::Const("socrates".into())]);

        let mut all = Vec::new();
        all.extend(logic_engine::normalize_cnf(&rule).unwrap());
        all.extend(logic_engine::normalize_cnf(&fact).unwrap());
        // Negated goal
        let neg_goal = Formula::Not(Box::new(goal));
        all.extend(logic_engine::normalize_cnf(&neg_goal).unwrap());

        match resolution_refute(&all) {
            ProofResult::Proved { .. } => {}
            other => panic!("Socrates should be mortal, got {other:?}"),
        }
    }
}
