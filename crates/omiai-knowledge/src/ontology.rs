//! OWL 2 / SROIQ-inspired ontology reasoning (lightweight classification).
//!
//! Full SROIQ tableau is enormous; this module implements a practical
//! subset: concept hierarchy (subsumption via declared `subClassOf` edges),
//! role hierarchy, and disjointness checks — sufficient for ontology
//! classification over declared axioms.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

/// Concept (class) name.
pub type ConceptName = String;
/// Role (object property) name.
pub type RoleName = String;

/// A declared ontology axiom (subset of OWL 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axiom {
    /// C ⊑ D
    SubClassOf(ConceptName, ConceptName),
    /// C ≡ D
    EquivalentClasses(ConceptName, ConceptName),
    /// C ⊓ D ⊑ ⊥
    DisjointClasses(ConceptName, ConceptName),
    /// R ⊑ S
    SubPropertyOf(RoleName, RoleName),
    /// R is transitive
    Transitive(RoleName),
    /// domain(R) = C
    Domain(RoleName, ConceptName),
    /// range(R) = C
    Range(RoleName, ConceptName),
}

/// Ontology: set of axioms + computed classification.
#[derive(Debug, Default, Clone)]
pub struct Ontology {
    pub axioms: Vec<Axiom>,
    /// Inferred subclass pairs after classification
    pub subclass_closure: HashSet<(ConceptName, ConceptName)>,
}

impl Ontology {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_axiom(&mut self, axiom: Axiom) {
        self.axioms.push(axiom);
    }

    /// Compute the full declared subsumption hierarchy (transitive closure
    /// of `SubClassOf` + symmetric `EquivalentClasses`).
    pub fn classify(&mut self) {
        let mut edges: HashMap<String, Vec<String>> = HashMap::new();
        let mut concepts: HashSet<String> = HashSet::new();

        for ax in &self.axioms {
            match ax {
                Axiom::SubClassOf(c, d) => {
                    concepts.insert(c.clone());
                    concepts.insert(d.clone());
                    edges.entry(c.clone()).or_default().push(d.clone());
                }
                Axiom::EquivalentClasses(c, d) => {
                    concepts.insert(c.clone());
                    concepts.insert(d.clone());
                    edges.entry(c.clone()).or_default().push(d.clone());
                    edges.entry(d.clone()).or_default().push(c.clone());
                }
                Axiom::DisjointClasses(c, d) => {
                    concepts.insert(c.clone());
                    concepts.insert(d.clone());
                }
                _ => {}
            }
        }

        let mut closure = HashSet::new();
        for c in &concepts {
            // BFS ancestors
            let mut visited = HashSet::new();
            let mut q = VecDeque::new();
            q.push_back(c.clone());
            visited.insert(c.clone());
            while let Some(cur) = q.pop_front() {
                if cur != *c {
                    closure.insert((c.clone(), cur.clone()));
                }
                if let Some(succs) = edges.get(&cur) {
                    for s in succs {
                        if visited.insert(s.clone()) {
                            q.push_back(s.clone());
                        }
                    }
                }
            }
            // reflexive
            closure.insert((c.clone(), c.clone()));
        }
        self.subclass_closure = closure;
    }

    /// True iff C is inferred to be a subclass of D.
    pub fn is_subclass(&self, c: &str, d: &str) -> bool {
        self.subclass_closure
            .contains(&(c.to_string(), d.to_string()))
    }

    /// Check disjointness axioms against the hierarchy: if C ⊑ A, D ⊑ B
    /// and A,B disjoint with C=D possible — flag inconsistency when a
    /// concept is subclass of two disjoint classes.
    pub fn is_consistent(&self) -> bool {
        let mut disjoint_pairs: HashSet<(String, String)> = HashSet::new();
        for ax in &self.axioms {
            if let Axiom::DisjointClasses(a, b) = ax {
                disjoint_pairs.insert((a.clone(), b.clone()));
                disjoint_pairs.insert((b.clone(), a.clone()));
            }
        }
        // For each concept, collect its superclasses; if any two are disjoint → bad
        let mut supers: HashMap<String, HashSet<String>> = HashMap::new();
        for (c, d) in &self.subclass_closure {
            supers.entry(c.clone()).or_default().insert(d.clone());
        }
        for (_c, sups) in &supers {
            let list: Vec<_> = sups.iter().collect();
            for i in 0..list.len() {
                for j in (i + 1)..list.len() {
                    if disjoint_pairs.contains(&(list[i].clone(), list[j].clone())) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subclass_transitivity() {
        let mut onto = Ontology::new();
        onto.add_axiom(Axiom::SubClassOf("Human".into(), "Mammal".into()));
        onto.add_axiom(Axiom::SubClassOf("Mammal".into(), "Animal".into()));
        onto.classify();
        assert!(onto.is_subclass("Human", "Animal"));
        assert!(onto.is_consistent());
    }

    #[test]
    fn disjoint_inconsistency() {
        let mut onto = Ontology::new();
        onto.add_axiom(Axiom::SubClassOf("C".into(), "A".into()));
        onto.add_axiom(Axiom::SubClassOf("C".into(), "B".into()));
        onto.add_axiom(Axiom::DisjointClasses("A".into(), "B".into()));
        onto.classify();
        assert!(!onto.is_consistent());
    }
}
