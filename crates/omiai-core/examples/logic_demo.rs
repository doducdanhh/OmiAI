//! Example: normalize first-order premises and prove a goal with
//! [`omiai_core::prover::TheoremProver`].
//!
//! Run with: `cargo run --example logic_demo`

use std::time::Instant;

use omiai_core::inference::ProofResult;
use omiai_core::logic_engine::{self, Formula, Term};
use omiai_core::prover::TheoremProver;
use omiai_meta::introspection;

fn main() {
    // Premises: ∀x (Human(x) -> Mortal(x))  AND  Human(socrates)
    // Goal: Mortal(socrates)
    let human_x = Formula::atom("Human", vec![Term::Var("x".into())]);
    let mortal_x = Formula::atom("Mortal", vec![Term::Var("x".into())]);
    let all_humans_mortal = Formula::ForAll(
        "x".into(),
        Box::new(Formula::Implies(Box::new(human_x), Box::new(mortal_x))),
    );
    let socrates_human = Formula::atom("Human", vec![Term::Const("socrates".into())]);
    let goal = Formula::atom("Mortal", vec![Term::Const("socrates".into())]);

    println!("Premises:");
    println!("  ∀x (Human(x) → Mortal(x))");
    println!("  Human(socrates)");
    println!("Goal: Mortal(socrates)");
    println!();

    let start = Instant::now();
    let clauses = {
        let mut c = logic_engine::normalize_cnf(&all_humans_mortal).unwrap();
        c.extend(logic_engine::normalize_cnf(&socrates_human).unwrap());
        c
    };
    println!("CNF of premises ({} clauses):", clauses.len());
    for (i, clause) in clauses.iter().enumerate() {
        let rendered: Vec<String> = clause.iter().map(|l| l.to_string()).collect();
        println!("  clause {}: ({})", i + 1, rendered.join(" OR "));
    }
    println!();

    let prover = TheoremProver::new();
    let report = prover.prove_timed(&[all_humans_mortal, socrates_human], &goal);
    let elapsed = start.elapsed();

    println!("{}", introspection::explain_proof(&report.result));
    match report.result {
        ProofResult::Proved { steps } => {
            println!("Proof steps (resolution): {}", steps.len());
        }
        other => println!("Outcome: {other:?}"),
    }
    println!(
        "Total time: {:.3}ms (prover internal {:.3}ms)",
        elapsed.as_secs_f64() * 1000.0,
        report.elapsed_ms
    );
}
