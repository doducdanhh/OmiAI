//! Benchmarks for Cartesian Genetic Programming: evolutionary search
//! throughput at varying population / generation counts.
//!
//! Run with: `cargo bench --bench cgp`

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use omiai_evolution::fitness::mse_to_fitness;
use omiai_evolution::genetic_programming::GeneticProgram;

fn bench_evolve(c: &mut Criterion) {
    let mut group = c.benchmark_group("CGP_evolve");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(15);
    // Symbolically fit y = x^2 on [-1,1]
    let data: Vec<(f64, f64)> = (-50..=50)
        .map(|i| {
            let x = i as f64 / 50.0;
            (x, x * x)
        })
        .collect();
    for &(pop, gens) in &[(32usize, 10usize), (64, 20), (128, 30), (256, 50)] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("pop{}_gens{}", pop, gens)),
            &(pop, gens),
            |b, &(pop, gens)| {
                b.iter(|| {
                    let best = GeneticProgram::evolve(
                        black_box(pop),
                        2,
                        black_box(gens),
                        1,
                        12,
                        1,
                        |prog| {
                            let preds: Vec<f64> =
                                data.iter().map(|(x, _)| prog.eval(&[*x])[0]).collect();
                            let targets: Vec<f64> = data.iter().map(|(_, y)| *y).collect();
                            mse_to_fitness(&preds, &targets)
                        },
                        17,
                    );
                    black_box(best.eval(&[0.5])[0]);
                });
            },
        );
    }
    group.finish();
}

fn bench_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("CGP_eval");
    group.measurement_time(Duration::from_secs(3));
    for &nodes in &[10usize, 50, 200, 1000] {
        let prog = GeneticProgram::random(1, nodes, 1, &mut rand::thread_rng());
        let input = vec![0.42];
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &nodes, |b, &_nodes| {
            b.iter(|| black_box(prog.eval(black_box(&input))));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_evolve, bench_eval);
criterion_main!(benches);
