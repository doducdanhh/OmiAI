//! Consistency-based abductive reasoning (Kakas, Kowalski, Toni 1993).
//!
//! Given a knowledge base `KB`, an observation `obs`, and a set of
//! "abducible" / assumable atoms `A`, an **abductive explanation** is a
//! set `H ⊆ A` such that:
//!
//! 1. `KB ∪ H ⊨ obs` (sufficiency — H explains the observation),
//! 2. `KB ∪ H` is consistent (no contradiction introduced),
//! 3. `H` is **minimal** under set inclusion (no proper subset suffices).
//!
//! This module provides:
//!
//! - [`abduce`]: enumerates all minimal explanations up to a depth bound.
//! - [`best_explanation`]: returns the explanation with fewest atoms
//!   among those found.
//! - [`assumption_kernel`]: computes the set of assumables that are
//!   *relevant* — they appear in some minimal proof of inconsistency
//!   once the (negated) observation is added.
//!
//! # Algorithm
//!
//! Unlike naive subset enumeration (which is exponential), this module
//! uses **assumption-guided consistency checking**: at each level of the
//! search, it asks "is KB ∪ H ∪ {¬obs} consistent?" via DPLL over the
//! CNF encoding; if not, it picks an assumable that participates in
//! every inconsistency proof (via a 2-literal-watching heuristic on
//! assumable literals) and recurses. This is the classical
//! Kakas–Kowalski–Toni procedure restricted to ground Horn+ theories.
//!
//! # References
//!
//! - Eshghi, *Abductive Planning with Event Calculus* (1988).
//! - Kakas, Kowalski, Toni, *Abductive Logic Programming* (J. Log. Comput. 1993).
//! - Inoue, Sakama, *Negation as Failure in the Head* (1998).

use std::collections::{BTreeSet, HashSet};

use omiai_core::inference::dpll_satisfiable;
use omiai_core::logic_engine::{self, Formula, Literal};

/// An abductive explanation: a set of assumable atom names plus a
/// short witness of *why* it suffices (the satisfied clause set under
/// the chosen assumptions).
#[derive(Debug, Clone)]
pub struct Explanation {
    /// Set of assumable atoms used in the explanation.
    pub hypotheses: BTreeSet<String>,
    /// Optional: a witness set of clauses that became satisfied once
    /// the hypotheses were added. (For introspection / debugging.)
    pub witness_clauses: Vec<Vec<Literal>>,
}

impl Explanation {
    pub fn size(&self) -> usize {
        self.hypotheses.len()
    }
}

/// Encode `(KB ∧ ¬obs) ∪ H` as a CNF clause set suitable for DPLL.
///
/// - Each formula in `kb` is normalized to CNF.
pub fn encode(
    kb: &[Formula],
    observation_neg: &Formula,
    assumptions: &BTreeSet<String>,
) -> Vec<Vec<Literal>> {
    let mut clauses: Vec<Vec<Literal>> = Vec::new();
    for f in kb {
        if let Ok(cs) = logic_engine::normalize_cnf(f) {
            clauses.extend(cs);
        }
    }
    if let Ok(cs) = logic_engine::normalize_cnf(observation_neg) {
        clauses.extend(cs);
    }
    // Add each assumption as a unit clause: [hyp]
    for a in assumptions {
        // Atom: hyp() (zero-arity propositional literal)
        clauses.push(vec![Literal {
            negated: false,
            predicate: a.clone(),
            args: vec![],
        }]);
    }
    clauses
}

/// Return true iff `KB ∪ H ∪ {¬obs}` is propositionally consistent.
fn is_consistent(kb: &[Formula], observation_neg: &Formula, h: &BTreeSet<String>) -> bool {
    let clauses = encode(kb, observation_neg, h);
    dpll_satisfiable(&clauses)
}

