//! Junction Tree algorithm for exact inference in Bayesian networks.
//!
//! Provides polynomial-time (exponential in the graph's treewidth, not in
//! the total variable count) inference via:
//!
//! 1. **Moralization** — connect all parents of each node to make the
//!    graph undirected and triangulated-friendly.
//! 2. **Triangulation** — eliminate nodes in a fill-minimizing order so
//!    the moralized graph becomes chordal.
//! 3. **Clique identification** — find the maximal cliques of the
//!    triangulated graph.
//! 4. **Junction tree construction** — connect cliques into a tree
//!    satisfying the **running intersection property**.
//! 5. **Potential initialization** — assign each CPT to a containing clique.
//! 6. **Two-pass message passing** — `collect` (toward root) then
//!    `distribute` (from root) with sum-product.
//! 7. **Query** — marginalize the clique that contains the query variable.
//!
//! Like [`super::bayesian`], this implementation assumes Bernoulli
//! (binary) variables for tractability; the extension to discrete
//! variables of arbitrary cardinality is straightforward.
//!
//! # References
//!
//! - Cowell, Dawid, Lauritzen, Spiegelhalter,
//!   *Probabilistic Networks and Expert Systems* (1999).
//! - Jensen, *Bayesian Networks and Decision Graphs* (2001).

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use super::bayesian::{BayesianNetwork, Cpt};

// ---------------------------------------------------------------------------
// Potential: a non-negative function over an ordered set of Bernoulli vars.
// ---------------------------------------------------------------------------

/// A potential table over an ordered set of variable names.
/// `data[mask]` = value where bit `i` of `mask` is the value of `vars[i]`.
#[derive(Debug, Clone)]
pub struct Potential {
    pub vars: Vec<String>,
    pub data: Vec<f64>,
}

impl Potential {
    /// Uniform potential over the given variables (value 1 everywhere).
    pub fn uniform(vars: Vec<String>) -> Self {
        let n = 1usize << vars.len();
        Self {
            vars,
            data: vec![1.0; n],
        }
    }

    /// Direct construction (caller ensures `data.len() == 1 << vars.len()`).
    pub fn new(vars: Vec<String>, data: Vec<f64>) -> Self {
        debug_assert_eq!(data.len(), 1usize << vars.len());
        Self { vars, data }
    }

    /// Pointwise product with another potential. Variables are the union;
    /// missing entries in either factor default to 1.
    pub fn multiply(&self, other: &Potential) -> Potential {
        self.combine(other, |a, b| a * b)
    }

    /// Pointwise division by another potential (variables union; missing
    /// entries in denominator default to 1, so they're a no-op). Zeros in
    /// the denominator yield `1.0` to avoid panics (sum-product uses
    /// division only when previous message was positive).
    pub fn divide(&self, other: &Potential) -> Potential {
        self.combine(other, |a, b| if b.abs() < 1e-300 { 1.0 } else { a / b })
    }

    /// Generic elementwise combine over the union of variables.
    ///
    /// For each variable `v` in the union, takes `a = self.get_at(v)` and
    /// `b = other.get_at(v)` and accumulates `v *= f(a, b)` into the
    /// running product. `f` is applied per-variable, not pairwise across
    /// variables (which is what sum-product marginalization needs).
    fn combine<F: Fn(f64, f64) -> f64>(&self, other: &Potential, f: F) -> Potential {
        let combined: Vec<String> = {
            let mut seen = HashSet::new();
            self.vars
                .iter()
                .chain(other.vars.iter())
                .filter(|v| seen.insert((*v).clone()))
                .cloned()
                .collect()
        };
        let n = 1usize << combined.len();
        let mut data = vec![1.0f64; n];
        for mask in 0..n {
            let mut v = 1.0;
            for var in &combined {
                let a = self.get_at(var, mask, &combined);
                let b = other.get_at(var, mask, &combined);
                v *= f(a, b);
            }
            data[mask] = v;
        }
        Potential {
            vars: combined,
            data,
        }
    }

