//! Semantic memory: concept embeddings as sparse feature sets and
//! hierarchical is-a links (integrates with knowledge graphs).

use std::collections::{HashMap, HashSet};

/// A semantic concept with feature set.
#[derive(Debug, Clone)]
pub struct SemanticConcept {
    pub name: String,
    pub features: HashSet<String>,
}

/// Semantic memory store.
#[derive(Debug, Default, Clone)]
pub struct SemanticMemory {
    pub concepts: HashMap<String, SemanticConcept>,
    /// is-a edges: child → parents
    pub is_a: HashMap<String, Vec<String>>,
}

impl SemanticMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_concept(&mut self, concept: SemanticConcept) {
        self.concepts.insert(concept.name.clone(), concept);
    }

    pub fn add_is_a(&mut self, child: impl Into<String>, parent: impl Into<String>) {
        self.is_a
            .entry(child.into())
            .or_default()
            .push(parent.into());
    }

    /// Feature overlap (Jaccard).
    pub fn similarity(&self, a: &str, b: &str) -> f64 {
        let (Some(ca), Some(cb)) = (self.concepts.get(a), self.concepts.get(b)) else {
            return 0.0;
        };
        let inter = ca.features.intersection(&cb.features).count() as f64;
        let union = ca.features.union(&cb.features).count() as f64;
        if union < 1e-9 { 0.0 } else { inter / union }
    }

    /// Inherited features via is-a.
    pub fn inherited_features(&self, name: &str) -> HashSet<String> {
        let mut feats = self
            .concepts
            .get(name)
            .map(|c| c.features.clone())
            .unwrap_or_default();
        let mut stack = self.is_a.get(name).cloned().unwrap_or_default();
        let mut seen = HashSet::new();
        seen.insert(name.to_string());
        while let Some(p) = stack.pop() {
            if !seen.insert(p.clone()) {
                continue;
            }
            if let Some(c) = self.concepts.get(&p) {
                feats.extend(c.features.iter().cloned());
            }
            if let Some(pps) = self.is_a.get(&p) {
                stack.extend(pps.iter().cloned());
            }
        }
        feats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inheritance() {
        let mut m = SemanticMemory::new();
        m.add_concept(SemanticConcept {
            name: "Animal".into(),
            features: ["moves".into(), "alive".into()].into_iter().collect(),
        });
        m.add_concept(SemanticConcept {
            name: "Bird".into(),
            features: ["wings".into()].into_iter().collect(),
        });
        m.add_is_a("Bird", "Animal");
        let f = m.inherited_features("Bird");
        assert!(f.contains("moves") && f.contains("wings"));
    }
}
