//! Confounder detection via back-door paths in a causal DAG.

use std::collections::HashSet;

use super::dag::CausalDag;
use super::do_calculus::backdoor_criterion;

/// Find a minimal set of observed variables that blocks all back-door paths
/// from `treatment` to `outcome` (adjustment set).
///
/// Strategy: candidates = parents of treatment (classic heuristic); also try
/// empty set and all non-descendants.
pub fn find_adjustment_set(
    dag: &CausalDag,
    treatment: &str,
    outcome: &str,
    observed: &HashSet<String>,
) -> Option<HashSet<String>> {
    // Try empty
    let empty = HashSet::new();
    if backdoor_criterion(dag, treatment, outcome, &empty) {
        return Some(empty);
    }
    // Parents of treatment
    let parents: HashSet<String> = dag
        .parents
        .get(treatment)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| observed.contains(p))
        .collect();
    if backdoor_criterion(dag, treatment, outcome, &parents) {
        return Some(parents);
    }
    // All non-descendants of treatment that are observed
    let desc = dag.descendants(treatment);
    let candidates: HashSet<String> = observed
        .iter()
        .filter(|v| v.as_str() != treatment && v.as_str() != outcome && !desc.contains(v.as_str()))
        .cloned()
        .collect();
    if backdoor_criterion(dag, treatment, outcome, &candidates) {
        return Some(candidates);
    }
    None
}

/// Variables that lie on a back-door path (potential confounders).
pub fn potential_confounders(dag: &CausalDag, treatment: &str, outcome: &str) -> HashSet<String> {
    // Ancestors of treatment or outcome, excluding the pair themselves
    let mut conf = dag.ancestors(treatment);
    conf.extend(dag.ancestors(outcome));
    conf.remove(treatment);
    conf.remove(outcome);
    conf
}

/// Report whether unobserved confounding is possible given observed set.
pub fn has_unblocked_backdoor(
    dag: &CausalDag,
    treatment: &str,
    outcome: &str,
    observed: &HashSet<String>,
) -> bool {
    find_adjustment_set(dag, treatment, outcome, observed).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_parent_adjustment() {
        let mut g = CausalDag::new();
        g.add_edge("Z", "X");
        g.add_edge("X", "Y");
        g.add_edge("Z", "Y");
        let mut obs = HashSet::new();
        obs.insert("Z".into());
        obs.insert("X".into());
        obs.insert("Y".into());
        let adj = find_adjustment_set(&g, "X", "Y", &obs).unwrap();
        assert!(adj.contains("Z"));
    }
}