    /// Marginalize (sum) out a subset of variables.
    pub fn marginalize(&self, keep: &[String]) -> Potential {
        let mut keep_idx: Vec<usize> = Vec::new();
        for v in keep {
            if let Some(i) = self.vars.iter().position(|x| x == v) {
                keep_idx.push(i);
            }
        }
        let keep_vars: Vec<String> = keep_idx.iter().map(|&i| self.vars[i].clone()).collect();
        let new_n = 1usize << keep_vars.len();
        let mut new_data = vec![0.0f64; new_n];
        for mask in 0..(1usize << self.vars.len()) {
            let mut k = 0usize;
            for (i, &ki) in keep_idx.iter().enumerate() {
                if (mask >> ki) & 1 == 1 {
                    k |= 1 << i;
                }
            }
            new_data[k] += self.data[mask];
        }
        Potential {
            vars: keep_vars,
            data: new_data,
        }
    }

    /// Look up the value of `var` at the combined-mask `mask`, returning
    /// 1.0 if `var` is not in `self.vars`.
    fn get_at(&self, var: &str, mask: usize, combined: &[String]) -> f64 {
        let Some(combined_i) = combined.iter().position(|x| x == var) else {
            return 1.0;
        };
        let bit = (mask >> combined_i) & 1 == 1;
        if !self.vars.contains(&var.to_string()) {
            return 1.0;
        }
        let local_i = self.vars.iter().position(|x| x == var).unwrap();
        let local_mask = if bit { 1 << local_i } else { 0 };
        self.data[local_mask]
    }

    /// Probability table for a single variable, normalizing to sum=1.
    pub fn normalize(&self) -> Potential {
        let z: f64 = self.data.iter().sum();
        if z <= 0.0 {
            return self.clone();
        }
        let data = self.data.iter().map(|v| v / z).collect();
        Potential {
            vars: self.vars.clone(),
            data,
        }
    }
}

// ---------------------------------------------------------------------------
// Junction Tree data structure
// ---------------------------------------------------------------------------

/// A clique in the junction tree: a maximal subset of variables with
/// an attached potential.
#[derive(Debug, Clone)]
pub struct Clique {
    pub id: usize,
    pub vars: Vec<String>,
    pub potential: Potential,
}

/// Separator between two adjacent cliques: the intersection of their
/// variables, with a potential for message passing.
#[derive(Debug, Clone)]
pub struct Separator {
    pub a: usize,
    pub b: usize,
    pub vars: Vec<String>,
    pub potential: Potential,
}

/// A junction tree of cliques with separators.
#[derive(Debug, Clone)]
pub struct JunctionTree {
    pub cliques: Vec<Clique>,
    pub separators: Vec<Separator>,
    /// Adjacency: clique id → neighbor clique ids.
    pub adjacency: HashMap<usize, Vec<usize>>,
}

