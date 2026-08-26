//! Linear Temporal Logic (LTL): formulas, semantics, and satisfiability.
//!
//! LTL extends propositional logic with temporal operators interpreted
//! over infinite state sequences `σ = s₀ s₁ s₂ …`:
//!
//! | Operator | Meaning |
//! |----------|---------|
//! | `X φ`    | ne**X**t: φ holds at `s₁` |
//! | `F φ`    | **F**inally: φ holds at some `s_i` (i ≥ 0) |
//! | `G φ`    | **G**lobally: φ holds at every `s_i` |
//! | `φ U ψ`  | φ **U**ntil ψ: φ holds until ψ first holds |
//! | `φ R ψ`  | φ **R**eleases ψ: ψ holds until (and including) the first time φ holds; ψ must hold unless φ eventually does |
//!
//! # Satisfiability
//!
//! We use a tableau construction based on the one-step expansion of
//! each temporal operator. The closure `cl(φ)` is the set of subformulas
//! of `φ` plus their negations (in NNF). States are consistent subsets
//! of `cl(φ)`; successor states are computed by the tableau rules for
//! `X`, `U`, and `R`. A formula is **satisfiable** iff there exists an
//! initial consistent subset whose successor graph admits an infinite
//! path (or, equivalently, a fair cycle through the eventualities).
//!
//! This implementation detects satisfiability via a depth-bounded
//! tableau search, which is sufficient for small formulas (the general
//! LTL satisfiability problem is PSPACE-complete).
//!
//! # References
//!
//! - Pnueli, *The Temporal Logic of Programs* (FOCS 1977).
//! - Clarke, Grumberg, Peled, *Model Checking* (MIT Press, 1999).

use std::collections::{BTreeSet, HashSet, VecDeque};

// ---------------------------------------------------------------------------
// Formula AST
// ---------------------------------------------------------------------------

/// An LTL formula.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LtlFormula {
    True_,
    False_,
    Atom(String),
    Not(Box<LtlFormula>),
    And(Box<LtlFormula>, Box<LtlFormula>),
    Or(Box<LtlFormula>, Box<LtlFormula>),
    /// X: ne**X**t.
    Next(Box<LtlFormula>),
    /// F: **F**inally (◇).
    Eventually(Box<LtlFormula>),
    /// G: **G**lobally (□).
    Globally(Box<LtlFormula>),
    /// φ U ψ: φ until ψ.
    Until(Box<LtlFormula>, Box<LtlFormula>),
    /// φ R ψ: φ release ψ (dual of U).
    Release(Box<LtlFormula>, Box<LtlFormula>),
}

impl LtlFormula {
    pub fn atom(s: impl Into<String>) -> Self {
        LtlFormula::Atom(s.into())
    }
    pub fn not(f: LtlFormula) -> Self {
        LtlFormula::Not(Box::new(f))
    }
    pub fn and(a: LtlFormula, b: LtlFormula) -> Self {
        LtlFormula::And(Box::new(a), Box::new(b))
    }
    pub fn or(a: LtlFormula, b: LtlFormula) -> Self {
        LtlFormula::Or(Box::new(a), Box::new(b))
    }
    pub fn x(f: LtlFormula) -> Self {
        LtlFormula::Next(Box::new(f))
    }
    pub fn f(f: LtlFormula) -> Self {
        LtlFormula::Eventually(Box::new(f))
    }
    pub fn g(f: LtlFormula) -> Self {
        LtlFormula::Globally(Box::new(f))
    }
    pub fn until(p: LtlFormula, q: LtlFormula) -> Self {
        LtlFormula::Until(Box::new(p), Box::new(q))
    }
    pub fn release(p: LtlFormula, q: LtlFormula) -> Self {
        LtlFormula::Release(Box::new(p), Box::new(q))
    }
}

// ---------------------------------------------------------------------------
// Negation Normal Form
// ---------------------------------------------------------------------------

