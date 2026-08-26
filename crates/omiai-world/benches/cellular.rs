//! Benchmarks for the reversible block cellular automaton.
//!
//! Varies grid size from 64×64 to 1024×1024 and step count, measuring
//! per-step throughput on a multi-core machine (rayon parallel sweep).
//!
//! Run with: `cargo bench --bench cellular`

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use omiai_world::substrate::CellularAutomaton;

fn bench_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("CA_step");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(20);
    for &size in &[64usize, 128, 256, 512, 1024] {
        let mut ca = CellularAutomaton::random(size, size, 0.3, 7);
        group.throughput(Throughput::Elements((size * size) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &_size| {
            b.iter(|| {
                ca.step();
                black_box(ca.population());
            });
        });
    }
    group.finish();
}

fn bench_steps_burst(c: &mut Criterion) {
    let mut group = c.benchmark_group("CA_steps_burst");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(15);
    for &(size, n) in &[(128usize, 10usize), (256, 10), (512, 10), (1024, 5)] {
        let mut ca = CellularAutomaton::random(size, size, 0.25, 99);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("size{}_n{}", size, n)),
            &n,
            |b, &n| {
                b.iter(|| {
                    ca.steps(black_box(n));
                    black_box(ca.population());
                });
            },
        );
    }
    group.finish();
}

fn bench_pattern_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("CA_patterns");
    group.measurement_time(Duration::from_secs(3));
    for &size in &[64usize, 128, 256] {
        let mut ca = CellularAutomaton::random(size, size, 0.3, 13);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &_size| {
            b.iter(|| black_box(ca.detect_patterns()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_step,
    bench_steps_burst,
    bench_pattern_detection
);
criterion_main!(benches);
