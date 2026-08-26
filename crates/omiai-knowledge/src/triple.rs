//! RDF-style triple store with SPO / POS / OSP secondary indexes for
//! pattern matching (the backbone of SPARQL-like queries).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// An RDF-like triple `(subject, predicate, object)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Triple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// Pattern component: bound term or variable (name starting with `?`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermPattern {
    Bound(String),
    Var(String),
}

/// A triple pattern for matching.
#[derive(Debug, Clone)]
pub struct TriplePattern {
    pub subject: TermPattern,
    pub predicate: TermPattern,
    pub object: TermPattern,
}

/// Indexed triple store.
#[derive(Debug, Default, Clone)]
pub struct TripleStore {
    triples: Vec<Triple>,
    /// subject → indices
    spo: HashMap<String, Vec<usize>>,
    /// predicate → indices
    pos: HashMap<String, Vec<usize>>,
    /// object → indices
    osp: HashMap<String, Vec<usize>>,
}

impl TripleStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.triples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }

    pub fn insert(&mut self, triple: Triple) {
        let i = self.triples.len();
        self.spo.entry(triple.subject.clone()).or_default().push(i);
        self.pos
            .entry(triple.predicate.clone())
            .or_default()
            .push(i);
        self.osp.entry(triple.object.clone()).or_default().push(i);
        self.triples.push(triple);
    }

    /// Pattern match using the most selective index available.
    ///
    /// Unbound variables match anything; bound terms must equal.
    pub fn match_pattern(&self, pattern: &TriplePattern) -> Vec<Triple> {
        let candidates = self.candidate_indices(pattern);
        candidates
            .into_iter()
            .filter_map(|i| {
                let t = &self.triples[i];
                if matches_component(&pattern.subject, &t.subject)
                    && matches_component(&pattern.predicate, &t.predicate)
                    && matches_component(&pattern.object, &t.object)
                {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn candidate_indices(&self, pattern: &TriplePattern) -> Vec<usize> {
        // Prefer bound subject, then predicate, then object
        if let TermPattern::Bound(s) = &pattern.subject {
            return self.spo.get(s).cloned().unwrap_or_default();
        }
        if let TermPattern::Bound(p) = &pattern.predicate {
            return self.pos.get(p).cloned().unwrap_or_default();
        }
        if let TermPattern::Bound(o) = &pattern.object {
            return self.osp.get(o).cloned().unwrap_or_default();
        }
        (0..self.triples.len()).collect()
    }

    pub fn all(&self) -> &[Triple] {
        &self.triples
    }
}

fn matches_component(pat: &TermPattern, value: &str) -> bool {
    match pat {
        TermPattern::Var(_) => true,
        TermPattern::Bound(b) => b == value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_by_predicate() {
        let mut store = TripleStore::new();
        store.insert(Triple {
            subject: "socrates".into(),
            predicate: "type".into(),
            object: "Human".into(),
        });
        store.insert(Triple {
            subject: "plato".into(),
            predicate: "type".into(),
            object: "Human".into(),
        });
        store.insert(Triple {
            subject: "socrates".into(),
            predicate: "taught".into(),
            object: "plato".into(),
        });
        let hits = store.match_pattern(&TriplePattern {
            subject: TermPattern::Var("?x".into()),
            predicate: TermPattern::Bound("type".into()),
            object: TermPattern::Bound("Human".into()),
        });
        assert_eq!(hits.len(), 2);
    }
}