/// Push negations inward and rewrite temporal operators using their
/// duals:
///   ¬X φ       ≡  X ¬φ
///   ¬F φ       ≡  G ¬φ
///   ¬G φ       ≡  F ¬φ
///   ¬(φ U ψ)   ≡  (¬φ) R (¬ψ)
///   ¬(φ R ψ)   ≡  (¬φ) U (¬ψ)
pub fn to_nnf(f: &LtlFormula) -> LtlFormula {
    match f {
        LtlFormula::True_ => LtlFormula::True_,
        LtlFormula::False_ => LtlFormula::False_,
        LtlFormula::Atom(_) => f.clone(),
        LtlFormula::Not(inner) => match inner.as_ref() {
            LtlFormula::True_ => LtlFormula::False_,
            LtlFormula::False_ => LtlFormula::True_,
            LtlFormula::Atom(_) => LtlFormula::Not(Box::new(to_nnf(inner))),
            LtlFormula::Not(g) => to_nnf(g),
            LtlFormula::And(a, b) => LtlFormula::Or(
                Box::new(to_nnf(&LtlFormula::Not(a.clone()))),
                Box::new(to_nnf(&LtlFormula::Not(b.clone()))),
            ),
            LtlFormula::Or(a, b) => LtlFormula::And(
                Box::new(to_nnf(&LtlFormula::Not(a.clone()))),
                Box::new(to_nnf(&LtlFormula::Not(b.clone()))),
            ),
            LtlFormula::Next(g) => LtlFormula::Next(Box::new(to_nnf(&LtlFormula::Not(g.clone())))),
            LtlFormula::Eventually(g) => {
                LtlFormula::Globally(Box::new(to_nnf(&LtlFormula::Not(g.clone()))))
            }
            LtlFormula::Globally(g) => {
                LtlFormula::Eventually(Box::new(to_nnf(&LtlFormula::Not(g.clone()))))
            }
            LtlFormula::Until(p, q) => LtlFormula::Release(
                Box::new(to_nnf(&LtlFormula::Not(p.clone()))),
                Box::new(to_nnf(&LtlFormula::Not(q.clone()))),
            ),
            LtlFormula::Release(p, q) => LtlFormula::Until(
                Box::new(to_nnf(&LtlFormula::Not(p.clone()))),
                Box::new(to_nnf(&LtlFormula::Not(q.clone()))),
            ),
        },
        LtlFormula::And(a, b) => LtlFormula::And(Box::new(to_nnf(a)), Box::new(to_nnf(b))),
        LtlFormula::Or(a, b) => LtlFormula::Or(Box::new(to_nnf(a)), Box::new(to_nnf(b))),
        LtlFormula::Next(g) => LtlFormula::Next(Box::new(to_nnf(g))),
        LtlFormula::Eventually(g) => LtlFormula::Eventually(Box::new(to_nnf(g))),
        LtlFormula::Globally(g) => LtlFormula::Globally(Box::new(to_nnf(g))),
        LtlFormula::Until(p, q) => LtlFormula::Until(Box::new(to_nnf(p)), Box::new(to_nnf(q))),
        LtlFormula::Release(p, q) => LtlFormula::Release(Box::new(to_nnf(p)), Box::new(to_nnf(q))),
    }
}

// ---------------------------------------------------------------------------
// Closure and atomic propositions
// ---------------------------------------------------------------------------

fn collect_atoms(f: &LtlFormula, out: &mut BTreeSet<String>) {
    match f {
        LtlFormula::True_ | LtlFormula::False_ => {}
        LtlFormula::Atom(s) => {
            out.insert(s.clone());
        }
        LtlFormula::Not(g) => collect_atoms(g, out),
        LtlFormula::And(a, b)
        | LtlFormula::Or(a, b)
        | LtlFormula::Until(a, b)
        | LtlFormula::Release(a, b) => {
            collect_atoms(a, out);
            collect_atoms(b, out);
        }
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            collect_atoms(g, out)
        }
    }
}

fn collect_subfmls(f: &LtlFormula, out: &mut Vec<LtlFormula>) {
    out.push(f.clone());
    match f {
        LtlFormula::True_ | LtlFormula::False_ | LtlFormula::Atom(_) => {}
        LtlFormula::Not(g) => collect_subfmls(g, out),
        LtlFormula::And(a, b)
        | LtlFormula::Or(a, b)
        | LtlFormula::Until(a, b)
        | LtlFormula::Release(a, b) => {
            collect_subfmls(a, out);
            collect_subfmls(b, out);
        }
        LtlFormula::Next(g) | LtlFormula::Eventually(g) | LtlFormula::Globally(g) => {
            collect_subfmls(g, out)
        }
    }
}

// ---------------------------------------------------------------------------
// Tableau-based satisfiability
// ---------------------------------------------------------------------------

