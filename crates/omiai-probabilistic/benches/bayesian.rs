//! Benchmarks for Bayesian network inference: exact variable elimination
//! and approximate Metropolis–Hastings MCMC.
//!
//! Run with: `cargo bench --bench bayesian`

use std::collections::HashMap;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use omiai_probabilistic::bayesian::{BayesianNetwork, Cpt};

/// Build a chain Bayesian network X1 → X2 → … → Xn with binary parents.
fn build_chain(n: usize) -> BayesianNetwork {
    let mut bn = BayesianNetwork::new();
    bn.add_node(Cpt {
        variable: "X0".into(),
        parents: vec![],
        probs_true: vec![0.3],
    });
    for i in 1..n {
        bn.add_node(Cpt {
            variable: format!("X{i}"),
            parents: vec![format!("X{}", i - 1)],
            probs_true: vec![0.2, 0.7],
        });
    }
    bn
}

/// Build a "alarm"-style network: Burglary → Alarm ← Earthquake; Alarm → JohnCalls / MaryCalls.
fn build_alarm() -> BayesianNetwork {
    let mut bn = BayesianNetwork::new();
    bn.add_node(Cpt {
        variable: "Burglary".into(),
        parents: vec![],
        probs_true: vec![0.01],
    });
    bn.add_node(Cpt {
        variable: "Earthquake".into(),
        parents: vec![],
        probs_true: vec![0.02],
    });
    // P(Alarm | B, E): index bits B=0, E=1
    bn.add_node(Cpt {
        variable: "Alarm".into(),
        parents: vec!["Burglary".into(), "Earthquake".into()],
        probs_true: vec![0.001, 0.29, 0.94, 0.95],
    });
    bn.add_node(Cpt {
        variable: "JohnCalls".into(),
        parents: vec!["Alarm".into()],
        probs_true: vec![0.05, 0.90],
    });
    bn.add_node(Cpt {
        variable: "MaryCalls".into(),
        parents: vec!["Alarm".into()],
        probs_true: vec![0.01, 0.70],
    });
    bn
}

fn bench_ve_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("BN_VE_chain");
    group.measurement_time(Duration::from_secs(3));
    for &n in &[4usize, 6, 8, 10] {
        let bn = build_chain(n);
        let mut ev = HashMap::new();
        ev.insert(format!("X{}", n - 1), true);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &_n| {
            b.iter(|| black_box(bn.variable_elimination("X0", &ev)));
        });
    }
    group.finish();
}

fn bench_ve_alarm(c: &mut Criterion) {
    let bn = build_alarm();
    let mut ev = HashMap::new();
    ev.insert("JohnCalls".into(), true);
    ev.insert("MaryCalls".into(), true);
    c.bench_function("BN_VE_alarm_Burglary", |b| {
        b.iter(|| black_box(bn.variable_elimination("Burglary", &ev)));
    });
    let mut ev2 = HashMap::new();
    ev2.insert("Burglary".into(), true);
    c.bench_function("BN_VE_alarm_Alarm", |b| {
        b.iter(|| black_box(bn.variable_elimination("Alarm", &ev2)));
    });
}

fn bench_mcmc_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("BN_MCMC_chain");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(15);
    for &n in &[4usize, 6, 8] {
        let bn = build_chain(n);
        let ev = HashMap::new();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &_n| {
            b.iter(|| black_box(bn.mcmc("X0", &ev, 500)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_ve_chain, bench_ve_alarm, bench_mcmc_chain);
criterion_main!(benches);
