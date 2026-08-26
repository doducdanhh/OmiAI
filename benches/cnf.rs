//! Benchmarks for the propositional / first-order CNF normalization
//! pipeline (`eliminate_iff_implies` → `to_nnf` → `skolemize` → drop ∀
//! → distribute).
//!
//! Run with: `cargo bench --bench cnf`

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use omiai::core::logic_engine::{self, Formula, Term};

/// Build a balanced binary propositional formula of depth `d`.
fn balanced(d: usize) -> Formula {
    fn go(d: usize, var_idx: &mut usize) -> Formula {
        if d == 0 {
            let v = *var_idx;
            *var_idx += 1;
            Formula::Atom(format!("p{v}"), vec![])
        } else {
            let a = go(d - 1, var_idx);
            let b = go(d - 1, var_idx);
            Formula::And(Box::new(a), Box::new(b))
        }
    }
    let mut idx = 0;
    go(d, &mut idx)
}

/// Build a quantified formula `∀x₁…∀xₙ (∃y P(x₁,…,xₙ, y))` to exercise
/// Skolemization at scale.
fn forall_exists(n: usize) -> Formula {
    let mk_term = |i: usize| Term::Var(format!("x{i}"));
    let mut body_args: Vec<Term> = (0..n).map(mk_term).collect();
    body_args.push(Term::Var("y".into()));
    let inner = Formula::Atom("P".into(), body_args);
    let exists = Formula::Exists("y".into(), Box::new(inner));
    let mut acc = exists;
    for i in (0..n).rev() {
        acc = Formula::ForAll(format!("x{i}"), Box::new(acc));
    }
    acc
}

/// Alternating `P ∨ ¬Q ∧ R → (¬P ↔ S) ⋯` formula to stress NNF.
fn alternating(n: usize) -> Formula {
    let mut acc = Formula::Atom("P".into(), vec![]);
    for i in 0..n {
        let pos = Formula::Atom(format!("p{i}"), vec![]);
        let neg = Formula::Not(Box::new(pos));
        acc = if i % 2 == 0 {
            Formula::Or(Box::new(acc), Box::new(neg))
        } else {
            Formula::Implies(Box::new(acc), Box::new(neg))
        };
    }
    acc
}

fn bench_cnf_propositional(c: &mut Criterion) {
    let mut group = c.benchmark_group("CNF_propositional");
    group.measurement_time(Duration::from_secs(3));
    for &depth in &[3usize, 5, 7, 9] {
        let f = balanced(depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &_depth| {
            b.iter(|| black_box(logic_engine::normalize_cnf(&f).unwrap()));
        });
    }
    group.finish();
}

fn bench_cnf_skolemize(c: &mut Criterion) {
    let mut group = c.benchmark_group("CNF_skolemize");
    group.measurement_time(Duration::from_secs(3));
    for &n in &[3usize, 5, 8, 12] {
        let f = forall_exists(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &_n| {
            b.iter(|| black_box(logic_engine::normalize_cnf(&f).unwrap()));
        });
    }
    group.finish();
}

fn bench_cnf_mixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("CNF_mixed");
    group.measurement_time(Duration::from_secs(3));
    for &n in &[5usize, 10, 20, 40] {
        let f = alternating(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &_n| {
            b.iter(|| black_box(logic_engine::normalize_cnf(&f).unwrap()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cnf_propositional,
    bench_cnf_skolemize,
    bench_cnf_mixed
);
criterion_main!(benches);
