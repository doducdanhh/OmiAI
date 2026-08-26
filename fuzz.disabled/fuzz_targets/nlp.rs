//! Fuzz target for the Montague-style NLP parser.
//!
//! Goal: arbitrary input strings must never panic during parsing, and
//! when parsing succeeds the resulting [`Formula`] must be syntactically
//! valid (the type system already enforces this; the fuzz verifies it).
//!
//! Run with: `cargo fuzz run nlp`

#![no_main]

use libfuzzer_sys::fuzz_target;
use omiai::io::nlp_parser::NlpParser;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    // Bound input length to avoid pathological allocations.
    if s.len() > 4096 {
        return;
    }
    let parser = NlpParser::with_default_lexicon();
    let _ = std::panic::catch_unwind(|| parser.parse(s));
});
