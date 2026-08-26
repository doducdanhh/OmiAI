//! Fuzz target for the indexed triple store and SPARQL-like pattern matcher.
//!
//! Goal: random triples and random patterns must never panic, and the
//! number of matches must be bounded by `store.len()`.
//!
//! Run with: `cargo fuzz run triple`

#![no_main]

use arbitrary::{Arbitrary, Result, Unstructured};
use libfuzzer_sys::fuzz_target;
use omiai::knowledge::triple::{TermPattern, Triple, TriplePattern, TripleStore};

/// A fuzzed triple of bounded-size strings.
#[derive(Debug)]
struct FuzzTriple(Triple);

impl<'a> Arbitrary<'a> for FuzzTriple {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        fn short(u: &mut Unstructured<'_>) -> Result<String> {
            let s: String = u.arbitrary()?;
            Ok(s.chars().take(16).collect::<String>())
        }
        Ok(FuzzTriple(Triple {
            subject: short(u)?,
            predicate: short(u)?,
            object: short(u)?,
        }))
    }
}

#[derive(Debug)]
struct FuzzPattern(TriplePattern);

impl<'a> Arbitrary<'a> for FuzzPattern {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        fn term(u: &mut Unstructured<'_>) -> Result<TermPattern> {
            let s: String = u.arbitrary()?;
            let s: String = s.chars().take(16).collect();
            if u.arbitrary()? {
                Ok(TermPattern::Bound(s))
            } else {
                Ok(TermPattern::Var(s))
            }
        }
        Ok(FuzzPattern(TriplePattern {
            subject: term(u)?,
            predicate: term(u)?,
            object: term(u)?,
        }))
    }
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let n_triples = u.int_in_range(0usize..=64).unwrap_or(0);
    let mut store = TripleStore::new();
    for _ in 0..n_triples {
        if let Ok(FuzzTriple(t)) = FuzzTriple::arbitrary(&mut u) {
            store.insert(t);
        }
    }
    let n_patterns = u.int_in_range(0usize..=8).unwrap_or(0);
    for _ in 0..n_patterns {
        let Ok(FuzzPattern(p)) = FuzzPattern::arbitrary(&mut u) else {
            continue;
        };
        // match_pattern must never panic.
        let _ = std::panic::catch_unwind(|| store.match_pattern(&p));
    }
});