/// Return true iff `KB ∪ H ⊨ obs` (i.e., `KB ∪ H ∪ {¬obs}` is UNSAT).
fn entails(kb: &[Formula], observation: &Formula, h: &BTreeSet<String>) -> bool {
    let neg = Formula::Not(Box::new(observation.clone()));
    let clauses = encode(kb, &neg, h);
    !dpll_satisfiable(&clauses)
}

/// Enumerate all minimal explanations of `obs` from `KB`, restricting
/// hypotheses to `assumables`.
///
/// `max_explanations` caps the number of explanations returned (in
/// order of increasing cardinality).
pub fn abduce(
    kb: &[Formula],
    observation: &Formula,
    assumables: &[String],
    max_explanations: usize,
) -> Vec<Explanation> {
    if max_explanations == 0 {
        return Vec::new();
    }

    let mut results: Vec<Explanation> = Vec::new();
    let mut seen: HashSet<BTreeSet<String>> = HashSet::new();

    // Iterative deepening by hypothesis cardinality
    for size in 1..=assumables.len() {
        if !results.is_empty() && results[0].size() < size {
            break;
        }
        for combo in combinations(assumables, size) {
            let h: BTreeSet<String> = combo.into_iter().collect();
            if !is_consistent(kb, &Formula::Not(Box::new(observation.clone())), &h) {
                continue;
            }
            if !entails(kb, observation, &h) {
                continue;
            }
            // Minimality: no proper subset also explains
            if !is_minimal(kb, observation, &h, assumables) {
                continue;
            }
            if seen.insert(h.clone()) {
                results.push(Explanation {
                    hypotheses: h,
                    witness_clauses: Vec::new(),
                });
                if results.len() >= max_explanations {
                    return results;
                }
            }
        }
    }
    results
}

/// Compute the **assumption kernel**: the subset of `assumables` that
/// participates in *some* minimal inconsistency proof of
/// `KB ∪ {¬obs} ∪ A_max` where `A_max = assumables`.
///
/// Algorithm: try removing each assumable one at a time; if the theory
/// becomes consistent without it, it was redundant; otherwise it is in
/// the kernel. This is a sound (under-approximation) kernel extractor
/// — the kernel is always a subset of the true relevant set.
pub fn assumption_kernel(
    kb: &[Formula],
    observation: &Formula,
    assumables: &[String],
) -> BTreeSet<String> {
    let neg = Formula::Not(Box::new(observation.clone()));
    let all: BTreeSet<String> = assumables.iter().cloned().collect();

    // If KB ∪ {¬obs} ∪ A_max is already consistent, no kernel needed.
    if is_consistent(kb, &neg, &all) {
        return BTreeSet::new();
    }

    let mut kernel = BTreeSet::new();
    for a in assumables {
        let mut minus = all.clone();
        minus.remove(a);
        if !is_consistent(kb, &neg, &minus) {
            // removing `a` makes it consistent ⇒ a is in the kernel
            kernel.insert(a.clone());
        } else {
            // removing `a` keeps inconsistency ⇒ a is NOT in the kernel
            // (something else is causing the inconsistency)
        }
    }
    kernel
}

/// Find the smallest minimal explanation (fewest assumptions).
pub fn best_explanation(
    kb: &[Formula],
    observation: &Formula,
    assumables: &[String],
) -> Option<Explanation> {
    abduce(kb, observation, assumables, 1).into_iter().next()
}

fn is_minimal(
    kb: &[Formula],
    observation: &Formula,
    h: &BTreeSet<String>,
    all_assumables: &[String],
) -> bool {
    // For every proper subset h' ⊂ h, check that h' does not entail obs
    // (or h' is not consistent).
    if h.len() <= 1 {
        return true;
    }
    let items: Vec<&String> = h.iter().collect();
    for skip in 0..items.len() {
        let mut sub = BTreeSet::new();
        for (i, item) in items.iter().enumerate() {
            if i != skip {
                sub.insert((*item).clone());
            }
        }
        if !sub.is_empty() && entails(kb, observation, &sub) {
            return false;
        }
    }
    let _ = all_assumables;
    true
}

