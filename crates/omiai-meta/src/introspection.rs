//! Self-observation helpers over proof outcomes.

use omiai_core::inference::ProofResult;

/// Produce a compact, inspectable explanation of a proof outcome.
pub fn explain_proof(result: &ProofResult) -> String {
    match result {
        ProofResult::Proved { steps } if steps.is_empty() => {
            "Proved by propositional inconsistency detection.".into()
        }
        ProofResult::Proved { steps } => {
            format!(
                "Proved by resolution in {} derivation step(s).",
                steps.len()
            )
        }
        ProofResult::Disproved { counterexample } => {
            format!(
                "Not entailed; a counterexample contains {} literal(s).",
                counterexample.len()
            )
        }
        ProofResult::Unknown => "No conclusion within the configured resource budget.".into(),
    }
}