impl JunctionTree {
    /// Build a junction tree from a Bayesian network using greedy
    /// fill-minimizing elimination.
    pub fn from_network(bn: &BayesianNetwork) -> Self {
        let vars: Vec<String> = bn.nodes.iter().map(|n| n.variable.clone()).collect();

        // Step 1: moral graph (undirected, parents connected).
        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
        for v in &vars {
            adj.entry(v.clone()).or_default();
        }
        for cpt in &bn.nodes {
            for p in &cpt.parents {
                if p != &cpt.variable {
                    adj.entry(cpt.variable.clone())
                        .or_default()
                        .insert(p.clone());
                    adj.entry(p.clone())
                        .or_default()
                        .insert(cpt.variable.clone());
                }
            }
            // Moralize: connect all parents of cpt.variable.
            for i in 0..cpt.parents.len() {
                for j in (i + 1)..cpt.parents.len() {
                    let a = cpt.parents[i].clone();
                    let b = cpt.parents[j].clone();
                    adj.entry(a.clone()).or_default().insert(b.clone());
                    adj.entry(b).or_default().insert(a);
                }
            }
        }

        // Step 2: greedy fill-minimizing triangulation. Repeatedly pick
        // the variable that introduces the fewest fill edges.
        let mut order: Vec<String> = Vec::new();
        let mut remaining: BTreeSet<String> = vars.iter().cloned().collect();
        let mut fill_adj = adj.clone();
        while !remaining.is_empty() {
            // Pick node with min fill in current graph
            let mut best: Option<(usize, &String)> = None;
            for v in &remaining {
                let nbrs: Vec<String> = fill_adj
                    .get(v)
                    .map(|s| s.iter().cloned().collect())
                    .unwrap_or_default();
                let mut fill = 0usize;
                for i in 0..nbrs.len() {
                    for j in (i + 1)..nbrs.len() {
                        let a = &nbrs[i];
                        let b = &nbrs[j];
                        if !fill_adj.get(a).map(|s| s.contains(b)).unwrap_or(false) {
                            fill += 1;
                        }
                    }
                }
                if best.is_none() || fill < best.unwrap().0 {
                    best = Some((fill, v));
                }
            }
            let (_, v) = best.unwrap();
            let v_owned = v.clone();
            order.push(v_owned.clone());
            // Connect all neighbors of v into a clique (add fill edges)
            let nbrs: Vec<String> = fill_adj
                .get(&v_owned)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            for i in 0..nbrs.len() {
                for j in (i + 1)..nbrs.len() {
                    let a = nbrs[i].clone();
                    let b = nbrs[j].clone();
                    fill_adj.entry(a.clone()).or_default().insert(b.clone());
                    fill_adj.entry(b).or_default().insert(a);
                }
            }
            // Remove v
            remaining.remove(&v_owned);
            fill_adj.remove(&v_owned);
            for nbrs_set in fill_adj.values_mut() {
                nbrs_set.remove(&v_owned);
            }
        }

        // Step 3: maximal cliques. Each time we eliminate a node v,
        // the elimination step creates a clique on v ∪ neighbors(v)
        // (in the fill graph after v's neighbors have been connected).
        let mut cliques: Vec<Vec<String>> = Vec::new();
        let mut state_adj = adj.clone();
        for v in &order {
            let nbrs: Vec<String> = state_adj
                .get(v)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            // connect all pairs of nbrs in state_adj
            for i in 0..nbrs.len() {
                for j in (i + 1)..nbrs.len() {
                    let a = nbrs[i].clone();
                    let b = nbrs[j].clone();
                    state_adj.entry(a.clone()).or_default().insert(b.clone());
                    state_adj.entry(b).or_default().insert(a);
                }
            }
            let mut clique = nbrs;
            clique.push(v.clone());
            clique.sort();
            if !cliques.iter().any(|c| c == &clique) {
                cliques.push(clique);
            }
            // remove v
            state_adj.remove(v);
            for nbrs_set in state_adj.values_mut() {
                nbrs_set.remove(v);
            }
        }

        // Step 4: build clique graph, find MST with maximum-weight spanning
        // forest (cliques with larger intersections get stronger edges).
        let mut edges: Vec<(usize, usize, Vec<String>)> = Vec::new();
        for i in 0..cliques.len() {
            for j in (i + 1)..cliques.len() {
                let a: HashSet<&String> = cliques[i].iter().collect();
                let b: HashSet<&String> = cliques[j].iter().collect();
                let inter: Vec<String> = a.intersection(&b).cloned().cloned().collect();
                if !inter.is_empty() {
                    edges.push((i, j, inter));
                }
            }
        }
        // Kruskal
        edges.sort_by(|x, y| y.2.len().cmp(&x.2.len()));
        let mut parent: Vec<usize> = (0..cliques.len()).collect();
        fn find(parent: &mut Vec<usize>, x: usize) -> usize {
            if parent[x] != x {
                parent[x] = find(parent, parent[x]);
            }
            parent[x]
        }
        let mut tree_edges: Vec<(usize, usize, Vec<String>)> = Vec::new();
        for (i, j, inter) in edges {
            let ri = find(&mut parent, i);
            let rj = find(&mut parent, j);
            if ri != rj {
                parent[ri] = rj;
                tree_edges.push((i, j, inter));
            }
        }

        let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut separators: Vec<Separator> = Vec::new();
        for &(i, j, ref inter) in &tree_edges {
            adjacency.entry(i).or_default().push(j);
            adjacency.entry(j).or_default().push(i);
            let pot = Potential::uniform(inter.clone());
            separators.push(Separator {
                a: i,
                b: j,
                vars: inter.clone(),
                potential: pot,
            });
        }
        for i in 0..cliques.len() {
            adjacency.entry(i).or_default();
        }

        // Step 5: initialize cliques with CPT products. For each CPT,
        // find a clique that contains all of its variables (CPT vars
        // were used to form cliques during elimination).
        let mut clique_objs: Vec<Clique> = cliques
            .iter()
            .enumerate()
            .map(|(id, vars)| Clique {
                id,
                vars: vars.clone(),
                potential: Potential::uniform(vars.clone()),
            })
            .collect();

        for cpt in &bn.nodes {
            let mut target: Option<usize> = None;
            for c in &clique_objs {
                let s: HashSet<&String> = c.vars.iter().collect();
                let mut cpt_vars = std::iter::once(&cpt.variable).chain(cpt.parents.iter());
                if cpt_vars.all(|v| s.contains(v)) {
                    target = Some(c.id);
                    break;
                }
            }
            let Some(target) = target else {
                continue;
            };
            let cp_pot = cpt_to_potential(cpt);
            clique_objs[target].potential = clique_objs[target].potential.multiply(&cp_pot);
        }

        JunctionTree {
            cliques: clique_objs,
            separators,
            adjacency,
        }
    }

