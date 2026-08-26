//! Benchmarks for knowledge graph and triple store operations.
//!
//! Run with: `cargo bench --bench knowledge`

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use omiai::knowledge::graph::{Concept, KnowledgeGraph};
use omiai::knowledge::triple::{TermPattern, Triple, TriplePattern, TripleStore};

/// Build a chain graph: 0 → 1 → 2 → … → n-1.
fn build_chain_graph(n: usize) -> KnowledgeGraph {
    let mut g = KnowledgeGraph::new();
    for i in 0..n {
        g.add_concept(Concept {
            id: format!("n{i}"),
            label: format!("N{i}"),
        });
    }
    for i in 0..n.saturating_sub(1) {
        g.add_relation(&format!("n{i}"), &format!("n{}", i + 1), "partOf")
            .unwrap();
    }
    g
}

fn bench_path_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("KG_path");
    group.measurement_time(Duration::from_secs(3));
    for &n in &[100usize, 1_000, 10_000] {
        let g = build_chain_graph(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &_n| {
            b.iter(|| {
                let p = g.query_path("n0", &format!("n{}", n - 1));
                black_box(p);
            });
        });
    }
    group.finish();
}

fn bench_transitive_closure(c: &mut Criterion) {
    let mut group = c.benchmark_group("KG_transitive");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10);
    for &n in &[20usize, 50, 100] {
        let g = build_chain_graph(n);
        group.throughput(Throughput::Elements((n * n) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &_n| {
            b.iter(|| black_box(g.infer_transitive("partOf")));
        });
    }
    group.finish();
}

fn bench_triple_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("Triple_insert");
    group.measurement_time(Duration::from_secs(3));
    for &n in &[1_000usize, 10_000, 50_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut s = TripleStore::new();
                for i in 0..n {
                    s.insert(Triple {
                        subject: format!("s{i}"),
                        predicate: format!("p{}", i % 5),
                        object: format!("o{i}"),
                    });
                }
                black_box(s.len());
            });
        });
    }
    group.finish();
}

fn bench_triple_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("Triple_match");
    group.measurement_time(Duration::from_secs(3));
    for &n in &[1_000usize, 10_000, 50_000] {
        let mut s = TripleStore::new();
        for i in 0..n {
            s.insert(Triple {
                subject: format!("s{i}"),
                predicate: format!("p{}", i % 5),
                object: format!("o{i}"),
            });
        }
        let pat = TriplePattern {
            subject: TermPattern::Var("?s".into()),
            predicate: TermPattern::Bound("p2".into()),
            object: TermPattern::Var("?o".into()),
        };
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &_n| {
            b.iter(|| black_box(s.match_pattern(&pat)));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_path_query,
    bench_transitive_closure,
    bench_triple_insert,
    bench_triple_match
);
criterion_main!(benches);
