//! Benchmarks for the propositional and first-order provers.
//!
//! Compares:
//! - `dpll_satisfiable` on random 3-SAT instances at varying sizes
//! - `cdcl_satisfiable` on the same
//! - `resolution_refute_bounded` on first-order premise sets
//!
//! Run with: `cargo bench --bench sat`

use std::time::Duration;

use criterion::{Criterion, SamplingMode, black_box, criterion_group, criterion_main};
use omiai_core::inference::{cdcl_satisfiable, dpll_satisfiable, resolution_refute_bounded};
use omiai_core::logic_engine::{Literal, Term};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Generate a random 3-SAT instance. With `vars` variables and `clauses`
/// clauses, the satisfiability threshold is around `4.258 * vars`.
fn random_3sat(vars: usize, clauses: usize, seed: u64) -> Vec<Vec<Literal>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut out = Vec::with_capacity(clauses);
    while out.len() < clauses {
        let mut clause = Vec::with_capacity(3);
        while clause.len() < 3 {
            let v = rng.gen_range(0..vars);
            clause.push(Literal {
                negated: rng.r#gen::<bool>(),
                predicate: format!("v{v}"),
                args: vec![],
            });
        }
        // Avoid tautological clauses (x ∨ ¬x) for cleaner timing
        let mut is_taut = false;
        'outer: for i in 0..clause.len() {
            for j in (i + 1)..clause.len() {
                if clause[i].predicate == clause[j].predicate
                    && clause[i].negated != clause[j].negated
                {
                    is_taut = true;
                    break 'outer;
                }
            }
        }
        if !is_taut {
            out.push(clause);
        }
    }
    out
}

fn bench_dpll(c: &mut Criterion) {
    let mut group = c.benchmark_group("DPLL");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(3));
    for &(vars, clauses) in &[(20usize, 60usize), (40, 120), (60, 180), (80, 240)] {
        let inst = random_3sat(vars, clauses, 42);
        group.bench_function(format!("dpll_{vars}vars_{clauses}cls"), |b| {
            b.iter(|| dpll_satisfiable(black_box(&inst)));
        });
    }
    group.finish();
}

fn bench_cdcl(c: &mut Criterion) {
    let mut group = c.benchmark_group("CDCL");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(3));
    for &(vars, clauses) in &[(20usize, 60usize), (40, 120), (60, 180), (80, 240)] {
        let inst = random_3sat(vars, clauses, 42);
        group.bench_function(format!("cdcl_{vars}vars_{clauses}cls"), |b| {
            b.iter(|| cdcl_satisfiable(black_box(&inst)));
        });
    }
    group.finish();
}

fn bench_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Resolution");
    group.sampling_mode(SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(3));
    // Ground unit clauses — fast path
    for &n in &[100usize, 500, 1000, 5000] {
        let mut clauses: Vec<Vec<Literal>> = Vec::new();
        for i in 0..n {
            clauses.push(vec![Literal {
                negated: i % 2 == 0,
                predicate: format!("p{i}"),
                args: vec![],
            }]);
        }
        // Add an unsatisfiable core
        clauses.push(vec![Literal {
            negated: false,
            predicate: "p0".into(),
            args: vec![],
        }]);
        clauses.push(vec![Literal {
            negated: true,
            predicate: "p0".into(),
            args: vec![],
        }]);
        group.bench_function(format!("ground_{n}"), |b| {
            b.iter(|| resolution_refute_bounded(black_box(&clauses), 2000));
        });
    }
    group.finish();
}

#[allow(dead_code)]
fn _term_marker(_: &Term) {}

criterion_group!(benches, bench_dpll, bench_cdcl, bench_resolution);
criterion_main!(benches);
