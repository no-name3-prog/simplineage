//! Integration tests for SQLite metadata storage.

use simplineage_analysis::LineageGraph;
use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
use simplineage_core::model::ids::FullyQualifiedName;
use simplineage_core::model::objects::{ObjectMeta, Schema, Table};
use simplineage_core::{ObjectId, Snapshot};
use simplineage_storage::{ImportMode, MetadataStore};
use tempfile::tempdir;

fn oid(s: &str) -> ObjectId {
    ObjectId::from_trusted(s)
}

fn dep(from: &str, to: &str) -> Dependency {
    Dependency {
        id: oid(&format!("{from}->{to}")),
        from_id: oid(from),
        to_id: oid(to),
        kind: DependencyKind::ViewDefinition,
        level: DependencyLevel::Relation,
        confidence: None,
        attributes: Default::default(),
    }
}

fn sample_snapshot(tag: &str) -> Snapshot {
    let mut s = Snapshot::new();
    s.label = Some(tag.into());
    s.source = Some("test".into());
    s.created_at = Some("2026-07-10T00:00:00Z".into());
    s.schemas.push(Schema {
        meta: ObjectMeta::new(
            oid("sch:public"),
            FullyQualifiedName::parse_dotted("public").unwrap(),
        ),
        database_id: None,
        catalog_id: None,
    });
    for name in ["orders", "facts"] {
        s.tables.push(Table {
            meta: ObjectMeta::new(
                oid(&format!("table:{name}")),
                FullyQualifiedName::parse_dotted(&format!("public.{name}")).unwrap(),
            ),
            schema_id: Some(oid("sch:public")),
            column_ids: vec![],
        });
    }
    s.dependencies.push(dep("table:orders", "table:facts"));
    s
}

#[test]
fn migrations_and_roundtrip() {
    let store = MetadataStore::open_in_memory().unwrap();
    assert_eq!(store.schema_version(), 1);
    assert_eq!(store.applied_migrations().unwrap(), vec![1]);

    let snap = sample_snapshot("v1");
    store.save_snapshot(&snap, true).unwrap();

    let loaded = store.load_snapshot(&snap.id).unwrap();
    assert_eq!(loaded.id, snap.id);
    assert_eq!(loaded.tables.len(), 2);
    assert_eq!(loaded.dependencies.len(), 1);

    let current = store.load_current().unwrap().unwrap();
    assert_eq!(current.id, snap.id);

    let list = store.list_snapshots().unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].is_current);
}

#[test]
fn incremental_merge_import() {
    let store = MetadataStore::open_in_memory().unwrap();
    let base = sample_snapshot("base");
    store
        .import(base, ImportMode::Replace, Some("csv:base"))
        .unwrap();

    let mut incoming = Snapshot::new();
    incoming.tables.push(Table {
        meta: ObjectMeta::new(
            oid("table:dim"),
            FullyQualifiedName::parse_dotted("public.dim").unwrap(),
        ),
        schema_id: Some(oid("sch:public")),
        column_ids: vec![],
    });
    incoming.dependencies.push(dep("table:dim", "table:facts"));

    let result = store
        .import(incoming, ImportMode::Merge, Some("csv:delta"))
        .unwrap();
    assert_eq!(result.mode, "merge");
    assert!(result.objects_added >= 1);

    let current = store.load_current().unwrap().unwrap();
    assert!(
        current
            .tables
            .iter()
            .any(|t| t.meta.id.as_str() == "table:dim")
    );
    assert!(
        current
            .tables
            .iter()
            .any(|t| t.meta.id.as_str() == "table:orders")
    );
    assert!(current.dependencies.len() >= 2);
}

#[test]
fn fast_graph_reconstruction_from_edges() {
    let dir = tempdir().unwrap();
    let store = MetadataStore::open(dir.path()).unwrap();
    let snap = sample_snapshot("graph");
    store.save_snapshot(&snap, true).unwrap();

    let deps = store.load_dependencies(&snap.id).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].from_id.as_str(), "table:orders");

    let g = LineageGraph::from_dependencies(deps);
    assert_eq!(g.edge_count(), 1);
    assert!(g.contains(&oid("table:orders")));
    assert!(g.contains(&oid("table:facts")));
}

#[test]
fn set_current_and_delete() {
    let store = MetadataStore::open_in_memory().unwrap();
    let a = sample_snapshot("a");
    let b = sample_snapshot("b");
    let id_a = a.id.clone();
    let id_b = b.id.clone();
    store.save_snapshot(&a, true).unwrap();
    store.save_snapshot(&b, true).unwrap();
    assert_eq!(store.load_current().unwrap().unwrap().id, id_b);
    store.set_current(&id_a).unwrap();
    assert_eq!(store.load_current().unwrap().unwrap().id, id_a);
    store.delete_snapshot(&id_b).unwrap();
    assert_eq!(store.list_snapshots().unwrap().len(), 1);
}
