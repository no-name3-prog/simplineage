//! Storage performance benchmarks (SQLite).

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use simplineage_analysis::LineageGraph;
use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
use simplineage_core::model::ids::FullyQualifiedName;
use simplineage_core::model::objects::{ObjectMeta, Table};
use simplineage_core::{ObjectId, Snapshot};
use simplineage_storage::{ImportMode, MetadataStore};
use tempfile::tempdir;

fn oid(s: impl Into<String>) -> ObjectId {
    ObjectId::from_trusted(s.into())
}

fn large_snapshot(n_tables: usize, n_edges: usize) -> Snapshot {
    let mut s = Snapshot::new();
    s.label = Some(format!("bench-{n_tables}"));
    for i in 0..n_tables {
        s.tables.push(Table {
            meta: ObjectMeta::new(
                oid(format!("table:{i}")),
                FullyQualifiedName::parse_dotted(&format!("public.t{i}")).unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
        });
    }
    for i in 0..n_edges.min(n_tables.saturating_sub(1)) {
        s.dependencies.push(Dependency {
            id: oid(format!("e{i}")),
            from_id: oid(format!("table:{i}")),
            to_id: oid(format!("table:{}", i + 1)),
            kind: DependencyKind::Manual,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });
    }
    s
}

fn bench_save_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_save_load");
    for n in [100usize, 500, 1_000] {
        let snap = large_snapshot(n, n.saturating_sub(1));
        group.bench_with_input(BenchmarkId::new("save_in_memory", n), &snap, |b, snap| {
            b.iter(|| {
                let store = MetadataStore::open_in_memory().unwrap();
                store.save_snapshot(black_box(snap), true).unwrap();
            });
        });
        group.bench_with_input(BenchmarkId::new("load_in_memory", n), &snap, |b, snap| {
            let store = MetadataStore::open_in_memory().unwrap();
            store.save_snapshot(snap, true).unwrap();
            let id = snap.id.clone();
            b.iter(|| {
                let loaded = store.load_snapshot(black_box(&id)).unwrap();
                black_box(loaded.object_count());
            });
        });
    }
    group.finish();
}

fn bench_edges_vs_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_graph_reconstruct");
    let snap = large_snapshot(2_000, 1_999);
    let store = MetadataStore::open_in_memory().unwrap();
    store.save_snapshot(&snap, true).unwrap();
    let id = snap.id.clone();

    group.bench_function("load_full_snapshot_json", |b| {
        b.iter(|| {
            let s = store.load_snapshot(black_box(&id)).unwrap();
            black_box(s.dependencies.len());
        });
    });
    group.bench_function("load_edges_only", |b| {
        b.iter(|| {
            let deps = store.load_dependencies(black_box(&id)).unwrap();
            black_box(deps.len());
        });
    });
    group.bench_function("edges_to_lineage_graph", |b| {
        b.iter(|| {
            let deps = store.load_dependencies(black_box(&id)).unwrap();
            let g = LineageGraph::from_dependencies(deps);
            black_box(g.edge_count());
        });
    });
    group.finish();
}

fn bench_incremental_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_incremental");
    let base = large_snapshot(500, 499);
    group.bench_function("merge_import_100_delta", |b| {
        b.iter(|| {
            let store = MetadataStore::open_in_memory().unwrap();
            store
                .import(base.clone(), ImportMode::Replace, Some("base"))
                .unwrap();
            let mut delta = Snapshot::new();
            for i in 500..600 {
                delta.tables.push(Table {
                    meta: ObjectMeta::new(
                        oid(format!("table:{i}")),
                        FullyQualifiedName::parse_dotted(&format!("public.t{i}")).unwrap(),
                    ),
                    schema_id: None,
                    column_ids: vec![],
                });
            }
            let r = store
                .import(delta, ImportMode::Merge, Some("delta"))
                .unwrap();
            black_box(r.objects_added);
        });
    });
    group.finish();
}

fn bench_file_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_file");
    let snap = large_snapshot(1_000, 999);
    group.bench_function("save_load_file_1k", |b| {
        b.iter(|| {
            let dir = tempdir().unwrap();
            let store = MetadataStore::open(dir.path()).unwrap();
            store.save_snapshot(black_box(&snap), true).unwrap();
            let loaded = store.load_current().unwrap().unwrap();
            black_box(loaded.tables.len());
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_save_load,
    bench_edges_vs_full,
    bench_incremental_merge,
    bench_file_store
);
criterion_main!(benches);
