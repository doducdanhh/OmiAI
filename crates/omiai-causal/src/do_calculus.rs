//! Pearl's do-calculus: back-door / front-door criteria and the three
//! rules for transforming interventional queries.

use std::collections::HashSet;

use super::dag::CausalDag;

/// Back-door criterion: Z satisfies the back-door criterion relative to
/// (X, Y) if:
/// 1. No node in Z is a descendant of X
/// 2. Z blocks every path from X to Y that enters X through an arrow into X
pub fn backdoor_criterion(dag: &CausalDag, x: &str, y: &str, z: &HashSet<String>) -> bool {
    // (1) no descendant of X in Z
    let desc_x = dag.descendants(x);
    if z.iter().any(|z_i| desc_x.contains(z_i) || z_i == x) {
        return false;
    }
    // (2) block all back-door paths: paths into X
    // Construct graph with edges out of X removed, check d-sep of X and Y given Z
    let mutilated = remove_outgoing(dag, x);
    mutilated.d_separated(x, y, z)
}

/// Front-door criterion: Z satisfies front-door relative to (X, Y) if:
/// 1. Z intercepts all directed paths from X to Y
/// 2. There is no back-door path from X to Z
/// 3. All back-door paths from Z to Y are blocked by X
pub fn frontdoor_criterion(dag: &CausalDag, x: &str, y: &str, z: &HashSet<String>) -> bool {
    if z.is_empty() {
        return false;
    }
    // (1) Z intercepts all directed paths X ↝ Y — every child chain goes through Z
    // Approximate: every child of X that can reach Y is in Z or routes through Z
    let children_x = dag.children.get(x).cloned().unwrap_or_default();
    for c in &children_x {
        if c == y {
            // direct edge X→Y not intercepted unless Z somehow... front-door fails
            return false;
        }
        // c must be in Z or all paths c→Y go through Z
        if !z.contains(c) {
            // check if every path from c to y hits z — approx: c not ancestor of y without z
            let mut blocked = false;
            for z_i in z {
                // if c reaches z_i and z_i reaches y
                if reaches(dag, c, z_i) && reaches(dag, z_i, y) {
                    blocked = true;
                }
            }
            if !blocked && reaches(dag, c, y) {
                return false;
            }
        }
    }
    // (2) no unblocked back-door X—Z
    for z_i in z {
        if !backdoor_criterion(dag, x, z_i, &HashSet::new()) {
            // empty set should block all back-doors (no back-door exists)
            // if backdoor_criterion with empty is false, either descendants or open path
            // For (2) we need: back-door criterion of {} for (X,Z) holds only if no back-doors
            // Actually: "no back-door path from X to Z" ≡ d-sep in G under empty after removing outgoing X...
            if !backdoor_criterion(dag, x, z_i, &HashSet::<String>::new()) {
                // if Z is descendant of X, backdoor fails for a different reason — that's OK for front-door
                let desc = dag.descendants(x);
                if !desc.contains(z_i) {
                    return false;
                }
            }
        }
    }
    // (3) X blocks all back-doors from Z to Y
    let mut xset = HashSet::new();
    xset.insert(x.to_string());
    for z_i in z {
        if !backdoor_criterion(dag, z_i, y, &xset) {
            return false;
        }
    }
    true
}

fn reaches(dag: &CausalDag, from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    dag.descendants(from).contains(to)
}

/// Graph mutilation: remove all edges out of `x` (do(x) surgery).
pub fn remove_outgoing(dag: &CausalDag, x: &str) -> CausalDag {
    let mut g = CausalDag::new();
    for (p, children) in &dag.children {
        for c in children {
            if p == x {
                continue; // delete X → *
            }
            g.add_edge(p.clone(), c.clone());
        }
    }
    // ensure all nodes exist
    for n in dag.parents.keys() {
        g.add_node(n.clone());
    }
    g
}

/// Graph mutilation: remove all edges into `x`.
pub fn remove_incoming(dag: &CausalDag, x: &str) -> CausalDag {
    let mut g = CausalDag::new();
    for (p, children) in &dag.children {
        for c in children {
            if c == x {
                continue; // delete * → X
            }
            g.add_edge(p.clone(), c.clone());
        }
    }
    for n in dag.parents.keys() {
        g.add_node(n.clone());
    }
    g
}

/// Do-calculus Rule 1: insertion/deletion of observations  
/// `P(y|do(x),z,w) = P(y|do(x),w)` if `(Y ⊥ Z | X,W)_{G_{\\bar{X}}}`
pub fn rule1_observation_deletion(
    dag: &CausalDag,
    y: &str,
    x: &str,
    z: &HashSet<String>,
    w: &HashSet<String>,
) -> bool {
    let g = remove_outgoing(dag, x);
    let mut cond = w.clone();
    cond.insert(x.to_string());
    // check Y d-sep Z given cond — for each z
    z.iter().all(|zi| g.d_separated(y, zi, &cond))
}

/// Rule 2: action/observation exchange  
/// `P(y|do(x),do(z),w) = P(y|do(x),z,w)` if `(Y ⊥ Z | X,W)_{G_{\\bar{X}\\underline{Z}}}`
pub fn rule2_action_observation_exchange(
    dag: &CausalDag,
    y: &str,
    x: &str,
    z: &str,
    w: &HashSet<String>,
) -> bool {
    let mut g = remove_outgoing(dag, x);
    g = remove_incoming(&g, z);
    let mut cond = w.clone();
    cond.insert(x.to_string());
    g.d_separated(y, z, &cond)
}

/// Rule 3: insertion/deletion of actions  
/// `P(y|do(x),do(z),w) = P(y|do(x),w)` if `(Y ⊥ Z | X,W)_{G_{\\bar{X}\\bar{Z(W)}}}`
pub fn rule3_action_deletion(
    dag: &CausalDag,
    y: &str,
    x: &str,
    z: &str,
    w: &HashSet<String>,
) -> bool {
    let mut g = remove_outgoing(dag, x);
    // Z(W) = Z-nodes that are not ancestors of any W-node in G_{\bar X}
    let ancestors_of_w: HashSet<String> = w
        .iter()
        .flat_map(|wi| {
            let mut a = g.ancestors(wi);
            a.insert(wi.clone());
            a
        })
        .collect();
    if !ancestors_of_w.contains(z) {
        g = remove_outgoing(&g, z);
    }
    let mut cond = w.clone();
    cond.insert(x.to_string());
    g.d_separated(y, z, &cond)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdoor_confounder() {
        // Z → X → Y, Z → Y
        let mut g = CausalDag::new();
        g.add_edge("Z", "X");
        g.add_edge("X", "Y");
        g.add_edge("Z", "Y");
        let mut z = HashSet::new();
        z.insert("Z".into());
        assert!(backdoor_criterion(&g, "X", "Y", &z));
        assert!(!backdoor_criterion(&g, "X", "Y", &HashSet::new()));
    }
}