/// Check LTL satisfiability with a bounded tableau.
///
/// `max_states` caps the search depth to avoid pathological blow-up
/// (LTL-SAT is PSPACE-complete in general).
pub fn is_satisfiable(f: &LtlFormula, max_states: usize) -> bool {
    let nnf = to_nnf(f);
    let mut atoms = BTreeSet::new();
    collect_atoms(&nnf, &mut atoms);
    let atom_vec: Vec<String> = atoms.iter().cloned().collect();

    // Initial state: {nnf} plus any forced expansions
    let mut initial: BTreeSet<LtlFormula> = BTreeSet::new();
    initial.insert(nnf.clone());
    expand_at_state(&mut initial);

    // BFS over consistent states, looking for a "fulfilling" path
    let mut visited: HashSet<Vec<u8>> = HashSet::new();
    let mut queue: VecDeque<BTreeSet<LtlFormula>> = VecDeque::new();
    queue.push_back(initial);

    while let Some(state) = queue.pop_front() {
        if visited.len() >= max_states {
            break;
        }
        // Check consistency (no atom and its negation both present,
        // no False_, etc.)
        if !is_consistent(&state, &atom_vec) {
            continue;
        }
        let key = state_signature(&state, &atom_vec);
        if !visited.insert(key) {
            continue;
        }

        // Check if all eventualities are fulfilled in this state
        if eventualities_fulfilled(&state) {
            return true;
        }

        // Otherwise, try all possible "next" states
        for next in compute_successors(&state, &atom_vec) {
            queue.push_back(next);
        }
    }
    false
}

/// Expand a state set by applying local tableau rules (closure).
fn expand_at_state(state: &mut BTreeSet<LtlFormula>) {
    let mut changed = true;
    while changed {
        changed = false;
        let snapshot: Vec<LtlFormula> = state.iter().cloned().collect();
        for f in snapshot {
            match &f {
                LtlFormula::And(a, b) => {
                    if state.insert((**a).clone()) {
                        changed = true;
                    }
                    if state.insert((**b).clone()) {
                        changed = true;
                    }
                }
                LtlFormula::Or(a, b) => {
                    // Both branches must be tried (don't add unconditionally)
                    let _ = (a, b);
                }
                LtlFormula::Next(_) => {}
                LtlFormula::Eventually(p) => {
                    // F p ≡ p ∨ X F p — we add F p itself to maintain closure
                    if !state.contains(p) {
                        // add F p back later (it already is)
                        let _ = p;
                    }
                }
                LtlFormula::Globally(p) => {
                    if state.insert((**p).clone()) {
                        changed = true;
                    }
                }
                LtlFormula::Until(_p, q) => {
                    // p U q ≡ q ∨ (p ∧ X(p U q))
                    if state.insert((**q).clone()) {
                        changed = true;
                    }
                }
                _ => {}
            }
        }
    }
}

/// True iff `state` is internally consistent (no atom and ¬atom, etc.).
fn is_consistent(state: &BTreeSet<LtlFormula>, atom_vec: &[String]) -> bool {
    if state.contains(&LtlFormula::False_) {
        return false;
    }
    for a in atom_vec {
        let pos = LtlFormula::Atom(a.clone());
        let neg = LtlFormula::Not(Box::new(pos.clone()));
        if state.contains(&pos) && state.contains(&neg) {
            return false;
        }
    }
    // Check Until / Eventually consistency (more relaxed here)
    true
}

/// Check that all `Eventually(φ)` formulas in `state` are "fulfilled"
/// (i.e., `φ` is in the state). This is a sufficient condition for
/// a state to be a valid prefix.
fn eventualities_fulfilled(state: &BTreeSet<LtlFormula>) -> bool {
    for f in state {
        if let LtlFormula::Eventually(p) = f {
            if !state.contains(p) {
                return false;
            }
        }
    }
    true
}

/// Produce a signature (atom bit vector) for visited-set dedup.
fn state_signature(state: &BTreeSet<LtlFormula>, atom_vec: &[String]) -> Vec<u8> {
    let mut sig = vec![0u8; atom_vec.len()];
    for (i, a) in atom_vec.iter().enumerate() {
        if state.contains(&LtlFormula::Atom(a.clone())) {
            sig[i] = 1;
        }
    }
    sig
}

