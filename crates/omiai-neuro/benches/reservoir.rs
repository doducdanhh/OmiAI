//! Benchmarks for the Echo State Network reservoir: per-step compute and
//! RLS readout training throughput.
//!
//! Run with: `cargo bench --bench reservoir`

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use omiai_neuro::reservoir::Reservoir;

fn bench_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("ESN_step");
    group.measurement_time(Duration::from_secs(3));
    for &size in &[100usize, 500, 1000, 2000] {
        let mut r = Reservoir::new(size, 1, 1, 0.9, 42);
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &_size| {
            b.iter(|| {
                let _ = r.step(black_box(&[0.3]));
            });
        });
    }
    group.finish();
}

fn bench_rls_train(c: &mut Criterion) {
    let mut group = c.benchmark_group("ESN_rls_train");
    group.measurement_time(Duration::from_secs(5));
    for &size in &[100usize, 200, 500] {
        let r = Reservoir::new(size, 1, 1, 0.9, 7);
        let inputs: Vec<Vec<f64>> = (0..100).map(|t| vec![(t as f64 * 0.1).sin()]).collect();
        let targets: Vec<Vec<f64>> = inputs.iter().map(|u| vec![u[0] * 0.5]).collect();
        group.throughput(Throughput::Elements((100 * size) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &_size| {
            b.iter(|| {
                let mut r2 = r.clone();
                r2.train_readout(black_box(&inputs), black_box(&targets));
                black_box(r2.state().to_vec());
            });
        });
    }
    group.finish();
}

fn bench_ridge_readout(c: &mut Criterion) {
    let mut group = c.benchmark_group("ESN_ridge_readout");
    group.measurement_time(Duration::from_secs(3));
    for &size in &[50usize, 100, 200] {
        let mut r = Reservoir::new(size, 1, 1, 0.9, 11);
        let states: Vec<Vec<f64>> = (0..50)
            .map(|_| (0..size).map(|i| (i as f64 * 0.01).sin()).collect())
            .collect();
        let targets: Vec<Vec<f64>> = states
            .iter()
            .map(|s| vec![s.iter().sum::<f64>() * 0.1])
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &_size| {
            b.iter(|| {
                r.train_readout_ridge(black_box(&states), black_box(&targets), 0.01);
            });
        });
    }
    group.finish();
}

fn bench_lyapunov(c: &mut Criterion) {
    let mut group = c.benchmark_group("ESN_lyapunov");
    group.measurement_time(Duration::from_secs(3));
    for &size in &[100usize, 200, 500] {
        let r = Reservoir::new(size, 1, 1, 0.9, 5);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &_size| {
            b.iter(|| black_box(r.largest_lyapunov_exponent()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_step,
    bench_rls_train,
    bench_ridge_readout,
    bench_lyapunov
);
criterion_main!(benches);
