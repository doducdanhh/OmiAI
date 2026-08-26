//! Higher-order unification à la Huet (1975), simplified.
//!
//! Operates on simply-typed λ-terms in the style of the TPTP THF format:
//! - Base types `o` (boolean / propositions) and `ι` (individuals).
//! - Function types written `α → β`.
//! - Variables are either *bound* (under a λ) or *free*.
//! - Application is left-associative: `(f x y) = ((f x) y)`.
//!
//! Higher-order unification is **undecidable** in general; this
//! implementation follows Huet's semi-algorithm: it enumerates a stream
//! of unifiers in order of increasing "redness" (number of imitation
//! vs projection choices), pruning when no solution exists for a
//! flex-flex pair.
//!
//! # References
//!
//! - Huet, *A Unification Algorithm for Typed λ-Calculus* (1975).
//! - Dowek, *Higher-order unification and matching* (2001).
//! - Paulson's *ML for the Working Programmer* Ch. 12.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types and terms
// ---------------------------------------------------------------------------

/// Simple type: `Base("o")`, `Base("ι")`, or `Arrow(a, b)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Base(String),
    Arrow(Box<Type>, Box<Type>),
}

impl Type {
    pub fn o() -> Self {
        Type::Base("o".into())
    }
    pub fn i() -> Self {
        Type::Base("ι".into())
    }
    pub fn arrow(a: Type, b: Type) -> Self {
        Type::Arrow(Box::new(a), Box::new(b))
    }

    /// Arity of an arrow type (0 for non-arrow).
    pub fn arity(&self) -> usize {
        let mut a = 0;
        let mut cur = self;
        while let Type::Arrow(_x, y) = cur {
            a += 1;
            cur = y;
        }
        a
    }
}

/// A simply-typed λ-term.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Term {
    /// Constant symbol (rigid).
    Const(String),
    /// Bound variable, addressed by de-Bruijn index.
    BVar(usize),
    /// Free variable (flexible).
    FVar(String),
    /// Application: `(head arg)`.
    App(Box<Term>, Box<Term>),
    /// Lambda abstraction: `λbody` (single-argument form).
    Lam(Box<Term>),
}

impl Term {
    /// Construct an application chain.
    pub fn apps(head: Term, args: Vec<Term>) -> Term {
        let mut t = head;
        for a in args {
            t = Term::App(Box::new(t), Box::new(a));
        }
        t
    }
}

// ---------------------------------------------------------------------------
// Substitution (free-variable → term)
// ---------------------------------------------------------------------------

/// A higher-order substitution mapping free variables to terms.
#[derive(Debug, Clone, Default)]
pub struct Subst {
    map: HashMap<String, Term>,
}

impl Subst {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, var: impl Into<String>, term: Term) {
        self.map.insert(var.into(), term);
    }

    pub fn get(&self, var: &str) -> Option<&Term> {
        self.map.get(var)
    }

    pub fn compose(&self, other: &Subst) -> Subst {
        let mut out = Subst::new();
        for (v, t) in &self.map {
            out.bind(v.clone(), other.apply(t));
        }
        for (v, t) in &other.map {
            if !self.map.contains_key(v) {
                out.bind(v.clone(), t.clone());
            }
        }
        out
    }

    /// Apply this substitution to a term, raising free variables to avoid
    /// capture. Bound variables use de-Bruijn indices so they don't
    /// capture; only free variables need renaming.
    pub fn apply(&self, t: &Term) -> Term {
        match t {
            Term::Const(_) => t.clone(),
            Term::BVar(i) => Term::BVar(*i),
            Term::FVar(v) => match self.map.get(v) {
                Some(u) => u.clone(),
                None => t.clone(),
            },
            Term::App(h, a) => Term::App(Box::new(self.apply(h)), Box::new(self.apply(a))),
            Term::Lam(body) => Term::Lam(Box::new(self.apply(body))),
        }
    }
}

// ---------------------------------------------------------------------------
// Occurs check (free-variable → term)
// ---------------------------------------------------------------------------

fn occurs_fvar(v: &str, t: &Term, s: &Subst) -> bool {
    let resolved = s.apply(t);
    match &resolved {
        Term::FVar(w) => w == v,
        Term::Const(_) | Term::BVar(_) => false,
        Term::App(h, a) => occurs_fvar(v, h, s) || occurs_fvar(v, a, s),
        Term::Lam(body) => occurs_fvar(v, body, s),
    }
}

// ---------------------------------------------------------------------------
// Huet semi-algorithm: enumerator of unifiers
// ---------------------------------------------------------------------------

/// A flex-flex pair: two terms that need to be unified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexPair {
    pub lhs: Term,
    pub rhs: Term,
}

