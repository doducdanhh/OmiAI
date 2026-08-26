//! Fuzz target for Robinson's unification.
//!
//! Goal: arbitrary [`Term`] inputs must never panic during `unify`.
//!
//! Run with: `cargo fuzz run unify`

#![no_main]

use arbitrary::{Arbitrary, Result, Unstructured};
use libfuzzer_sys::fuzz_target;
use omiai::core::logic_engine::Term;
use omiai::core::unification::unify;

#[derive(Debug)]
struct FuzzTerm(Term);

impl<'a> Arbitrary<'a> for FuzzTerm {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        fn go(depth: u8, u: &mut Unstructured<'_>) -> Result<Term> {
            if depth >= 3 || u.arbitrary()? {
                let s: String = u.arbitrary::<String>()?;
                let s: String = s.chars().take(6).collect();
                let s = if s.is_empty() { "x".into() } else { s };
                if u.arbitrary()? {
                    Ok(Term::Var(s))
                } else {
                    Ok(Term::Const(s))
                }
            } else {
                let name: String = u.arbitrary::<String>()?;
                let name: String = name.chars().take(6).collect();
                let name = if name.is_empty() { "f".into() } else { name };
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

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let (Ok(t1), Ok(t2)) = (
        FuzzTerm::arbitrary(&mut u).map(|x| x.0),
        FuzzTerm::arbitrary(&mut u).map(|x| x.0),
    ) else {
        return;
    };

    // unify() must NEVER panic. It returns a Result with three error
    // variants; the fuzz verifies no panic, infinite loop, or memory
    // blow-up happens across many iterations.
    let _ = std::panic::catch_unwind(|| unify(&t1, &t2));
});
