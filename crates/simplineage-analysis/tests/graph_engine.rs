//! Integration tests for the lineage graph engine.

use simplineage_analysis::{LineageGraph, TraversalOptions};
use simplineage_core::Snapshot;
use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
use simplineage_core::model::ids::{FullyQualifiedName, ObjectId};
use simplineage_core::model::objects::{ObjectMeta, Schema, Table};

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

#[test]
fn from_snapshot_includes_isolated_nodes() {
    let mut snap = Snapshot::new();
    snap.schemas.push(Schema {
        meta: ObjectMeta::new(
            oid("schema:public"),
            FullyQualifiedName::parse_dotted("public").unwrap(),
        ),
        database_id: None,
        catalog_id: None,
    });
    snap.tables.push(Table {
        meta: ObjectMeta::new(
            oid("table:orphan"),
            FullyQualifiedName::parse_dotted("public.orphan").unwrap(),
        ),
        schema_id: Some(oid("schema:public")),
        column_ids: vec![],
    });
    snap.tables.push(Table {
        meta: ObjectMeta::new(
            oid("table:orders"),
            FullyQualifiedName::parse_dotted("public.orders").unwrap(),
        ),
        schema_id: Some(oid("schema:public")),
        column_ids: vec![],
    });
    snap.tables.push(Table {
        meta: ObjectMeta::new(
            oid("table:facts"),
            FullyQualifiedName::parse_dotted("public.facts").unwrap(),
        ),
        schema_id: Some(oid("schema:public")),
        column_ids: vec![],
    });
    snap.dependencies.push(dep("table:orders", "table:facts"));

    let g = LineageGraph::from_snapshot(&snap);
    assert!(g.contains(&oid("table:orphan")));
    assert!(g.contains(&oid("table:orders")));
    let down = g
        .downstream(&oid("table:orders"), &TraversalOptions::default())
        .unwrap();
    assert_eq!(down, vec![oid("table:facts")]);
}

#[test]
fn diamond_all_paths() {
    // A→B→D, A→C→D
    let g = LineageGraph::from_dependencies([
        dep("A", "B"),
        dep("A", "C"),
        dep("B", "D"),
        dep("C", "D"),
    ]);
    let paths = g
        .all_paths(&oid("A"), &oid("D"), &TraversalOptions::default())
        .unwrap();
    assert_eq!(paths.len(), 2);
    let sp = g.shortest_path(&oid("A"), &oid("D")).unwrap().unwrap();
    assert_eq!(sp.len(), 3);
}

#[test]
fn no_path_returns_none() {
    let g = LineageGraph::from_dependencies([dep("A", "B"), dep("C", "D")]);
    assert!(g.shortest_path(&oid("A"), &oid("D")).unwrap().is_none());
    assert!(
        g.all_paths(&oid("A"), &oid("D"), &TraversalOptions::default())
            .unwrap()
            .is_empty()
    );
}
