//! Integration tests for the Phase 4 analysis engine.

use simplineage_analysis::{
    AnalysisEngine, ChainOptions, CriticalOptions, ImpactDirection, ImpactOptions,
};
use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
use simplineage_core::model::ids::FullyQualifiedName;
use simplineage_core::model::objects::{ObjectMeta, Schema, Table};
use simplineage_core::{ObjectId, Snapshot};

fn oid(s: &str) -> ObjectId {
    ObjectId::from_trusted(s)
}

fn dep(from: &str, to: &str) -> Dependency {
    Dependency {
        id: oid(&format!("{from}->{to}")),
        from_id: oid(from),
        to_id: oid(to),
        kind: DependencyKind::Manual,
        level: DependencyLevel::Relation,
        confidence: None,
        attributes: Default::default(),
    }
}

fn catalog() -> Snapshot {
    let mut s = Snapshot::new();
    s.schemas.push(Schema {
        meta: ObjectMeta::new(
            oid("sch:public"),
            FullyQualifiedName::parse_dotted("public").unwrap(),
        ),
        database_id: None,
        catalog_id: None,
    });
    for name in [
        "raw_orders",
        "stg_orders",
        "dim_customer",
        "fct_sales",
        "unused_export",
    ] {
        s.tables.push(Table {
            meta: ObjectMeta::new(
                oid(&format!("table:{name}")),
                FullyQualifiedName::parse_dotted(&format!("public.{name}")).unwrap(),
            ),
            schema_id: Some(oid("sch:public")),
            column_ids: vec![],
        });
    }
    // raw → stg → fct; dim → fct; unused_export isolated
    s.dependencies.extend([
        dep("table:raw_orders", "table:stg_orders"),
        dep("table:stg_orders", "table:fct_sales"),
        dep("table:dim_customer", "table:fct_sales"),
    ]);
    s
}

#[test]
fn end_to_end_analysis_api() {
    let engine = AnalysisEngine::from_snapshot(catalog());

    let impact = engine
        .impact(
            &oid("table:stg_orders"),
            &ImpactOptions {
                direction: ImpactDirection::Both,
                relations_only: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        impact
            .upstream
            .iter()
            .any(|i| i.as_str() == "table:raw_orders")
    );
    assert!(
        impact
            .downstream
            .iter()
            .any(|i| i.as_str() == "table:fct_sales")
    );

    let validation = engine.validate_dependencies();
    assert!(validation.ok);

    let orphans = engine.orphans();
    assert!(
        orphans
            .iter()
            .any(|o| o.id.as_str() == "table:unused_export")
    );

    // fct_sales is a sink (has upstream, no downstream) → unused
    let unused = engine.unused_objects();
    assert!(unused.iter().any(|o| o.id.as_str() == "table:fct_sales"));

    assert!(engine.circular_dependencies().is_empty());

    let critical = engine.critical_tables(&CriticalOptions {
        limit: 3,
        relations_only: true,
        min_downstream: 1,
    });
    assert!(!critical.is_empty());
    // raw_orders or stg_orders should rank high
    assert!(
        critical[0].id.as_str() == "table:raw_orders"
            || critical[0].id.as_str() == "table:stg_orders"
            || critical[0].id.as_str() == "table:dim_customer"
    );

    let chains = engine.longest_chains(&ChainOptions {
        limit: 5,
        max_length: 32,
    });
    assert!(!chains.is_empty());
    assert!(chains[0].length >= 2); // raw → stg → fct

    let quality = engine.quality_checks();
    assert!((0.0..=100.0).contains(&quality.score));

    let full = engine.analyze_all();
    assert_eq!(full.orphans.len(), orphans.len());
    assert!(full.statistics.edge_count >= 3);
}

#[test]
fn detects_cycle_in_analysis() {
    let mut s = catalog();
    s.dependencies
        .push(dep("table:fct_sales", "table:stg_orders"));
    let engine = AnalysisEngine::from_snapshot(s);
    let cycles = engine.circular_dependencies();
    assert!(!cycles.is_empty());
    let q = engine.quality_checks();
    assert!(q.checks.iter().any(|c| c.id == "no_cycles" && !c.passed));
}
