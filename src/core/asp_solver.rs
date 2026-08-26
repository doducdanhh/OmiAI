//! Answer Set Programming under Gelfond–Lifschitz stable-model semantics.
//!
//! A normal logic program is a set of rules
//! ```text
//! head ← body⁺ ∧ not body⁻
//! ```
//! The **reduct** of program `P` w.r.t. candidate answer set `S` deletes
//! rules whose negative body intersects `S`, then drops negative literals.
//! `S` is stable iff it is the least Herbrand model of the reduct.

use std::collections::{BTreeSet, HashMap, HashSet};

/// A normal ASP rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub head: String,
    /// Positive body atoms.
    pub body_pos: Vec<String>,
    /// Negative body atoms (default negation).
    pub body_neg: Vec<String>,
}

/// A ground normal logic program.
#[derive(Debug, Clone, Default)]
pub struct Program {
    pub rules: Vec<Rule>,
}

impl Program {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Collect the Herbrand base (all atoms appearing in the program).
    pub fn herbrand_base(&self) -> BTreeSet<String> {
        let mut base = BTreeSet::new();
        for r in &self.rules {
            base.insert(r.head.clone());
            for a in &r.body_pos {
                base.insert(a.clone());
            }
            for a in &r.body_neg {
                base.insert(a.clone());
            }
        }
        base
    }

    /// Grounding over a finite Herbrand universe of constants.
    ///
    /// For already-ground programs this is the identity. Template rules
    /// containing `$X` style placeholders are expanded over `universe`.
    pub fn ground(&self, universe: &[String]) -> Program {
        if universe.is_empty() {
            return self.clone();
        }
        let mut grounded = Program::new();
        for rule in &self.rules {
            if needs_grounding(rule) {
                for c in universe {
                    grounded.rules.push(instantiate(rule, c));
                }
            } else {
                grounded.rules.push(rule.clone());
            }
        }
        grounded
    }

    /// Enumerate stable models by checking each subset of the Herbrand base
    /// (exponential — correct for small educational instances).
    pub fn stable_models(&self) -> Vec<BTreeSet<String>> {
        let base: Vec<String> = self.herbrand_base().into_iter().collect();
        let n = base.len();
        if n > 20 {
            // Safety: refuse combinatorial explosion; use heuristic search.
            return self.stable_models_heuristic();
        }
        let mut models = Vec::new();
        let total = 1usize << n;
        for mask in 0..total {
            let mut candidate = BTreeSet::new();
            for (i, atom) in base.iter().enumerate() {
                if (mask >> i) & 1 == 1 {
                    candidate.insert(atom.clone());
                }
            }
            if self.is_stable_model(&candidate) {
                models.push(candidate);
            }
        }
        models
    }

    /// Gelfond–Lifschitz check: S is stable iff least model of P^S equals S.
    pub fn is_stable_model(&self, s: &BTreeSet<String>) -> bool {
        let reduct = self.reduct(s);
        let least = least_model(&reduct);
        &least == s
    }

    /// Reduct P^S.
    pub fn reduct(&self, s: &BTreeSet<String>) -> Program {
        let mut out = Program::new();
        for r in &self.rules {
            // Drop rule if some negative body atom is in S
            if r.body_neg.iter().any(|a| s.contains(a)) {
                continue;
            }
            out.rules.push(Rule {
                head: r.head.clone(),
                body_pos: r.body_pos.clone(),
                body_neg: vec![], // drop negation
            });
        }
        out
    }

    fn stable_models_heuristic(&self) -> Vec<BTreeSet<String>> {
        // Greedy: start from empty, fixpoint of immediate consequence with
        // random flips — returns at most one approximate model.
        let mut s = BTreeSet::new();
        for _ in 0..100 {
            let reduct = self.reduct(&s);
            let next = least_model(&reduct);
            if next == s {
                if self.is_stable_model(&s) {
                    return vec![s];
                }
                break;
            }
            s = next;
        }
        vec![]
    }
}

/// Immediate-consequence operator least fixpoint (definite program).
fn least_model(program: &Program) -> BTreeSet<String> {
    let mut model = BTreeSet::new();
    loop {
        let mut added = false;
        for r in &program.rules {
            if r.body_pos.iter().all(|a| model.contains(a)) && model.insert(r.head.clone()) {
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    model
}

fn needs_grounding(rule: &Rule) -> bool {
    rule.head.contains('$')
        || rule.body_pos.iter().any(|a| a.contains('$'))
        || rule.body_neg.iter().any(|a| a.contains('$'))
}

fn instantiate(rule: &Rule, constant: &str) -> Rule {
    let sub = |s: &str| s.replace("$X", constant);
    Rule {
        head: sub(&rule.head),
        body_pos: rule.body_pos.iter().map(|a| sub(a)).collect(),
        body_neg: rule.body_neg.iter().map(|a| sub(a)).collect(),
    }
}

/// Convenience: atoms that appear only as facts (empty body).
pub fn facts(program: &Program) -> HashSet<String> {
    program
        .rules
        .iter()
        .filter(|r| r.body_pos.is_empty() && r.body_neg.is_empty())
        .map(|r| r.head.clone())
        .collect()
}

/// Indexed lookup of rules by head (for forward chaining integration).
pub fn index_by_head(program: &Program) -> HashMap<String, Vec<usize>> {
    let mut idx = HashMap::new();
    for (i, r) in program.rules.iter().enumerate() {
        idx.entry(r.head.clone()).or_insert_with(Vec::new).push(i);
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_stable_model() {
        // a ← not b.
        // b ← not a.
        // Two stable models: {a} and {b}
        let mut p = Program::new();
        p.add_rule(Rule {
            head: "a".into(),
            body_pos: vec![],
            body_neg: vec!["b".into()],
        });
        p.add_rule(Rule {
            head: "b".into(),
            body_pos: vec![],
            body_neg: vec!["a".into()],
        });
        let models = p.stable_models();
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|m| m.contains("a") && !m.contains("b")));
        assert!(models.iter().any(|m| m.contains("b") && !m.contains("a")));
    }

    #[test]
    fn fact_only_program() {
        let mut p = Program::new();
        p.add_rule(Rule {
            head: "p".into(),
            body_pos: vec![],
            body_neg: vec![],
        });
        let models = p.stable_models();
        assert_eq!(models.len(), 1);
        assert!(models[0].contains("p"));
    }
}
