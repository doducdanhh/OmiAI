//! Modal logic K: Kripke semantics, model checking, and validity.
//!
//! Modal logic extends propositional logic with two dual operators:
//!
//! - `□ φ` (necessity): φ holds in **every** accessible world
//! - `◇ φ` (possibility): φ holds in **some** accessible world
//!
//! ## Semantics (Kripke)
//!
//! A Kripke structure is a triple `M = (W, R, V)`:
//! - `W`: a non-empty set of **worlds**.
//! - `R ⊆ W × W`: a binary **accessibility relation**.
//! - `V : W → 2^Prop`: a **valuation** mapping each world to the set of
//!   propositional atoms true there.
//!
//! Satisfaction is defined recursively:
//!
//! ```text
//!   M, w ⊨ p            iff  p ∈ V(w)
//!   M, w ⊨ ¬φ           iff  M, w ⊭ φ
//!   M, w ⊨ φ ∧ ψ        iff  M, w ⊨ φ  and  M, w ⊨ ψ
//!   M, w ⊨ □φ            iff  for all v with w R v, M, v ⊨ φ
//!   M, w ⊨ ◇φ            iff  exists v with w R v, M, v ⊨ φ
//! ```
//!
//! Modal logic **K** has no extra axioms beyond these rules (S4 adds
//! reflexivity, S5 adds equivalence, etc.).
//!
//! # What this module provides
//!
//! - [`KripkeStructure`]: build a finite Kripke structure.
//! - [`satisfies`]: check `M, w ⊨ φ` (model checking).
//! - [`is_valid`]: check whether φ is valid in **all** worlds of **all**
//!   finite Kripke structures up to size `n` (a small-model validity test).
//!
//! # References
//!
//! - Hughes & Cresswell, *A New Introduction to Modal Logic* (1996).
//! - Blackburn, de Rijke, Venema, *Modal Logic* (CUP, 2001).

use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Modal formula AST
// ---------------------------------------------------------------------------

/// Modal formula over a finite set of propositional atoms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModalFormula {
    True_,
    False_,
    Atom(String),
    Not(Box<ModalFormula>),
    And(Box<ModalFormula>, Box<ModalFormula>),
    Or(Box<ModalFormula>, Box<ModalFormula>),
    Implies(Box<ModalFormula>, Box<ModalFormula>),
    /// □ (necessity): holds in all accessible worlds.
    Box(Box<ModalFormula>),
    /// ◇ (possibility): holds in some accessible world.
    Diamond(Box<ModalFormula>),
}

impl ModalFormula {
    pub fn atom(s: impl Into<String>) -> Self {
        ModalFormula::Atom(s.into())
    }
    /// Negation constructor (named `neg` to avoid clashing with `std::ops::Not`).
    #[allow(clippy::should_implement_trait)]
    pub fn neg(f: ModalFormula) -> Self {
        ModalFormula::Not(Box::new(f))
    }
    pub fn and(a: ModalFormula, b: ModalFormula) -> Self {
        ModalFormula::And(Box::new(a), Box::new(b))
    }
    pub fn or(a: ModalFormula, b: ModalFormula) -> Self {
        ModalFormula::Or(Box::new(a), Box::new(b))
    }
    pub fn implies(a: ModalFormula, b: ModalFormula) -> Self {
        ModalFormula::Implies(Box::new(a), Box::new(b))
    }
    pub fn box_(f: ModalFormula) -> Self {
        ModalFormula::Box(Box::new(f))
    }
    pub fn diamond(f: ModalFormula) -> Self {
        ModalFormula::Diamond(Box::new(f))
    }

    /// Collect all atomic propositions in the formula.
    pub fn atoms(&self) -> HashSet<String> {
        let mut s = HashSet::new();
        collect_atoms(self, &mut s);
        s
    }
}

fn collect_atoms(f: &ModalFormula, out: &mut HashSet<String>) {
    match f {
        ModalFormula::True_ | ModalFormula::False_ => {}
        ModalFormula::Atom(s) => {
            out.insert(s.clone());
        }
        ModalFormula::Not(g) => collect_atoms(g, out),
        ModalFormula::And(a, b) | ModalFormula::Or(a, b) | ModalFormula::Implies(a, b) => {
            collect_atoms(a, out);
            collect_atoms(b, out);
        }
        ModalFormula::Box(g) | ModalFormula::Diamond(g) => collect_atoms(g, out),
    }
}

// ---------------------------------------------------------------------------
// Kripke structure
// ---------------------------------------------------------------------------

/// A finite Kripke structure: worlds, accessibility relation, and valuation.
#[derive(Debug, Clone)]
pub struct KripkeStructure {
    /// World IDs (typically 0..n).
    pub worlds: Vec<usize>,
    /// Accessibility relation: `(from, to)`.
    pub accessible: HashSet<(usize, usize)>,
    /// Valuation: world → set of true atoms.
    pub valuation: HashMap<usize, HashSet<String>>,
}