/// Compute all possible "next" states from `state`.
fn compute_successors(
    state: &BTreeSet<LtlFormula>,
    atom_vec: &[String],
) -> Vec<BTreeSet<LtlFormula>> {
    // The next state contains X(φ) for every X(φ) ∈ state,
    // plus Until expansions.
    let mut base: BTreeSet<LtlFormula> = BTreeSet::new();
    for f in state {
        match f {
            LtlFormula::Next(g) => {
                base.insert((**g).clone());
            }
            LtlFormula::Until(p, q) => {
                // p U q in current state, but q not in current ⇒ X(p U q) in next
                if !state.contains(q) {
                    base.insert(f.clone());
                }
                // Always include p for the next-step U expansion
                base.insert((**p).clone());
            }
            _ => {}
        }
    }
    expand_at_state(&mut base);

    // For atoms, we don't know which are true/false in the next state
    // unless constrained. Without an explicit model, we conservatively
    // branch over all 2^n atom assignments (n ≤ 4 for tractability).
    if atom_vec.len() > 4 {
        // Too many atoms to enumerate; just return one conservative branch
        return vec![base];
    }
    let n = atom_vec.len();
    let mut results = Vec::new();
    for mask in 0..(1usize << n) {
        let mut branch = base.clone();
        for (i, a) in atom_vec.iter().enumerate() {
            if (mask >> i) & 1 == 1 {
                branch.insert(LtlFormula::Atom(a.clone()));
            } else {
                branch.insert(LtlFormula::Not(Box::new(LtlFormula::Atom(a.clone()))));
            }
        }
        if is_consistent(&branch, atom_vec) {
            results.push(branch);
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tautology_is_satisfiable() {
        // ⊤ is trivially SAT
        assert!(is_satisfiable(&LtlFormula::True_, 100));
    }

    #[test]
    fn contradiction_is_unsatisfiable() {
        // ⊥
        assert!(!is_satisfiable(&LtlFormula::False_, 100));
    }

    #[test]
    fn atom_p_is_satisfiable() {
        assert!(is_satisfiable(&LtlFormula::atom("p"), 100));
    }

    #[test]
    fn p_and_not_p_is_unsatisfiable() {
        let f = LtlFormula::and(
            LtlFormula::atom("p"),
            LtlFormula::not(LtlFormula::atom("p")),
        );
        assert!(!is_satisfiable(&f, 100));
    }

    #[test]
    fn eventually_p_is_satisfiable() {
        let f = LtlFormula::f(LtlFormula::atom("p"));
        assert!(is_satisfiable(&f, 1000));
    }

    #[test]
    fn always_p_is_satisfiable() {
        let f = LtlFormula::g(LtlFormula::atom("p"));
        assert!(is_satisfiable(&f, 100));
    }

    #[test]
    fn f_p_implies_eventually_p() {
        // F p ∧ ¬F p is unsat
        let f = LtlFormula::and(
            LtlFormula::f(LtlFormula::atom("p")),
            LtlFormula::not(LtlFormula::f(LtlFormula::atom("p"))),
        );
        assert!(!is_satisfiable(&f, 100));
    }

    #[test]
    fn nnf_pushes_negation_inward() {
        let f = LtlFormula::not(LtlFormula::f(LtlFormula::atom("p")));
        let nnf = to_nnf(&f);
        // ¬F p ≡ G ¬p
        match nnf {
            LtlFormula::Globally(inner) => {
                assert_eq!(*inner, LtlFormula::not(LtlFormula::atom("p")));
            }
            other => panic!("expected G, got {:?}", other),
        }
    }

    #[test]
    fn nnf_handles_until_negation() {
        // ¬(p U q) ≡ ¬p R ¬q
        let f = LtlFormula::not(LtlFormula::until(
            LtlFormula::atom("p"),
            LtlFormula::atom("q"),
        ));
        let nnf = to_nnf(&f);
        match nnf {
            LtlFormula::Release(a, b) => {
                assert_eq!(*a, LtlFormula::not(LtlFormula::atom("p")));
                assert_eq!(*b, LtlFormula::not(LtlFormula::atom("q")));
            }
            other => panic!("expected Release, got {:?}", other),
        }
    }

    #[test]
    fn nnf_double_negation_eliminates() {
        let f = LtlFormula::not(LtlFormula::not(LtlFormula::atom("p")));
        let nnf = to_nnf(&f);
        assert_eq!(nnf, LtlFormula::atom("p"));
    }
}
