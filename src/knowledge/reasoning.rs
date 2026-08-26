//! Forward chaining, backward chaining, and abductive reasoning over
//! Horn-like rules, plus a Structure-Mapping Engine (SME) sketch for
//! analogical matching.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

/// A Horn clause: `head ← body₁ ∧ … ∧ bodyₙ`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HornRule {
    pub head: String,
    pub body: Vec<String>,
}

/// Forward chaining: compute the least fixpoint of the immediate-consequence
/// operator over a set of facts and Horn rules.
pub fn forward_chain(facts: &HashSet<String>, rules: &[HornRule]) -> HashSet<String> {
    let mut known = facts.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for rule in rules {
            if rule.body.iter().all(|b| known.contains(b)) && known.insert(rule.head.clone()) {
                changed = true;
            }
        }
    }
    known
}

/// Backward chaining (SLD-style) for a ground goal against Horn rules + facts.
///
/// Returns `true` if the goal can be proved.
pub fn backward_chain(goal: &str, facts: &HashSet<String>, rules: &[HornRule]) -> bool {
    let mut visited = HashSet::new();
    bc_rec(goal, facts, rules, &mut visited)
}

fn bc_rec(
    goal: &str,
    facts: &HashSet<String>,
    rules: &[HornRule],
    visited: &mut HashSet<String>,
) -> bool {
    if facts.contains(goal) {
        return true;
    }
    if !visited.insert(goal.to_string()) {
        return false; // cycle
    }
    for rule in rules {
        if rule.head == goal {
            if rule.body.iter().all(|b| bc_rec(b, facts, rules, visited)) {
                return true;
            }
        }
    }
    false
}

/// Abductive reasoning: find minimal sets of assumptions that, together with
/// background knowledge, entail the observation (set-cover style).
///
/// Returns ranked hypotheses (assumption atoms), cheapest first (fewest atoms).
pub fn abduct(
    observation: &str,
    facts: &HashSet<String>,
    rules: &[HornRule],
    assumables: &[String],
) -> Vec<HashSet<String>> {
    // If already entailed, empty hypothesis
    if backward_chain(observation, facts, rules) {
        return vec![HashSet::new()];
    }

    let mut results = Vec::new();
    let n = assumables.len();
    if n > 16 {
        // Limit combinatorial explosion
        return results;
    }
    // Enumerate subsets by size
    for size in 1..=n {
        for mask in 0..(1usize << n) {
            if mask.count_ones() as usize != size {
                continue;
            }
            let mut hyp = HashSet::new();
            let mut extended = facts.clone();
            for (i, a) in assumables.iter().enumerate() {
                if (mask >> i) & 1 == 1 {
                    hyp.insert(a.clone());
                    extended.insert(a.clone());
                }
            }
            if backward_chain(observation, &extended, rules) {
                results.push(hyp);
            }
        }
        if !results.is_empty() {
            break; // minimal size only
        }
    }
    results
}

/// Relational structure for SME-style analogy.
#[derive(Debug, Clone)]
pub struct RelationalStructure {
    pub name: String,
    /// Predicates: name → argument entity ids
    pub relations: Vec<(String, Vec<String>)>,
}

/// Structure-Mapping Engine (Gentner): prefer systematic (deeply nested)
/// relational mappings between base and target.
///
/// Returns a mapping entity_base → entity_target scored by systematicity.
pub fn structure_map(
    base: &RelationalStructure,
    target: &RelationalStructure,
) -> HashMap<String, String> {
    // Collect entities
    let mut base_ents = HashSet::new();
    let mut target_ents = HashSet::new();
    for (_, args) in &base.relations {
        for a in args {
            base_ents.insert(a.clone());
        }
    }
    for (_, args) in &target.relations {
        for a in args {
            target_ents.insert(a.clone());
        }
    }

    // Score matchings of identical relation symbols
    let mut best_map = HashMap::new();
    let mut best_score = -1i32;

    // Greedy: for each base relation, find best matching target relation
    let mut used_target_rels = HashSet::new();
    let mut mapping = HashMap::new();
    let mut score = 0i32;

    for (i, (pred, b_args)) in base.relations.iter().enumerate() {
        let mut best_j = None;
        let mut best_local = -1i32;
        for (j, (tpred, t_args)) in target.relations.iter().enumerate() {
            if used_target_rels.contains(&j) || pred != tpred || b_args.len() != t_args.len() {
                continue;
            }
            let mut local = 1; // predicate match
            let mut ok = true;
            let mut trial = mapping.clone();
            for (ba, ta) in b_args.iter().zip(t_args.iter()) {
                if let Some(existing) = trial.get(ba) {
                    if existing != ta {
                        ok = false;
                        break;
                    }
                } else {
                    trial.insert(ba.clone(), ta.clone());
                    local += 1;
                }
            }
            if ok && local > best_local {
                best_local = local;
                best_j = Some((j, trial));
            }
        }
        if let Some((j, trial)) = best_j {
            used_target_rels.insert(j);
            mapping = trial;
            score += best_local;
            let _ = i;
        }
    }

    if score > best_score {
        best_score = score;
        best_map = mapping;
    }
    let _ = best_score;
    best_map
}

/// Queue-based production system step (for meta-cognition hooks).
pub fn fire_productions(
    facts: &mut HashSet<String>,
    rules: &[HornRule],
    max_firings: usize,
) -> usize {
    let mut firings = 0;
    let mut queue: VecDeque<usize> = (0..rules.len()).collect();
    while let Some(ri) = queue.pop_front() {
        if firings >= max_firings {
            break;
        }
        let rule = &rules[ri];
        if rule.body.iter().all(|b| facts.contains(b)) && facts.insert(rule.head.clone()) {
            firings += 1;
            // re-queue all rules
            queue.extend(0..rules.len());
        }
    }
    firings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_derives_mortal() {
        let mut facts = HashSet::new();
        facts.insert("human_socrates".into());
        let rules = vec![HornRule {
            head: "mortal_socrates".into(),
            body: vec!["human_socrates".into()],
        }];
        let derived = forward_chain(&facts, &rules);
        assert!(derived.contains("mortal_socrates"));
    }

    #[test]
    fn backward_proves_goal() {
        let mut facts = HashSet::new();
        facts.insert("a".into());
        let rules = vec![
            HornRule {
                head: "c".into(),
                body: vec!["b".into()],
            },
            HornRule {
                head: "b".into(),
                body: vec!["a".into()],
            },
        ];
        assert!(backward_chain("c", &facts, &rules));
    }

    #[test]
    fn abduct_finds_minimal() {
        let facts = HashSet::new();
        let rules = vec![HornRule {
            head: "wet".into(),
            body: vec!["rain".into()],
        }];
        let hyps = abduct("wet", &facts, &rules, &["rain".into(), "sprinkler".into()]);
        assert!(!hyps.is_empty());
        assert!(hyps[0].contains("rain"));
    }
}