impl KripkeStructure {
    pub fn new(worlds: Vec<usize>) -> Self {
        let mut valuation = HashMap::new();
        for w in &worlds {
            valuation.insert(*w, HashSet::new());
        }
        Self {
            worlds,
            accessible: HashSet::new(),
            valuation,
        }
    }

    pub fn add_access(&mut self, from: usize, to: usize) {
        self.accessible.insert((from, to));
    }

    pub fn set_true(&mut self, world: usize, atom: impl Into<String>) {
        self.valuation.entry(world).or_default().insert(atom.into());
    }

    pub fn set_false(&mut self, world: usize, atom: impl Into<String>) {
        self.valuation
            .entry(world)
            .or_default()
            .remove(&atom.into());
    }

    /// Successors of `world` under `R`.
    pub fn successors(&self, world: usize) -> Vec<usize> {
        self.accessible
            .iter()
            .filter(|(f, _)| *f == world)
            .map(|(_, t)| *t)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Model checking
// ---------------------------------------------------------------------------

/// Check `M, w ⊨ φ`.
pub fn satisfies(m: &KripkeStructure, world: usize, f: &ModalFormula) -> bool {
    match f {
        ModalFormula::True_ => true,
        ModalFormula::False_ => false,
        ModalFormula::Atom(s) => m
            .valuation
            .get(&world)
            .map(|vs| vs.contains(s))
            .unwrap_or(false),
        ModalFormula::Not(g) => !satisfies(m, world, g),
        ModalFormula::And(a, b) => satisfies(m, world, a) && satisfies(m, world, b),
        ModalFormula::Or(a, b) => satisfies(m, world, a) || satisfies(m, world, b),
        ModalFormula::Implies(a, b) => !satisfies(m, world, a) || satisfies(m, world, b),
        ModalFormula::Box(g) => m.successors(world).iter().all(|&w| satisfies(m, w, g)),
        ModalFormula::Diamond(g) => m.successors(world).iter().any(|&w| satisfies(m, w, g)),
    }
}

/// Check whether φ is satisfied in **every** world of `m`.
pub fn is_valid_in(m: &KripkeStructure, f: &ModalFormula) -> bool {
    m.worlds.iter().all(|&w| satisfies(m, w, f))
}

// ---------------------------------------------------------------------------
// Validity testing (small-model property)
// ---------------------------------------------------------------------------

/// Enumerate all Kripke structures over `n` worlds with all subsets of
/// atom valuations and the full power-set of the accessibility relation.
/// Check whether φ is valid in **all** such structures.
///
/// This is only feasible for tiny `n` (≤ 3) and few atoms (≤ 2), since
/// the number of structures grows as `2^(n²) · 2^(n·|atoms|)`.
pub fn is_valid_small(f: &ModalFormula, n: usize, atoms: &[String]) -> bool {
    // Enumerate all `R ⊆ W × W`
    let pairs: Vec<(usize, usize)> = (0..n).flat_map(|i| (0..n).map(move |j| (i, j))).collect();
    let n_pairs = pairs.len();

    for r_mask in 0..(1u64 << n_pairs) {
        let mut m = KripkeStructure::new((0..n).collect());
        for (k, (a, b)) in pairs.iter().enumerate() {
            if (r_mask >> k) & 1 == 1 {
                m.add_access(*a, *b);
            }
        }
        // Enumerate all valuations
        for v_mask in 0..(1u64 << (n * atoms.len())) {
            for w in 0..n {
                for (i, a) in atoms.iter().enumerate() {
                    let bit = (v_mask >> (w * atoms.len() + i)) & 1 == 1;
                    if bit {
                        m.set_true(w, a.clone());
                    } else {
                        m.set_false(w, a.clone());
                    }
                }
            }
            if !is_valid_in(&m, f) {
                return false;
            }
        }
    }
    true
}

/// Construct the canonical single-world structure with `atoms` all true.
pub fn trivial_model(atoms: &[String]) -> KripkeStructure {
    let mut m = KripkeStructure::new(vec![0]);
    for a in atoms {
        m.set_true(0, a.clone());
    }
    m
}

/// Construct a two-world model where world 0 accesses world 1.
pub fn two_world_model(atom: &str, w0_true: bool, w1_true: bool) -> KripkeStructure {
    let mut m = KripkeStructure::new(vec![0, 1]);
    m.add_access(0, 1);
    if w0_true {
        m.set_true(0, atom);
    }
    if w1_true {
        m.set_true(1, atom);
    }
    m
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_holds_in_true_world() {
        let m = trivial_model(&["p".into()]);
        assert!(satisfies(&m, 0, &ModalFormula::atom("p")));
    }

    #[test]
    fn not_atom_fails_in_true_world() {
        let m = trivial_model(&["p".into()]);
        assert!(!satisfies(
            &m,
            0,
            &ModalFormula::neg(ModalFormula::atom("p"))
        ));
    }

    #[test]
    fn box_is_vacuous_without_successors() {
        // In a single-world model with no outgoing edges, □φ holds for all φ.
        let m = trivial_model(&["p".into()]);
        assert!(satisfies(
            &m,
            0,
            &ModalFormula::box_(ModalFormula::atom("p"))
        ));
        assert!(satisfies(
            &m,
            0,
            &ModalFormula::box_(ModalFormula::neg(ModalFormula::atom("p")))
        ));
    }

    #[test]
    fn box_requires_all_successors() {
        // Two-world model: 0 → 1, only world 1 has p. Then □p should hold at 0.
        let m = two_world_model("p", false, true);
        assert!(satisfies(
            &m,
            0,
            &ModalFormula::box_(ModalFormula::atom("p"))
        ));
    }

    #[test]
    fn box_fails_when_one_successor_lacks_atoms() {
        let m = two_world_model("p", false, false);
        // □p at world 0 requires p at world 1, which fails.
        assert!(!satisfies(
            &m,
            0,
            &ModalFormula::box_(ModalFormula::atom("p"))
        ));
    }

    #[test]
    fn diamond_requires_some_successor() {
        let m = two_world_model("p", false, true);
        assert!(satisfies(
            &m,
            0,
            &ModalFormula::diamond(ModalFormula::atom("p"))
        ));
        let m2 = two_world_model("p", false, false);
        assert!(!satisfies(
            &m2,
            0,
            &ModalFormula::diamond(ModalFormula::atom("p"))
        ));
    }

    #[test]
    fn diamond_dual_of_box() {
        // ◇φ ≡ ¬□¬φ  (classical modal duality)
        let m = two_world_model("p", false, true);
        let diamond_p = ModalFormula::diamond(ModalFormula::atom("p"));
        let not_box_not_p = ModalFormula::neg(ModalFormula::box_(ModalFormula::neg(
            ModalFormula::atom("p"),
        )));
        assert_eq!(
            satisfies(&m, 0, &diamond_p),
            satisfies(&m, 0, &not_box_not_p)
        );
    }

    #[test]
    fn tautology_p_or_not_p_is_valid_in_any_model() {
        let f = ModalFormula::or(
            ModalFormula::atom("p"),
            ModalFormula::neg(ModalFormula::atom("p")),
        );
        let m = trivial_model(&["p".into()]);
        assert!(is_valid_in(&m, &f));
        let m2 = KripkeStructure::new(vec![0]);
        assert!(is_valid_in(&m2, &f));
    }

    #[test]
    fn small_validity_test_for_p_or_not_p() {
        let f = ModalFormula::or(
            ModalFormula::atom("p"),
            ModalFormula::neg(ModalFormula::atom("p")),
        );
        // Should be valid in all 1-world models
        assert!(is_valid_small(&f, 1, &["p".into()]));
    }

    #[test]
    fn small_validity_test_for_box_p_implies_p_fails() {
        // □p → p is NOT valid in K (it requires reflexivity, i.e., S4)
        let f = ModalFormula::implies(
            ModalFormula::box_(ModalFormula::atom("p")),
            ModalFormula::atom("p"),
        );
        let m = KripkeStructure::new(vec![0, 1]);
        // No accessibility, □p is vacuously true at 0, but p is false at 0.
        // Counter-model: □p holds (no successors), p fails.
        assert!(!is_valid_in(&m, &f));
        // So □p → p is not valid in K.
        assert!(!is_valid_small(&f, 1, &["p".into()]));
    }

    #[test]
    fn k_axiom_is_valid() {
        // K axiom: □(φ → ψ) → (□φ → □ψ)
        let phi = ModalFormula::atom("p");
        let psi = ModalFormula::atom("q");
        let k = ModalFormula::implies(
            ModalFormula::box_(ModalFormula::implies(phi.clone(), psi.clone())),
            ModalFormula::implies(
                ModalFormula::box_(phi.clone()),
                ModalFormula::box_(psi.clone()),
            ),
        );
        // Should be valid in all Kripke models
        assert!(is_valid_small(&k, 1, &["p".into(), "q".into()]));
    }

    #[test]
    fn atoms_collection() {
        let f = ModalFormula::box_(ModalFormula::diamond(ModalFormula::and(
            ModalFormula::atom("p"),
            ModalFormula::atom("q"),
        )));
        let atoms = f.atoms();
        assert_eq!(atoms.len(), 2);
        assert!(atoms.contains("p"));
        assert!(atoms.contains("q"));
    }
}