    /// Find the clique containing `var`.
    pub fn find_clique(&self, var: &str) -> Option<usize> {
        self.cliques
            .iter()
            .find(|c| c.vars.iter().any(|v| v == var))
            .map(|c| c.id)
    }

    /// Pick an arbitrary root (clique with smallest id) for two-pass.
    pub fn root(&self) -> usize {
        self.cliques.iter().map(|c| c.id).min().unwrap_or(0)
    }

    /// Run the **collect** phase: messages flow from leaves toward `root`.
    pub fn collect(&mut self, root: usize) {
        let mut parent: HashMap<usize, usize> = HashMap::new();
        let mut order: Vec<usize> = Vec::new();
        let mut stack: Vec<(usize, Option<usize>)> = vec![(root, None)];
        while let Some((u, p)) = stack.pop() {
            parent.insert(u, p.unwrap_or(u));
            order.push(u);
            if let Some(nbrs) = self.adjacency.get(&u) {
                for &v in nbrs {
                    if Some(v) != p {
                        stack.push((v, Some(u)));
                    }
                }
            }
        }
        // Post-order traversal
        order.reverse();
        for &u in &order {
            if u == root {
                continue;
            }
            let p = parent[&u];
            self.send_message(u, p);
        }
    }

    /// Run the **distribute** phase: messages flow from `root` outward.
    pub fn distribute(&mut self, root: usize) {
        let mut queue: VecDeque<usize> = VecDeque::new();
        queue.push_back(root);
        let mut visited: HashSet<usize> = HashSet::new();
        visited.insert(root);
        let mut parent: HashMap<usize, usize> = HashMap::new();
        while let Some(u) = queue.pop_front() {
            if let Some(nbrs) = self.adjacency.get(&u).cloned() {
                for v in nbrs {
                    if !visited.contains(&v) {
                        visited.insert(v);
                        parent.insert(v, u);
                        self.send_message(u, v);
                        queue.push_back(v);
                    }
                }
            }
        }
        let _ = parent; // suppress warning
    }