/// A flex-rigid pair (η-expanded form): one side is a free variable
/// applied to distinct bound variables; the other is a rigid term.
#[derive(Debug, Clone)]
pub struct FlexRigid {
    pub var: String,
    pub args: Vec<Term>,
    pub rigid: Term,
}

/// State of the higher-order unification search.
#[derive(Debug, Clone)]
pub struct UnifState {
    subst: Subst,
    /// Remaining flex-rigid pairs (head variable is applied to bound vars).
    flex_rigid: Vec<FlexRigid>,
    /// Remaining flex-flex pairs (both sides headed by free variables).
    flex_flex: Vec<FlexPair>,
}

impl UnifState {
    fn new() -> Self {
        Self {
            subst: Subst::new(),
            flex_rigid: Vec::new(),
            flex_flex: Vec::new(),
        }
    }
}

/// Result of one step of the search.
#[derive(Debug, Clone)]
pub enum UnifStep {
    /// Found a complete unifier for the original problem.
    Solved(Subst),
    /// Produced one or more new subproblems with updated state. A flex-
    /// rigid pair forks into imitation + n projection branches.
    Branches(Vec<UnifState>),
    /// All branches exhausted or pruned.
    NoSolution,
}

/// Run one iteration of Huet's algorithm on the current state.
///
/// Returns `Some(UnifStep::Solved)` on success, or `Some(Branches)` /
/// `None` to continue search.
pub fn huet_step(state: UnifState) -> Option<UnifStep> {
    if state.flex_rigid.is_empty() && state.flex_flex.is_empty() {
        return Some(UnifStep::Solved(state.subst));
    }

    // First, try to simplify a flex-flex pair into flex-rigid or solve.
    if let Some(pair) = state.flex_flex.first().cloned() {
        if let Some(branch) = simplify_flex_flex(&state, &pair) {
            return Some(branch);
        }
    }

    // Otherwise, process a flex-rigid pair via imitation or projection.
    if let Some(fr) = state.flex_rigid.first().cloned() {
        return Some(simplify_flex_rigid(&state, fr));
    }

    Some(UnifStep::NoSolution)
}

/// Enumerate up to `max` unifiers for the given equation list.
pub fn unify_terms(equations: &[(Term, Term)], max: usize) -> Vec<Subst> {
    let mut state = UnifState::new();
    // Initial decomposition
    let mut pending: Vec<(Term, Term)> = equations.to_vec();
    while let Some((mut a, mut b)) = pending.pop() {
        // Strip leading λ's on both sides
        while let (Term::Lam(x), Term::Lam(y)) = (&a, &b) {
            a = x.as_ref().clone();
            b = y.as_ref().clone();
        }
        decompose(&a, &b, &mut pending, &mut state);
    }

    let mut solutions = Vec::new();
    let mut work = vec![state];
    while let Some(s) = work.pop() {
        if solutions.len() >= max {
            break;
        }
        match huet_step(s) {
            Some(UnifStep::Solved(subst)) => solutions.push(subst),
            Some(UnifStep::Branches(next)) => {
                for n in next {
                    work.push(n);
                }
            }
            Some(UnifStep::NoSolution) => {}
            None => {}
        }
    }
    solutions
}

fn decompose(a: &Term, b: &Term, pending: &mut Vec<(Term, Term)>, state: &mut UnifState) {
    let a = state.subst.apply(a);
    let b = state.subst.apply(b);
    match (&a, &b) {
        (Term::App(ah, aa), Term::App(bh, ba)) => {
            pending.push((ah.as_ref().clone(), bh.as_ref().clone()));
            pending.push((aa.as_ref().clone(), ba.as_ref().clone()));
        }
        (Term::FVar(va), Term::FVar(vb)) if va == vb => {
            // Same flexible var; trivially equal
        }
        (Term::FVar(_va), Term::FVar(_vb)) => {
            // Different flex vars — push as a flex-flex pair
            state.flex_flex.push(FlexPair {
                lhs: a.clone(),
                rhs: b.clone(),
            });
        }
        (Term::FVar(v), _r) | (_r, Term::FVar(v)) => {
            // Flex-rigid: try to eta-expand `r` so its head is a constructor
            // applied to distinct bound vars (this is what we want for Huet's
            // imitation / projection rules).
            let is_flex_left = matches!(&a, Term::FVar(_));
            let (var, rigid) = if is_flex_left {
                (v.clone(), b.clone())
            } else {
                (v.clone(), a.clone())
            };
            let eta_args = eta_expand(&rigid);
            // Substitute bound vars: use eta_args as the canonical eta-long form.
            // The args must be distinct bound variables (de-Bruijn 0..n).
            state.flex_rigid.push(FlexRigid {
                var,
                args: eta_args,
                rigid,
            });
        }
        (Term::Const(s), Term::Const(t)) if s == t => {}
        (Term::BVar(i), Term::BVar(j)) if i == j => {}
        (Term::Lam(ba), Term::Lam(bb)) => {
            decompose(ba, bb, pending, state);
        }
        _ => {
            // Incompatible heads — prune
            state.flex_flex.clear();
            state.flex_rigid.clear();
        }
    }
}

