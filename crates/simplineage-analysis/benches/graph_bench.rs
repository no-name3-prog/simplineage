//! Benchmarks for lineage graph construction and traversal.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use simplineage_analysis::{LineageGraph, TraversalOptions};
use simplineage_core::ObjectId;
use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};

fn oid(s: impl Into<String>) -> ObjectId {
    ObjectId::from_trusted(s.into())
}

/// Chain: n0 → n1 → … → n_{size-1}
fn chain_deps(size: usize) -> Vec<Dependency> {
    (0..size.saturating_sub(1))
        .map(|i| Dependency {
            id: oid(format!("e{i}")),
            from_id: oid(format!("n{i}")),
            to_id: oid(format!("n{}", i + 1)),
            kind: DependencyKind::Manual,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        })
        .collect()
}

/// Layered DAG: `width` nodes per layer, `depth` layers; full bipartite edges between layers.
fn layered_deps(width: usize, depth: usize) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut e = 0usize;
    for layer in 0..depth.saturating_sub(1) {
        for a in 0..width {
            for b in 0..width {
                deps.push(Dependency {
                    id: oid(format!("e{e}")),
                    from_id: oid(format!("L{layer}_{a}")),
                    to_id: oid(format!("L{}_{b}", layer + 1)),
                    kind: DependencyKind::Inferred,
                    level: DependencyLevel::Relation,
                    confidence: None,
                    attributes: Default::default(),
                });
                e += 1;
            }
        }
    }
    deps
}

fn bench_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_construction");
    for size in [100usize, 1_000, 5_000, 10_000] {
        let deps = chain_deps(size);
        group.bench_with_input(BenchmarkId::new("chain", size), &deps, |b, deps| {
            b.iter(|| {
                let g = LineageGraph::from_dependencies(deps.iter().cloned());
                black_box(g.node_count());
            });
        });
    }
    // denser layered graph
    let layered = layered_deps(20, 20); // 20*20*19 = 7600 edges
    group.bench_function("layered_20x20", |b| {
        b.iter(|| {
            let g = LineageGraph::from_dependencies(layered.iter().cloned());
            black_box(g.edge_count());
        });
    });
    group.finish();
}

fn bench_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_traversal");
    let chain = LineageGraph::from_dependencies(chain_deps(5_000));
    let start = oid("n0");
    let mid = oid("n2500");
    let end = oid("n4999");
    let opts = TraversalOptions::default();

    group.bench_function("downstream_chain_5k", |b| {
        b.iter(|| {
            let v = chain
                .downstream(black_box(&start), black_box(&opts))
                .unwrap();
            black_box(v.len());
        });
    });
    group.bench_function("upstream_chain_5k", |b| {
        b.iter(|| {
            let v = chain.upstream(black_box(&end), black_box(&opts)).unwrap();
            black_box(v.len());
        });
    });
    group.bench_function("shortest_path_chain_5k", |b| {
        b.iter(|| {
            let p = chain
                .shortest_path(black_box(&start), black_box(&end))
                .unwrap();
            black_box(p.as_ref().map(|x| x.len()));
        });
    });
    group.bench_function("shortest_path_mid_chain_5k", |b| {
        b.iter(|| {
            let p = chain
                .shortest_path(black_box(&start), black_box(&mid))
                .unwrap();
            black_box(p.as_ref().map(|x| x.len()));
        });
    });

    let layered = LineageGraph::from_dependencies(layered_deps(15, 15));
    let l_start = oid("L0_0");
    let l_end = oid("L14_7");
    group.bench_function("downstream_layered_15x15", |b| {
        b.iter(|| {
            let v = layered
                .downstream(black_box(&l_start), black_box(&opts))
                .unwrap();
            black_box(v.len());
        });
    });
    group.bench_function("all_paths_layered_bounded", |b| {
        let path_opts = TraversalOptions {
            max_paths: 50,
            max_path_length: Some(20),
            ..Default::default()
        };
        b.iter(|| {
            let paths = layered
                .all_paths(
                    black_box(&l_start),
                    black_box(&l_end),
                    black_box(&path_opts),
                )
                .unwrap();
            black_box(paths.len());
        });
    });
    group.bench_function("detect_cycles_chain_5k", |b| {
        b.iter(|| {
            let cyc = chain.detect_cycles();
            black_box(cyc.len());
        });
    });
    group.bench_function("statistics_chain_5k", |b| {
        b.iter(|| {
            let s = chain.statistics();
            black_box(s.edge_count);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_construction, bench_traversal);
criterion_main!(benches);
