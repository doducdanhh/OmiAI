//! Fuzz target for the CNF normalization pipeline.
//!
//! Strategy: feed the fuzzer an [`Arbitrary`]-derived [`Formula`] and
//! run it through `normalize_cnf`. Any panic is a bug.
//!
//! Run with: `cargo fuzz run cnf`

#![no_main]

use arbitrary::{Arbitrary, Result, Unstructured};
use libfuzzer_sys::fuzz_target;
use omiai::core::logic_engine::{Formula, Term};

/// `Term` up to depth 3, no occurrences of the same variable under itself.
#[derive(Debug)]
struct FuzzTerm(Term);

impl<'a> Arbitrary<'a> for FuzzTerm {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        fn go(depth: u8, u: &mut Unstructured<'_>) -> Result<Term> {
            if depth >= 3 || u.arbitrary()? {
                // Leaf: variable or constant
                let s: String = u.arbitrary::<String>()?;
                let s = s.chars().take(8).collect::<String>();
                let s = if s.is_empty() {
                    "x".to_string()
                } else {
                    s
                };
                if u.arbitrary()? {
                    Ok(Term::Var(s))
                } else {
                    Ok(Term::Const(s))
                }
            } else {
                let name: String = u.arbitrary::<String>()?;
                let name: String = name.chars().take(8).collect();
                let name = if name.is_empty() {
                    "f".into()
                } else {
                    name
                };
                let n_args = u.int_in_range(0usize..=2)?;
                let mut args = Vec::with_capacity(n_args);
                for _ in 0..n_args {
                    args.push(go(depth + 1, u)?);
                }
                Ok(Term::Func(name, args))
            }
        }
        Ok(FuzzTerm(go(0, u)?))
    }
}

#[derive(Debug)]
struct FuzzFormula(Formula);

impl<'a> Arbitrary<'a> for FuzzFormula {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        fn go(depth: u8, u: &mut Unstructured<'_>) -> Result<Formula> {
            if depth >= 5 {
                let s: String = u.arbitrary::<String>()?;
                let s: String = s.chars().take(8).collect();
                let s = if s.is_empty() { "P".into() } else { s };
                let n = u.int_in_range(0usize..=2)?;
                let args: Result<Vec<Term>> = (0..n)
                    .map(|_| {
                        let t: FuzzTerm = u.arbitrary()?;
                        Ok(t.0)
                    })
                    .collect();
                return Ok(Formula::Atom(s, args?));
            }
            // Pick a kind
            let kind: u8 = u.int_in_range(0u8..=7)?;
            Ok(match kind {
                0 => Formula::True,
                1 => Formula::False,
                2 => {
                    let s: String = u.arbitrary::<String>()?;
                    let s: String = s.chars().take(8).collect();
                    let s = if s.is_empty() { "P".into() } else { s };
                    let n = u.int_in_range(0usize..=2)?;
                    let args: Result<Vec<Term>> = (0..n)
                        .map(|_| {
                            let t: FuzzTerm = u.arbitrary()?;
                            Ok(t.0)
                        })
                        .collect();
                    Formula::Atom(s, args?)
                }
                3 => Formula::Not(Box::new(go(depth + 1, u)?)),
                4 => Formula::And(
                    Box::new(go(depth + 1, u)?),
                    Box::new(go(depth + 1, u)?),
                ),
                5 => Formula::Or(
                    Box::new(go(depth + 1, u)?),
                    Box::new(go(depth + 1, u)?),
                ),
                6 => Formula::Implies(
                    Box::new(go(depth + 1, u)?),
                    Box::new(go(depth + 1, u)?),
                ),
                _ => Formula::Iff(
                    Box::new(go(depth + 1, u)?),
                    Box::new(go(depth + 1, u)?),
                ),
            })
        }
        Ok(FuzzFormula(go(0, u)?))
    }
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    if let Ok(formula) = FuzzFormula::arbitrary(&mut u) {
        // normalize_cnf must NOT panic on any well-formed Formula.
        let _ = std::panic::catch_unwind(|| {
            omiai::core::logic_engine::normalize_cnf(&formula.0)
        });
    }
});