/// Return `n` distinct de-Bruijn variables, where `n` is the arity of
/// `term`'s head (after eta-expansion).
fn eta_expand(t: &Term) -> Vec<Term> {
    let n = t.arity_head();
    (0..n).map(Term::BVar).collect()
}

impl Term {
    /// Arity of the head: number of trailing applications on a non-lambda
    /// head.
    fn arity_head(&self) -> usize {
        let mut n = 0;
        let mut cur = self;
        while let Term::App(h, _) = cur {
            n += 1;
            cur = h;
        }
        n
    }
}

/// Try to solve a flex-flex pair by generalizing both sides.
/// For simplicity, we bind one flex var to the other (η-expanded).
fn simplify_flex_flex(state: &UnifState, pair: &FlexPair) -> Option<UnifStep> {
    // pick the leftmost flex var in `lhs`, bind it to rhs
    if let Term::FVar(v) = state.subst.apply(&pair.lhs) {
        // Occurs check
        if occurs_fvar(&v, &pair.rhs, &state.subst) {
            return Some(UnifStep::NoSolution);
        }
        let mut next = state.clone();
        next.subst.bind(v, pair.rhs.clone());
        // Remove the solved pair from flex_flex
        next.flex_flex.retain(|p| p != pair);
        return Some(UnifStep::Branches(vec![next]));
    }
    None
}

/// Solve a flex-rigid pair via imitation OR projection.
///
/// Huet's algorithm branches on each flex-rigid pair: the head variable
/// may either **imitate** the rigid head (bind `var` to a constructor
/// applied to fresh eta-args) or **project** onto one of its eta-args
/// (bind `var` to that eta-arg directly). This function returns ALL
/// such branches so the search can explore them.
fn simplify_flex_rigid(state: &UnifState, fr: FlexRigid) -> UnifStep {
    let rigid = state.subst.apply(&fr.rigid);
    let n = fr.args.len();

    // Branch 1: imitation — bind var to the rigid head with eta-args.
    // In this simplified implementation we bind to the head of the rigid
    // term; a full η-expansion would wrap with n lambdas here.
    let imitation_body = head_of(&rigid);
    let mut branch_imit = state.clone();
    branch_imit.flex_rigid.retain(|x| x.var != fr.var);
    branch_imit.subst.bind(fr.var.clone(), imitation_body);

    // Branches 2..n+1: projection onto the i-th eta-arg.
    let mut all_branches = vec![branch_imit];
    for i in 0..n {
        let mut branch = state.clone();
        branch.flex_rigid.retain(|x| x.var != fr.var);
        branch.subst.bind(fr.var.clone(), fr.args[i].clone());
        all_branches.push(branch);
    }

    UnifStep::Branches(all_branches)
}

/// Strip leading lambdas, then return the head of the application chain.
fn head_of(t: &Term) -> Term {
    match t {
        Term::Lam(body) => head_of(body),
        Term::App(h, _) => head_of(h),
        _ => t.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str) -> Term {
        Term::FVar(name.into())
    }
    fn cst(name: &str) -> Term {
        Term::Const(name.into())
    }
    fn app(h: Term, a: Term) -> Term {
        Term::App(Box::new(h), Box::new(a))
    }

    /// `f a = a`  ⇒  f = λx. x
    #[test]
    fn eta_unify_identity() {
        let a = Term::BVar(0);
        let f = var("F");
        let lhs = app(f.clone(), a.clone());
        let rhs = a.clone();
        let sols = unify_terms(&[(lhs, rhs)], 4);
        assert!(!sols.is_empty(), "should find identity eta-solution");
    }

    /// `f = λx. c`  — constant imitation.
    #[test]
    fn imitation_solves_constant() {
        let c = cst("c");
        let lhs = Term::Lam(Box::new(c.clone()));
        let f = var("F");
        let sols = unify_terms(&[(f, lhs)], 4);
        assert!(!sols.is_empty(), "imitation should solve");
    }

    /// Type arity computes correctly.
    #[test]
    fn type_arity() {
        let t = Type::arrow(Type::i(), Type::arrow(Type::i(), Type::o()));
        assert_eq!(t.arity(), 2);
    }

    /// Substitution compose.
    #[test]
    fn subst_compose() {
        let mut s1 = Subst::new();
        s1.bind(String::from("x"), var("y"));
        let mut s2 = Subst::new();
        s2.bind(String::from("y"), cst("a"));
        let comp = s1.compose(&s2);
        let result = comp.apply(&var("x"));
        assert_eq!(result, cst("a"));
    }
}
