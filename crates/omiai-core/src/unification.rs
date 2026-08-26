//! Robinson's first-order unification algorithm with an occurs check.
//!
//! This is syntactic (not higher-order) unification: it operates purely
//! over [`Term`]s built from variables, constants, and functions. Huet's
//! higher-order unification (needed for `∀`/`∃` over predicates rather
//! than individuals) is a separate, larger algorithm and is scaffolded
//! separately once `core::inference` lands.

use super::logic_engine::Term;
use super::substitution::Substitution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnificationError {
    OccursCheckFailed {
        var: String,
        term: Term,
    },
    MismatchedFunctors {
        left: String,
        right: String,
    },
    ArityMismatch {
        functor: String,
        left_arity: usize,
        right_arity: usize,
    },
}

impl std::fmt::Display for UnificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnificationError::OccursCheckFailed { var, term } => {
                write!(f, "occurs check failed: `{var}` occurs in `{term}`")
            }
            UnificationError::MismatchedFunctors { left, right } => {
                write!(f, "cannot unify distinct terms `{left}` and `{right}`")
            }
            UnificationError::ArityMismatch {
                functor,
                left_arity,
                right_arity,
            } => {
                write!(
                    f,
                    "arity mismatch for `{functor}`: {left_arity} vs {right_arity}"
                )
            }
        }
    }
}

impl std::error::Error for UnificationError {}

/// Unify two terms, returning the most general unifier as a
/// [`Substitution`] on success.
pub fn unify(t1: &Term, t2: &Term) -> Result<Substitution, UnificationError> {
    let mut subst = Substitution::new();
    unify_with(t1, t2, &mut subst)?;
    Ok(subst)
}

fn unify_with(t1: &Term, t2: &Term, subst: &mut Substitution) -> Result<(), UnificationError> {
    let t1r = resolve(t1, subst);
    let t2r = resolve(t2, subst);
    match (t1r, t2r) {
        (Term::Var(v1), Term::Var(v2)) if v1 == v2 => Ok(()),
        (Term::Var(v), other) => bind_var(v, other, subst),
        (other, Term::Var(v)) => bind_var(v, other, subst),
        (Term::Const(a), Term::Const(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(UnificationError::MismatchedFunctors { left: a, right: b })
            }
        }
        (Term::Func(f1, args1), Term::Func(f2, args2)) => {
            if f1 != f2 {
                return Err(UnificationError::MismatchedFunctors {
                    left: f1,
                    right: f2,
                });
            }
            if args1.len() != args2.len() {
                return Err(UnificationError::ArityMismatch {
                    functor: f1,
                    left_arity: args1.len(),
                    right_arity: args2.len(),
                });
            }
            for (a, b) in args1.iter().zip(args2.iter()) {
                unify_with(a, b, subst)?;
            }
            Ok(())
        }
        (left, right) => Err(UnificationError::MismatchedFunctors {
            left: left.to_string(),
            right: right.to_string(),
        }),
    }
}

/// Follow a chain of variable bindings to the current representative term.
fn resolve(term: &Term, subst: &Substitution) -> Term {
    match term {
        Term::Var(v) => match subst.get(v) {
            Some(bound) => resolve(bound, subst),
            None => term.clone(),
        },
        _ => term.clone(),
    }
}

fn occurs(var: &str, term: &Term, subst: &Substitution) -> bool {
    match resolve(term, subst) {
        Term::Var(v) => v == var,
        Term::Const(_) => false,
        Term::Func(_, args) => args.iter().any(|a| occurs(var, a, subst)),
    }
}

fn bind_var(var: String, term: Term, subst: &mut Substitution) -> Result<(), UnificationError> {
    if let Term::Var(v2) = &term
        && *v2 == var {
            return Ok(());
        }
    if occurs(&var, &term, subst) {
        return Err(UnificationError::OccursCheckFailed { var, term });
    }
    subst.bind(var, term);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unifies_function_terms_with_variables() {
        // f(x, a) and f(b, y)  ->  x = b, y = a
        let t1 = Term::Func(
            "f".into(),
            vec![Term::Var("x".into()), Term::Const("a".into())],
        );
        let t2 = Term::Func(
            "f".into(),
            vec![Term::Const("b".into()), Term::Var("y".into())],
        );

        let subst = unify(&t1, &t2).expect("should unify");
        assert_eq!(subst.get("x"), Some(&Term::Const("b".into())));
        assert_eq!(subst.get("y"), Some(&Term::Const("a".into())));
    }

    #[test]
    fn occurs_check_rejects_infinite_term() {
        // x  and  f(x)  ->  occurs check must fail
        let t1 = Term::Var("x".into());
        let t2 = Term::Func("f".into(), vec![Term::Var("x".into())]);

        let result = unify(&t1, &t2);
        assert!(matches!(
            result,
            Err(UnificationError::OccursCheckFailed { .. })
        ));
    }

    #[test]
    fn mismatched_constants_fail() {
        let t1 = Term::Const("a".into());
        let t2 = Term::Const("b".into());
        assert!(unify(&t1, &t2).is_err());
    }

    #[test]
    fn mismatched_arity_fails() {
        let t1 = Term::Func("f".into(), vec![Term::Var("x".into())]);
        let t2 = Term::Func(
            "f".into(),
            vec![Term::Var("x".into()), Term::Var("y".into())],
        );
        assert!(matches!(
            unify(&t1, &t2),
            Err(UnificationError::ArityMismatch { .. })
        ));
    }
}
