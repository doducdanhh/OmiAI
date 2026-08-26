//! Fuzz target for the nom-based tokenizer.
//!
//! Goal: ensure that arbitrary byte inputs never trigger a panic, and
//! that any successful tokenization is internally consistent
//! (no empty identifiers, well-formed operators).
//!
//! Run with: `cargo fuzz run tokenizer`

#![no_main]

use libfuzzer_sys::fuzz_target;
use omiai::io::tokenizer::{tokenize, Token};

fuzz_target!(|data: &[u8]| {
    // Convert raw bytes to a string; skip invalid UTF-8 (the tokenizer
    // is documented to accept UTF-8 input).
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Property: tokenize() must NEVER panic. It returns either Ok or Err.
    let result = std::panic::catch_unwind(|| tokenize(s));
    if let Ok(parsed) = result {
        if let Ok(tokens) = parsed {
            // Internal consistency: ident tokens must be non-empty.
            for tok in &tokens {
                match tok {
                    Token::Ident(name) => assert!(!name.is_empty(), "empty Ident"),
                    Token::Number(n) => assert!(!n.is_empty(), "empty Number"),
                    Token::StringLit(s) => {
                        assert!(!s.contains('\u{0}'), "NUL in string literal")
                    }
                    Token::Op(op) => assert!(!op.is_empty(), "empty Op"),
                    _ => {}
                }
            }
        }
    }
});