    /// Run both passes for a fully calibrated tree.
    pub fn calibrate(&mut self) {
        let r = self.root();
        self.collect(r);
        self.distribute(r);
    }

    /// Send a message from `from` to `to`: marginalize `from`'s potential
    /// onto the separator variables and store into the separator.
    fn send_message(&mut self, from: usize, to: usize) {
        // Find separator and capture the OLD message before overwriting it
        // (sum-product requires dividing by the old message).
        let sep_idx = self
            .separators
            .iter()
            .position(|s| (s.a == from && s.b == to) || (s.a == to && s.b == from));
        let Some(sep_idx) = sep_idx else {
            return;
        };
        let sep_vars = self.separators[sep_idx].vars.clone();
        let old_msg = self.separators[sep_idx].potential.clone();
        let had_old = old_msg.data.iter().any(|v| *v > 0.0);

        // Marginalize the source clique onto the separator variables.
        let from_pot = self.cliques[from].potential.clone();
        let new_msg = from_pot.marginalize(&sep_vars);
        self.separators[sep_idx].potential = new_msg.clone();

        // Update the destination clique: divide by old, multiply by new.
        let to_pot = self.cliques[to].potential.clone();
        let updated = if had_old {
            to_pot.divide(&old_msg).multiply(&new_msg)
        } else {
            to_pot.multiply(&new_msg)
        };
        self.cliques[to].potential = updated;
    }

    fn old_message_for_clique(&self, from: usize, to: usize) -> Option<Potential> {
        for sep in &self.separators {
            if (sep.a == from && sep.b == to) || (sep.a == to && sep.b == from) {
                return Some(sep.potential.clone());
            }
        }
        None
    }

    /// Return P(var=true | evidence) using the calibrated tree.
    pub fn query(&self, var: &str, evidence: &HashMap<String, bool>) -> Option<f64> {
        let clique_id = self.find_clique(var)?;
        let clique = &self.cliques[clique_id];
        let mut p = clique.potential.clone();
        for (e_var, e_val) in evidence {
            if p.vars.iter().any(|v| v == e_var) {
                p = restrict(&p, e_var, *e_val);
            }
        }
        let keep = vec![var.to_string()];
        let marginal = p.marginalize(&keep).normalize();
        // data[1] is var=true (since vars = [var])
        Some(marginal.data[1])
    }
}

/// Restrict a potential: collapse one variable to a fixed value.
fn restrict(pot: &Potential, var: &str, val: bool) -> Potential {
    let Some(i) = pot.vars.iter().position(|v| v == var) else {
        return pot.clone();
    };
    let bit = if val { 1 } else { 0 };
    let new_vars: Vec<String> = pot
        .vars
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != i)
        .map(|(_, v)| v.clone())
        .collect();
    let new_n = 1usize << new_vars.len();
    let mut new_data = vec![0.0f64; new_n];
    for mask in 0..(1usize << pot.vars.len()) {
        if (mask >> i) & 1 == bit {
            // compress mask: remove bit i
            let mut new_mask = 0usize;
            for (k, _) in new_vars.iter().enumerate() {
                let orig_j = if k < i { k } else { k + 1 };
                if (mask >> orig_j) & 1 == 1 {
                    new_mask |= 1 << k;
                }
            }
            new_data[new_mask] += pot.data[mask];
        }
    }
    Potential {
        vars: new_vars,
        data: new_data,
    }
}

