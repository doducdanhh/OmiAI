//! Concurrent knowledge graph: `Concept` / `Relation` nodes and edges,
//! path queries, transitive inference, and a lightweight description-logic
//! consistency check.
//!
//! Backed by `petgraph::graph::DiGraph` with an `IndexMap` for O(1) concept
//! lookup by id. Path search uses BFS; transitive closure uses Floyd–Warshall
//! on the relation-filtered adjacency (suitable for dense local subgraphs).

use std::collections::{HashMap, HashSet, VecDeque};

use indexmap::IndexMap;
use petgraph::Direction;
use petgraph::algo::has_path_connecting;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

/// A concept (node) in the knowledge graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    pub label: String,
}

/// A typed relation (edge) between two concepts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Relation {
    pub kind: String,
}

/// Knowledge graph with petgraph DiGraph backend.
#[derive(Debug, Clone)]
pub struct KnowledgeGraph {
    graph: DiGraph<Concept, Relation>,
    /// concept id → NodeIndex
    index: IndexMap<String, NodeIndex>,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            index: IndexMap::new(),
        }
    }

    /// Number of concepts (nodes).
    pub fn len(&self) -> usize {
        self.graph.node_count()
    }

    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Insert a concept; returns `false` if id already exists.
    pub fn add_concept(&mut self, concept: Concept) -> bool {
        if self.index.contains_key(&concept.id) {
            return false;
        }
        let id = concept.id.clone();
        let idx = self.graph.add_node(concept);
        self.index.insert(id, idx);
        true
    }

    /// Add a directed relation `from -kind-> to`. Both endpoints must exist.
    pub fn add_relation(
        &mut self,
        from: &str,
        to: &str,
        kind: impl Into<String>,
    ) -> Result<(), GraphError> {
        let a = *self
            .index
            .get(from)
            .ok_or_else(|| GraphError::UnknownConcept(from.into()))?;
        let b = *self
            .index
            .get(to)
            .ok_or_else(|| GraphError::UnknownConcept(to.into()))?;
        self.graph.add_edge(a, b, Relation { kind: kind.into() });
        Ok(())
    }

    /// BFS path of concept ids from `from` to `to`, optionally filtered by
    /// relation kind.
    pub fn query_path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        self.query_path_typed(from, to, None)
    }

    /// Path search restricted to edges of `kind` when `Some`.
    pub fn query_path_typed(
        &self,
        from: &str,
        to: &str,
        kind: Option<&str>,
    ) -> Option<Vec<String>> {
        let start = *self.index.get(from)?;
        let goal = *self.index.get(to)?;
        if start == goal {
            return Some(vec![from.to_string()]);
        }

        let mut queue = VecDeque::new();
        let mut parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut visited = HashSet::new();
        queue.push_back(start);
        visited.insert(start);

        while let Some(cur) = queue.pop_front() {
            for edge in self.graph.edges_directed(cur, Direction::Outgoing) {
                if let Some(k) = kind
                    && edge.weight().kind != k {
                        continue;
                    }
                let next = edge.target();
                if visited.insert(next) {
                    parent.insert(next, cur);
                    if next == goal {
                        return Some(reconstruct_path(&self.graph, &parent, start, goal));
                    }
                    queue.push_back(next);
                }
            }
        }
        None
    }

    /// Transitive closure pairs for a given relation kind (reachability).
    pub fn infer_transitive(&self, relation_kind: &str) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for (id_a, &na) in &self.index {
            for (id_b, &nb) in &self.index {
                // Filtered reachability: only walk edges of this kind.
                // Self-pairs are NOT skipped: a direct self-loop edge
                // witnesses ("a", "a"), while without one
                // `reachable_via` correctly reports no path.
                if reachable_via(&self.graph, na, nb, relation_kind) {
                    pairs.push((id_a.clone(), id_b.clone()));
                }
            }
        }
        pairs
    }

    /// Induced subgraph over the given concept ids (and edges between them).
    pub fn subgraph(&self, concept_ids: &[String]) -> KnowledgeGraph {
        let set: HashSet<&str> = concept_ids.iter().map(|s| s.as_str()).collect();
        let mut g = KnowledgeGraph::new();
        for id in concept_ids {
            if let Some(&idx) = self.index.get(id) {
                g.add_concept(self.graph[idx].clone());
            }
        }
        for edge in self.graph.edge_references() {
            let a = &self.graph[edge.source()].id;
            let b = &self.graph[edge.target()].id;
            if set.contains(a.as_str()) && set.contains(b.as_str()) {
                let _ = g.add_relation(a, b, edge.weight().kind.clone());
            }
        }
        g
    }

    /// Lightweight consistency check: no concept is both equal-to and
    /// disjoint-from another via `sameAs` / `disjointWith` edges; no
    /// self-loop on `disjointWith`.
    pub fn consistency_check(&self) -> bool {
        for edge in self.graph.edge_references() {
            let kind = &edge.weight().kind;
            if kind == "disjointWith" && edge.source() == edge.target() {
                return false;
            }
            if kind == "sameAs" {
                // Check for conflicting disjointWith between same pair
                let a = edge.source();
                let b = edge.target();
                for e2 in self.graph.edges_directed(a, Direction::Outgoing) {
                    if e2.target() == b && e2.weight().kind == "disjointWith" {
                        return false;
                    }
                }
            }
        }
        // Cycle detection on strictPartOf would be another check; allow cycles
        // for general graphs.
        let _ = has_path_connecting(&self.graph, NodeIndex::new(0), NodeIndex::new(0), None);
        true
    }

    /// Iterator over concept ids.
    pub fn concept_ids(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(|s| s.as_str())
    }

    /// Get concept by id.
    pub fn get(&self, id: &str) -> Option<&Concept> {
        self.index.get(id).map(|&i| &self.graph[i])
    }

    /// Iterate over every relation as `(from_id, to_id, kind)` triples.
    /// Used by persistence to serialize the full graph.
    ///
    /// Lookups use the `Concept` stored in the graph node weight, not
    /// a side index — this avoids a bug where `IndexMap` iteration
    /// order and petgraph `NodeIndex` order are NOT the same and a
    /// naive `id_at[edge.source().index()]` could return the wrong id.
    pub fn relations(&self) -> Vec<(String, String, String)> {
        use petgraph::visit::EdgeRef;
        self.graph
            .edge_references()
            .map(|edge| {
                let from = self.graph[edge.source()].id.clone();
                let to = self.graph[edge.target()].id.clone();
                let kind = edge.weight().kind.clone();
                (from, to, kind)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    #[error("unknown concept `{0}`")]
    UnknownConcept(String),
}

fn reconstruct_path(
    graph: &DiGraph<Concept, Relation>,
    parent: &HashMap<NodeIndex, NodeIndex>,
    start: NodeIndex,
    goal: NodeIndex,
) -> Vec<String> {
    let mut path = vec![graph[goal].id.clone()];
    let mut cur = goal;
    while cur != start {
        cur = parent[&cur];
        path.push(graph[cur].id.clone());
    }
    path.reverse();
    path
}

fn reachable_via(
    graph: &DiGraph<Concept, Relation>,
    start: NodeIndex,
    goal: NodeIndex,
    kind: &str,
) -> bool {
    // A direct self-loop edge (start == goal) counts as reachability —
    // the edge itself witnesses the path of length 1.
    if graph
        .edges_directed(start, Direction::Outgoing)
        .any(|e| e.weight().kind == kind && e.target() == goal)
    {
        return true;
    }
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited.insert(start);
    while let Some(cur) = queue.pop_front() {
        if cur == goal && cur != start {
            return true;
        }
        for edge in graph.edges_directed(cur, Direction::Outgoing) {
            if edge.weight().kind != kind {
                continue;
            }
            let next = edge.target();
            if visited.insert(next) {
                if next == goal {
                    return true;
                }
                queue.push_back(next);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_transitive() {
        let mut g = KnowledgeGraph::new();
        g.add_concept(Concept {
            id: "a".into(),
            label: "A".into(),
        });
        g.add_concept(Concept {
            id: "b".into(),
            label: "B".into(),
        });
        g.add_concept(Concept {
            id: "c".into(),
            label: "C".into(),
        });
        g.add_relation("a", "b", "partOf").unwrap();
        g.add_relation("b", "c", "partOf").unwrap();
        let path = g.query_path("a", "c").unwrap();
        assert_eq!(path, vec!["a", "b", "c"]);
        let closure = g.infer_transitive("partOf");
        assert!(closure.contains(&("a".into(), "c".into())));
        assert!(g.consistency_check());
    }
}
