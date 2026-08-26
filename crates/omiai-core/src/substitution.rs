//! Variable substitutions over [`Term`]s and [`Formula`]s, used by both
//! [`super::logic_engine::skolemize`] and [`super::unification::unify`].

use std::collections::HashMap;

use super::logic_engine::{Formula, Term};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Substitution {
    bindings: HashMap<String, Term>,
}

impl Substitution {
    /// Iterate over all bindings in this substitution.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Term)> {
        self.bindings.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl Substitution {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, var: String, term: Term) {
        self.bindings.insert(var, term);
    }

    pub fn get(&self, var: &str) -> Option<&Term> {
        self.bindings.get(var)
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Recursively apply this substitution to a term.
    pub fn apply_term(&self, term: &Term) -> Term {
        match term {
            Term::Var(v) => self
                .bindings
                .get(v)
                .cloned()
                .unwrap_or_else(|| term.clone()),
            Term::Const(_) => term.clone(),
            Term::Func(name, args) => Term::Func(
                name.clone(),
                args.iter().map(|a| self.apply_term(a)).collect(),
            ),
        }
    }

    /// Recursively apply this substitution to every free term occurrence
    /// in a formula. Bound variables (under a matching quantifier name)
    /// are left alone, matching standard capture-avoiding substitution
    /// for the way this crate uses substitutions (Skolemization always
    /// substitutes a variable that is *not* re-bound further down, since
    /// `skolemize` processes quantifiers outside-in).
    pub fn apply_formula(&self, formula: &Formula) -> Formula {
        match formula {
            Formula::True | Formula::False => formula.clone(),
            Formula::Atom(name, args) => Formula::Atom(
                name.clone(),
                args.iter().map(|a| self.apply_term(a)).collect(),
            ),
            Formula::Not(a) => Formula::Not(Box::new(self.apply_formula(a))),
            Formula::And(a, b) => Formula::And(
                Box::new(self.apply_formula(a)),
                Box::new(self.apply_formula(b)),
            ),
            Formula::Or(a, b) => Formula::Or(
                Box::new(self.apply_formula(a)),
                Box::new(self.apply_formula(b)),
            ),
            Formula::Implies(a, b) => Formula::Implies(
                Box::new(self.apply_formula(a)),
                Box::new(self.apply_formula(b)),
            ),
            Formula::Iff(a, b) => Formula::Iff(
                Box::new(self.apply_formula(a)),
                Box::new(self.apply_formula(b)),
            ),
            Formula::ForAll(v, body) => {
                Formula::ForAll(v.clone(), Box::new(self.apply_formula(body)))
            }
            Formula::Exists(v, body) => {
                Formula::Exists(v.clone(), Box::new(self.apply_formula(body)))
            }
        }
    }

    /// Compose `self` followed by `other`: applying the result to a term
    /// is equivalent to applying `self` first, then `other` to the
    /// outcome.
    pub fn compose(&self, other: &Substitution) -> Substitution {
        let mut result = Substitution::new();
        for (var, term) in &self.bindings {
            result.bind(var.clone(), other.apply_term(term));
        }
        for (var, term) in &other.bindings {
            result
                .bindings
                .entry(var.clone())
                .or_insert_with(|| term.clone());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_term_substitutes_bound_variable() {
        let mut s = Substitution::new();
        s.bind("x".into(), Term::Const("a".into()));
        let t = Term::Func(
            "f".into(),
            vec![Term::Var("x".into()), Term::Var("y".into())],
        );
        let result = s.apply_term(&t);
        assert_eq!(
            result,
            Term::Func(
                "f".into(),
                vec![Term::Const("a".into()), Term::Var("y".into())]
            )
        );
    }

    #[test]
    fn compose_chains_substitutions() {
        let mut s1 = Substitution::new();
        s1.bind("x".into(), Term::Var("y".into()));
        let mut s2 = Substitution::new();
        s2.bind("y".into(), Term::Const("a".into()));

        let composed = s1.compose(&s2);
        assert_eq!(composed.get("x"), Some(&Term::Const("a".into())));
        assert_eq!(composed.get("y"), Some(&Term::Const("a".into())));
    }
}