/// Convert a CPT to a Potential over its full variable set.
fn cpt_to_potential(cpt: &Cpt) -> Potential {
    let mut vars: Vec<String> = cpt.parents.clone();
    vars.push(cpt.variable.clone());
    let n = 1usize << vars.len();
    let mut data = vec![1.0f64; n];
    for mask in 0..n {
        // mask layout: bits 0..len(parents) are parents; bit len(parents) is variable
        let parent_mask = mask & ((1 << cpt.parents.len()) - 1);
        let var_bit = (mask >> cpt.parents.len()) & 1;
        let pt = cpt.probs_true.get(parent_mask).copied().unwrap_or(0.5);
        let p = if var_bit == 1 { pt } else { 1.0 - pt };
        data[mask] = p;
    }
    Potential::new(vars, data)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn alarm_network() -> BayesianNetwork {
        let mut bn = BayesianNetwork::new();
        bn.add_node(Cpt {
            variable: "B".into(),
            parents: vec![],
            probs_true: vec![0.001],
        });
        bn.add_node(Cpt {
            variable: "E".into(),
            parents: vec![],
            probs_true: vec![0.002],
        });
        bn.add_node(Cpt {
            variable: "A".into(),
            parents: vec!["B".into(), "E".into()],
            probs_true: vec![0.001, 0.29, 0.94, 0.95],
        });
        bn.add_node(Cpt {
            variable: "J".into(),
            parents: vec!["A".into()],
            probs_true: vec![0.05, 0.90],
        });
        bn.add_node(Cpt {
            variable: "M".into(),
            parents: vec!["A".into()],
            probs_true: vec![0.01, 0.70],
        });
        bn
    }

    #[test]
    fn build_junction_tree_alarm() {
        let bn = alarm_network();
        let jt = JunctionTree::from_network(&bn);
        assert!(!jt.cliques.is_empty(), "junction tree must have cliques");
        assert!(
            !jt.separators.is_empty(),
            "junction tree must have separators"
        );
    }

    #[test]
    fn calibrate_and_query_alarm() {
        let bn = alarm_network();
        let mut jt = JunctionTree::from_network(&bn);
        jt.calibrate();
        let mut ev = HashMap::new();
        ev.insert("J".into(), true);
        ev.insert("M".into(), true);
        let p_b = jt.query("B", &ev).expect("B is in tree");
        // P(Burglary | JohnCalls, MaryCalls) should be ~0.28 in the canonical example
        assert!(p_b > 0.0 && p_b < 1.0);
        let p_a = jt.query("A", &ev).expect("A is in tree");
        // P(Alarm | John, Mary) should be high
        assert!(p_a > 0.5, "P(A|J,M) = {p_a} should be > 0.5");
    }

    #[test]
    fn potential_multiply_and_marginalize() {
        // Simple chain P(A,B) = P(A) * P(B|A)
        let p_a = Potential::new(vec!["A".into()], vec![0.3, 0.7]);
        let p_b_given_a = Potential::new(vec!["A".into(), "B".into()], vec![0.8, 0.2, 0.1, 0.9]);
        let joint = p_a.multiply(&p_b_given_a);
        assert_eq!(joint.vars.len(), 2);
        let marg = joint.marginalize(&["B".into()]);
        // Should sum to 1
        let s: f64 = marg.data.iter().sum();
        assert!((s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn junction_tree_agrees_with_brute_force() {
        let mut bn = BayesianNetwork::new();
        bn.add_node(Cpt {
            variable: "X".into(),
            parents: vec![],
            probs_true: vec![0.4],
        });
        bn.add_node(Cpt {
            variable: "Y".into(),
            parents: vec!["X".into()],
            probs_true: vec![0.2, 0.7],
        });
        // Brute force
        let mut ev = HashMap::new();
        ev.insert("Y".into(), true);
        let p_brute = bn.variable_elimination("X", &ev);
        // Junction tree
        let mut jt = JunctionTree::from_network(&bn);
        jt.calibrate();
        let p_jt = jt.query("X", &ev).expect("query X");
        assert!(
            (p_brute - p_jt).abs() < 1e-6,
            "JT diverged from brute force: {p_jt} vs {p_brute}"
        );
    }
}
