//! Directed Acyclic Graphs for causal models: ancestor queries and
//! d-separation (Pearl).

use std::collections::{HashMap, HashSet, VecDeque};

/// Causal DAG over named variables.
#[derive(Debug, Clone, Default)]
pub struct CausalDag {
    /// adjacency: parent → children
    pub children: HashMap<String, Vec<String>>,
    /// reverse: child → parents
    pub parents: HashMap<String, Vec<String>>,
}

impl CausalDag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, name: impl Into<String>) {
        let n = name.into();
        self.children.entry(n.clone()).or_default();
        self.parents.entry(n).or_default();
    }

    /// Add directed edge `from → to` (from causes to).
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        let f = from.into();
        let t = to.into();
        self.add_node(f.clone());
        self.add_node(t.clone());
        self.children.get_mut(&f).unwrap().push(t.clone());
        self.parents.get_mut(&t).unwrap().push(f);
    }

    /// All ancestors of `node` (strict).
    pub fn ancestors(&self, node: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut q = VecDeque::new();
        if let Some(ps) = self.parents.get(node) {
            for p in ps {
                q.push_back(p.clone());
            }
        }
        while let Some(cur) = q.pop_front() {
            if out.insert(cur.clone()) {
                if let Some(ps) = self.parents.get(&cur) {
                    for p in ps {
                        q.push_back(p.clone());
                    }
                }
            }
        }
        out
    }

    /// All descendants of `node` (strict).
    pub fn descendants(&self, node: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut q = VecDeque::new();
        if let Some(cs) = self.children.get(node) {
            for c in cs {
                q.push_back(c.clone());
            }
        }
        while let Some(cur) = q.pop_front() {
            if out.insert(cur.clone()) {
                if let Some(cs) = self.children.get(&cur) {
                    for c in cs {
                        q.push_back(c.clone());
                    }
                }
            }
        }
        out
    }

    /// d-separation: returns true if `x` and `y` are d-separated by `z`.
    ///
    /// Uses the moralized-ancestral-graph / Bayes-ball algorithm.
    pub fn d_separated(&self, x: &str, y: &str, z: &HashSet<String>) -> bool {
        !self.d_connected(x, y, z)
    }

    /// Bayes-ball: true if there is an active path from x to y given Z.
    pub fn d_connected(&self, x: &str, y: &str, z: &HashSet<String>) -> bool {
        // Shachter's Bayes-ball algorithm
        // State: (node, direction) direction: true = from_child (going up), false = from_parent (going down)
        let mut visited: HashSet<(String, bool)> = HashSet::new();
        let mut queue: VecDeque<(String, bool)> = VecDeque::new();
        queue.push_back((x.to_string(), true));
        queue.push_back((x.to_string(), false));

        let conditioned: HashSet<String> = z.clone();
        // Nodes with a descendant in Z (for collider opening)
        let mut has_desc_in_z: HashMap<String, bool> = HashMap::new();
        for node in self.parents.keys() {
            let descs = self.descendants(node);
            let hit = conditioned.contains(node.as_str())
                || descs.iter().any(|d| conditioned.contains(d));
            has_desc_in_z.insert(node.clone(), hit);
        }

        while let Some((node, from_child)) = queue.pop_front() {
            if node == y {
                return true;
            }
            if !visited.insert((node.clone(), from_child)) {
                continue;
            }
            let is_cond = conditioned.contains(&node);

            if from_child {
                // arrived from a child (going upward along an edge)
                if !is_cond {
                    // continue to parents and other children
                    if let Some(ps) = self.parents.get(&node) {
                        for p in ps {
                            queue.push_back((p.clone(), true)); // still going up
                        }
                    }
                    if let Some(cs) = self.children.get(&node) {
                        for c in cs {
                            queue.push_back((c.clone(), false)); // going down
                        }
                    }
                }
            } else {
                // arrived from a parent (going downward)
                if is_cond {
                    // blocked as chain/fork unless... only colliders open when conditioned
                    // going down into conditioned non-collider is blocked — do nothing
                } else {
                    // pass to children
                    if let Some(cs) = self.children.get(&node) {
                        for c in cs {
                            queue.push_back((c.clone(), false));
                        }
                    }
                }
                // Collider case: if we arrive at a collider that is conditioned
                // (or has desc in Z), we can go up to its parents — handled when
                // from_child on the collider...
                // When arriving down at node, also try parents if node is collider-activated
                if has_desc_in_z.get(&node).copied().unwrap_or(false) {
                    if let Some(ps) = self.parents.get(&node) {
                        for p in ps {
                            queue.push_back((p.clone(), true));
                        }
                    }
                }
            }
        }
        false
    }

    /// Topological order (Kahn). Returns None if a cycle exists.
    pub fn topological_order(&self) -> Option<Vec<String>> {
        let mut indeg: HashMap<String, usize> = HashMap::new();
        for n in self.parents.keys() {
            indeg.insert(n.clone(), self.parents[n].len());
        }
        let mut q: VecDeque<String> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| k.clone())
            .collect();
        let mut order = Vec::new();
        while let Some(n) = q.pop_front() {
            order.push(n.clone());
            if let Some(cs) = self.children.get(&n) {
                for c in cs {
                    let e = indeg.get_mut(c).unwrap();
                    *e -= 1;
                    if *e == 0 {
                        q.push_back(c.clone());
                    }
                }
            }
        }
        if order.len() == indeg.len() {
            Some(order)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_d_separation() {
        // X → M → Y
        let mut g = CausalDag::new();
        g.add_edge("X", "M");
        g.add_edge("M", "Y");
        let empty = HashSet::new();
        assert!(g.d_connected("X", "Y", &empty));
        let mut z = HashSet::new();
        z.insert("M".into());
        assert!(g.d_separated("X", "Y", &z));
    }

    #[test]
    fn ancestors_work() {
        let mut g = CausalDag::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        let a = g.ancestors("C");
        assert!(a.contains("A") && a.contains("B"));
    }
}