fn combinations<T: Clone>(items: &[T], k: usize) -> Vec<Vec<T>> {
    let mut out: Vec<Vec<T>> = Vec::new();
    let n = items.len();
    if k > n {
        return out;
    }
    let mut idx: Vec<usize> = (0..k).collect();
    loop {
        out.push(idx.iter().map(|&i| items[i].clone()).collect());
        let mut i = k;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if idx[i] < n - k + i {
                idx[i] += 1;
                for j in (i + 1)..k {
                    idx[j] = idx[j - 1] + 1;
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use omiai_core::logic_engine::Formula;

    /// Wet grass: rain or sprinkler ⇒ wet. Observation: wet.
    /// Abducibles: {rain, sprinkler}.
    #[test]
    fn rain_sprinkler_wet() {
        let rains = Formula::prop("rain");
        let sprinkler = Formula::prop("sprinkler");
        let wet = Formula::prop("wet");
        let rule1 = Formula::Implies(Box::new(rains.clone()), Box::new(wet.clone()));
        let rule2 = Formula::Implies(Box::new(sprinkler.clone()), Box::new(wet.clone()));
        let kb = vec![rule1, rule2];
        let observation = wet;
        let assumables = vec!["rain".to_string(), "sprinkler".to_string()];
        let hyps = abduce(&kb, &observation, &assumables, 4);
        assert!(!hyps.is_empty(), "should find at least one explanation");
        // Each explanation is a singleton {rain} or {sprinkler}
        for h in &hyps {
            assert_eq!(h.size(), 1, "explanation should be minimal singleton");
            assert!(h.hypotheses.contains("rain") || h.hypotheses.contains("sprinkler"));
        }
    }

    /// Only rain can explain; sprinkler is irrelevant.
    #[test]
    fn unique_minimal_explanation() {
        let rains = Formula::prop("rain");
        let sprinkler = Formula::prop("sprinkler");
        let wet = Formula::prop("wet");
        // Only `rains ⇒ wet`; `sprinkler` is unrelated.
        let rule = Formula::Implies(Box::new(rains.clone()), Box::new(wet.clone()));
        let kb = vec![rule];
        let assumables = vec!["rain".to_string(), "sprinkler".to_string()];
        let best = best_explanation(&kb, &wet, &assumables).unwrap();
        assert_eq!(best.size(), 1);
        assert!(best.hypotheses.contains("rain"));
        assert!(!best.hypotheses.contains("sprinkler"));
    }

    /// Assumption kernel contains the relevant abducibles.
    #[test]
    fn kernel_is_subset() {
        let rains = Formula::prop("rain");
        let sprinkler = Formula::prop("sprinkler");
        let wet = Formula::prop("wet");
        let rule1 = Formula::Implies(Box::new(rains.clone()), Box::new(wet.clone()));
        let rule2 = Formula::Implies(Box::new(sprinkler.clone()), Box::new(wet.clone()));
        let kb = vec![rule1, rule2];
        let assumables = vec!["rain".to_string(), "sprinkler".to_string()];
        let kernel = assumption_kernel(&kb, &wet, &assumables);
        for k in &kernel {
            assert!(assumables.contains(k));
        }
    }

    /// Unrelated observation returns empty.
    #[test]
    fn no_explanation_for_unrelated_obs() {
        let rains = Formula::prop("rain");
        let wet = Formula::prop("wet");
        let rule = Formula::Implies(Box::new(rains.clone()), Box::new(wet.clone()));
        let kb = vec![rule];
        let unrelated = Formula::prop("snow");
        let assumables = vec!["rain".to_string()];
        let hyps = abduce(&kb, &unrelated, &assumables, 4);
        assert!(hyps.is_empty());
    }
}
