//! Constraint Satisfaction Problem (CSP) solver: AC-3 arc consistency
//! and backtracking search with forward checking.
//!
//! # Model
//! Variables have finite domains. Binary constraints are predicates
//! `C(x, y) ⊆ D(x) × D(y)`. AC-3 revises domains until a fixed point;
//! search then assigns variables in order, pruning with forward checking.

use std::collections::{HashMap, HashSet, VecDeque};

/// A finite-domain CSP.
#[derive(Debug, Clone)]
pub struct Csp {
    /// Variable name → domain values.
    pub domains: HashMap<String, Vec<i64>>,
    /// Binary constraints: (x, y, allowed pairs).
    pub constraints: Vec<BinaryConstraint>,
}

/// Binary constraint between two variables.
#[derive(Debug, Clone)]
pub struct BinaryConstraint {
    pub x: String,
    pub y: String,
    /// Allowed value pairs `(vx, vy)`.
    pub allowed: HashSet<(i64, i64)>,
}

impl Csp {
    pub fn new() -> Self {
        Self {
            domains: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    pub fn add_variable(&mut self, name: impl Into<String>, domain: Vec<i64>) {
        self.domains.insert(name.into(), domain);
    }

    pub fn add_constraint(&mut self, constraint: BinaryConstraint) {
        self.constraints.push(constraint);
    }

    /// AC-3 arc-consistency propagation.
    ///
    /// Returns `false` if some domain is emptied (problem is unsatisfiable
    /// under the current domains).
    pub fn ac3(&mut self) -> bool {
        let mut queue: VecDeque<(String, String)> = VecDeque::new();
        for c in &self.constraints {
            queue.push_back((c.x.clone(), c.y.clone()));
            queue.push_back((c.y.clone(), c.x.clone()));
        }

        while let Some((xi, xj)) = queue.pop_front() {
            if self.revise(&xi, &xj) {
                if self.domains.get(&xi).map(|d| d.is_empty()).unwrap_or(true) {
                    return false;
                }
                // Re-enqueue arcs (xk, xi) for neighbors xk ≠ xj
                for c in &self.constraints {
                    if c.x == xi && c.y != xj {
                        queue.push_back((c.y.clone(), xi.clone()));
                    } else if c.y == xi && c.x != xj {
                        queue.push_back((c.x.clone(), xi.clone()));
                    }
                }
            }
        }
        true
    }

    fn revise(&mut self, xi: &str, xj: &str) -> bool {
        let di = match self.domains.get(xi) {
            Some(d) => d.clone(),
            None => return false,
        };
        let dj = match self.domains.get(xj) {
            Some(d) => d.clone(),
            None => return false,
        };

        let mut revised = false;
        let mut new_di = Vec::new();
        for &vi in &di {
            let supported = dj.iter().any(|&vj| self.allows(xi, xj, vi, vj));
            if supported {
                new_di.push(vi);
            } else {
                revised = true;
            }
        }
        if revised {
            self.domains.insert(xi.to_string(), new_di);
        }
        revised
    }

    fn allows(&self, xi: &str, xj: &str, vi: i64, vj: i64) -> bool {
        for c in &self.constraints {
            if c.x == xi && c.y == xj {
                return c.allowed.contains(&(vi, vj));
            }
            if c.x == xj && c.y == xi {
                return c.allowed.contains(&(vj, vi));
            }
        }
        // No explicit constraint ⇒ always allowed
        true
    }

    /// Backtracking search with forward checking after AC-3.
    ///
    /// Returns a complete assignment if one exists.
    pub fn solve(&mut self) -> Option<HashMap<String, i64>> {
        if !self.ac3() {
            return None;
        }
        let mut assignment = HashMap::new();
        self.backtrack(&mut assignment)
    }

    fn backtrack(&mut self, assignment: &mut HashMap<String, i64>) -> Option<HashMap<String, i64>> {
        if assignment.len() == self.domains.len() {
            return Some(assignment.clone());
        }
        let var = self
            .domains
            .keys()
            .find(|v| !assignment.contains_key(v.as_str()))?
            .clone();
        let domain = self.domains.get(&var)?.clone();
        for value in domain {
            if self.consistent(&var, value, assignment) {
                assignment.insert(var.clone(), value);
                // Forward check: snapshot domains
                let snapshot = self.domains.clone();
                if self.forward_check(&var, value, assignment) {
                    if let Some(sol) = self.backtrack(assignment) {
                        return Some(sol);
                    }
                }
                self.domains = snapshot;
                assignment.remove(&var);
            }
        }
        None
    }

    fn consistent(&self, var: &str, value: i64, assignment: &HashMap<String, i64>) -> bool {
        for (other, &ov) in assignment {
            if !self.allows(var, other, value, ov) {
                return false;
            }
        }
        true
    }

    fn forward_check(&mut self, var: &str, value: i64, assignment: &HashMap<String, i64>) -> bool {
        // Restrict var's domain to {value}
        self.domains.insert(var.to_string(), vec![value]);
        for (other, domain) in self.domains.clone() {
            if assignment.contains_key(&other) || other == var {
                continue;
            }
            let filtered: Vec<i64> = domain
                .into_iter()
                .filter(|&ov| self.allows(var, &other, value, ov))
                .collect();
            if filtered.is_empty() {
                return false;
            }
            self.domains.insert(other, filtered);
        }
        true
    }
}

impl Default for Csp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_coloring_two_nodes() {
        let mut csp = Csp::new();
        csp.add_variable("A", vec![1, 2]);
        csp.add_variable("B", vec![1, 2]);
        // Adjacent regions must differ
        let mut allowed = HashSet::new();
        allowed.insert((1, 2));
        allowed.insert((2, 1));
        csp.add_constraint(BinaryConstraint {
            x: "A".into(),
            y: "B".into(),
            allowed,
        });
        let sol = csp.solve().expect("should color");
        assert_ne!(sol["A"], sol["B"]);
    }

    #[test]
    fn empty_domain_unsat() {
        let mut csp = Csp::new();
        csp.add_variable("X", vec![]);
        assert!(csp.solve().is_none());
    }
}
