//! SPARQL-like query language over a [`super::triple::TripleStore`].
//!
//! Supports basic graph patterns (BGPs), join ordering by selectivity
//! heuristics (bound subjects preferred), and simple FILTER equality.

use std::collections::HashMap;

use super::triple::{TermPattern, Triple, TriplePattern, TripleStore};

/// A SELECT query: variables to project + basic graph patterns.
#[derive(Debug, Clone)]
pub struct Query {
    pub select: Vec<String>,
    pub patterns: Vec<TriplePattern>,
    /// Optional FILTER: var == constant
    pub filters: Vec<(String, String)>,
}

/// One solution mapping: variable → bound value.
pub type Solution = HashMap<String, String>;

/// Execute a query with statistics-based join ordering.
///
/// Patterns with more bound terms are evaluated first (reduces intermediate
/// result size).
pub fn execute(store: &TripleStore, query: &Query) -> Vec<Solution> {
    if query.patterns.is_empty() {
        return vec![HashMap::new()];
    }

    // Order patterns by estimated selectivity (more Bound → earlier)
    let mut order: Vec<usize> = (0..query.patterns.len()).collect();
    order.sort_by_key(|&i| {
        let p = &query.patterns[i];
        let bound = [&p.subject, &p.predicate, &p.object]
            .iter()
            .filter(|t| matches!(t, TermPattern::Bound(_)))
            .count();
        // lower key first: prefer higher bound count
        3 - bound
    });

    let mut solutions: Vec<Solution> = vec![HashMap::new()];

    for idx in order {
        let pattern = &query.patterns[idx];
        let mut next_solutions = Vec::new();
        for sol in &solutions {
            let grounded = ground_pattern(pattern, sol);
            let matches = store.match_pattern(&grounded);
            for triple in matches {
                if let Some(extended) = extend_solution(sol, pattern, &triple)
                    && passes_filters(&extended, &query.filters) {
                        next_solutions.push(extended);
                    }
            }
        }
        solutions = next_solutions;
        if solutions.is_empty() {
            break;
        }
    }

    // Project SELECT variables
    if query.select.is_empty() {
        return solutions;
    }
    solutions
        .into_iter()
        .map(|sol| {
            sol.into_iter()
                .filter(|(k, _)| query.select.iter().any(|s| s == k || s == &format!("?{k}")))
                .collect()
        })
        .collect()
}

fn ground_pattern(pattern: &TriplePattern, sol: &Solution) -> TriplePattern {
    TriplePattern {
        subject: ground_term(&pattern.subject, sol),
        predicate: ground_term(&pattern.predicate, sol),
        object: ground_term(&pattern.object, sol),
    }
}

fn ground_term(term: &TermPattern, sol: &Solution) -> TermPattern {
    match term {
        TermPattern::Bound(b) => TermPattern::Bound(b.clone()),
        TermPattern::Var(v) => {
            let key = v.trim_start_matches('?');
            if let Some(val) = sol.get(key).or_else(|| sol.get(v)) {
                TermPattern::Bound(val.clone())
            } else {
                TermPattern::Var(v.clone())
            }
        }
    }
}

fn extend_solution(sol: &Solution, pattern: &TriplePattern, triple: &Triple) -> Option<Solution> {
    let mut out = sol.clone();
    bind(&mut out, &pattern.subject, &triple.subject)?;
    bind(&mut out, &pattern.predicate, &triple.predicate)?;
    bind(&mut out, &pattern.object, &triple.object)?;
    Some(out)
}

fn bind(sol: &mut Solution, pat: &TermPattern, value: &str) -> Option<()> {
    match pat {
        TermPattern::Bound(b) => {
            if b == value {
                Some(())
            } else {
                None
            }
        }
        TermPattern::Var(v) => {
            let key = v.trim_start_matches('?').to_string();
            if let Some(existing) = sol.get(&key) {
                if existing == value { Some(()) } else { None }
            } else {
                sol.insert(key, value.to_string());
                Some(())
            }
        }
    }
}

fn passes_filters(sol: &Solution, filters: &[(String, String)]) -> bool {
    for (var, val) in filters {
        let key = var.trim_start_matches('?');
        match sol.get(key) {
            Some(v) if v == val => {}
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triple::TermPattern;

    #[test]
    fn simple_select() {
        let mut store = TripleStore::new();
        store.insert(Triple {
            subject: "socrates".into(),
            predicate: "type".into(),
            object: "Human".into(),
        });
        let q = Query {
            select: vec!["x".into()],
            patterns: vec![TriplePattern {
                subject: TermPattern::Var("x".into()),
                predicate: TermPattern::Bound("type".into()),
                object: TermPattern::Bound("Human".into()),
            }],
            filters: vec![],
        };
        let sols = execute(&store, &q);
        assert_eq!(sols.len(), 1);
        assert_eq!(sols[0].get("x").map(|s| s.as_str()), Some("socrates"));
    }
}
