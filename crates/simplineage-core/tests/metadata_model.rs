//! Integration tests for the Phase 1 core metadata model.

use simplineage_core::model::graph::{
    ColumnMapping, Dependency, DependencyKind, DependencyLevel, Relationship, RelationshipKind,
};
use simplineage_core::model::ids::{FullyQualifiedName, ObjectId};
use simplineage_core::model::objects::{
    Catalog, Column, Database, MaterializedView, ObjectMeta, Schema, Table, View,
};
use simplineage_core::model::types::DataType;
use simplineage_core::model::{MODEL_VERSION, ModelVersion, Snapshot, Validate, validate_snapshot};

fn oid(s: &str) -> ObjectId {
    ObjectId::from_trusted(s)
}

fn meta(id: &str, fqn: &str) -> ObjectMeta {
    ObjectMeta::new(oid(id), FullyQualifiedName::parse_dotted(fqn).unwrap())
}

/// Full hierarchy with relationships and dependencies.
fn rich_snapshot() -> Snapshot {
    let mut s = Snapshot::new();
    s.label = Some("integration".into());
    s.created_at = Some("2026-07-10T12:00:00Z".into());
    s.source = Some("tests".into());

    s.catalogs.push(Catalog {
        meta: meta("cat:main", "main"),
    });
    s.databases.push(Database {
        meta: meta("db:analytics", "main.analytics"),
        catalog_id: Some(oid("cat:main")),
    });
    s.schemas.push(Schema {
        meta: meta("sch:public", "main.analytics.public"),
        database_id: Some(oid("db:analytics")),
        catalog_id: None,
    });

    s.tables.push(Table {
        meta: meta("tbl:customers", "main.analytics.public.customers"),
        schema_id: Some(oid("sch:public")),
        column_ids: vec![oid("col:customers.id"), oid("col:customers.email")],
    });
    s.tables.push(Table {
        meta: meta("tbl:orders", "main.analytics.public.orders"),
        schema_id: Some(oid("sch:public")),
        column_ids: vec![oid("col:orders.id"), oid("col:orders.customer_id")],
    });

    s.views.push(View {
        meta: meta("view:order_summary", "main.analytics.public.order_summary"),
        schema_id: Some(oid("sch:public")),
        column_ids: vec![oid("col:order_summary.customer_id")],
        definition: Some("SELECT customer_id FROM orders".into()),
    });

    s.materialized_views.push(MaterializedView {
        meta: meta("mv:order_facts", "main.analytics.public.order_facts"),
        schema_id: Some(oid("sch:public")),
        column_ids: vec![oid("col:order_facts.customer_id")],
        definition: Some("SELECT customer_id FROM orders".into()),
    });

    s.columns.extend([
        Column {
            meta: meta("col:customers.id", "main.analytics.public.customers.id"),
            parent_id: oid("tbl:customers"),
            ordinal: Some(0),
            data_type: DataType::Integer { bits: Some(64) },
            nullable: false,
            is_primary_key: Some(true),
            raw_type: Some("BIGINT".into()),
        },
        Column {
            meta: meta(
                "col:customers.email",
                "main.analytics.public.customers.email",
            ),
            parent_id: oid("tbl:customers"),
            ordinal: Some(1),
            data_type: DataType::String {
                max_length: Some(320),
                is_char_length: Some(true),
            },
            nullable: true,
            is_primary_key: Some(false),
            raw_type: Some("VARCHAR(320)".into()),
        },
        Column {
            meta: meta("col:orders.id", "main.analytics.public.orders.id"),
            parent_id: oid("tbl:orders"),
            ordinal: Some(0),
            data_type: DataType::Integer { bits: Some(64) },
            nullable: false,
            is_primary_key: Some(true),
            raw_type: None,
        },
        Column {
            meta: meta(
                "col:orders.customer_id",
                "main.analytics.public.orders.customer_id",
            ),
            parent_id: oid("tbl:orders"),
            ordinal: Some(1),
            data_type: DataType::Integer { bits: Some(64) },
            nullable: false,
            is_primary_key: Some(false),
            raw_type: None,
        },
        Column {
            meta: meta(
                "col:order_summary.customer_id",
                "main.analytics.public.order_summary.customer_id",
            ),
            parent_id: oid("view:order_summary"),
            ordinal: Some(0),
            data_type: DataType::Integer { bits: Some(64) },
            nullable: true,
            is_primary_key: None,
            raw_type: None,
        },
        Column {
            meta: meta(
                "col:order_facts.customer_id",
                "main.analytics.public.order_facts.customer_id",
            ),
            parent_id: oid("mv:order_facts"),
            ordinal: Some(0),
            data_type: DataType::Integer { bits: Some(64) },
            nullable: true,
            is_primary_key: None,
            raw_type: None,
        },
    ]);

    s.relationships.push(Relationship {
        id: oid("rel:orders_customer_fk"),
        kind: RelationshipKind::ForeignKey,
        from_id: oid("tbl:orders"),
        to_id: oid("tbl:customers"),
        column_mappings: vec![ColumnMapping {
            from_column_id: oid("col:orders.customer_id"),
            to_column_id: oid("col:customers.id"),
        }],
        name: Some("orders_customer_fk".into()),
        attributes: Default::default(),
    });

    s.dependencies.push(Dependency {
        id: oid("dep:view_orders"),
        from_id: oid("tbl:orders"),
        to_id: oid("view:order_summary"),
        kind: DependencyKind::ViewDefinition,
        level: DependencyLevel::Relation,
        confidence: None,
        attributes: Default::default(),
    });
    s.dependencies.push(Dependency {
        id: oid("dep:mv_orders"),
        from_id: oid("tbl:orders"),
        to_id: oid("mv:order_facts"),
        kind: DependencyKind::ViewDefinition,
        level: DependencyLevel::Relation,
        confidence: None,
        attributes: Default::default(),
    });
    s.dependencies.push(Dependency {
        id: oid("dep:col_lineage"),
        from_id: oid("col:orders.customer_id"),
        to_id: oid("col:order_summary.customer_id"),
        kind: DependencyKind::Inferred,
        level: DependencyLevel::Column,
        confidence: Some(simplineage_core::model::OrderedFloatCompat::clamped(0.9)),
        attributes: Default::default(),
    });

    s
}

#[test]
fn rich_snapshot_validates() {
    let s = rich_snapshot();
    assert_eq!(s.model_version.as_str(), MODEL_VERSION);
    validate_snapshot(&s).unwrap();
    assert!(s.object_count() >= 10);
}

#[test]
fn json_roundtrip_rich_snapshot() {
    let s = rich_snapshot();
    let json = s.to_json_pretty().unwrap();
    let back = Snapshot::from_json(json.as_bytes()).unwrap();
    back.validate().unwrap();
    assert_eq!(s.catalogs.len(), back.catalogs.len());
    assert_eq!(s.tables.len(), back.tables.len());
    assert_eq!(s.views.len(), back.views.len());
    assert_eq!(s.materialized_views.len(), back.materialized_views.len());
    assert_eq!(s.columns.len(), back.columns.len());
    assert_eq!(s.relationships.len(), back.relationships.len());
    assert_eq!(s.dependencies.len(), back.dependencies.len());
    assert_eq!(back.relationships[0].kind, RelationshipKind::ForeignKey);
}

#[test]
fn object_index_contains_all_kinds() {
    let s = rich_snapshot();
    let idx = s.object_index();
    assert!(idx.contains_key(&oid("cat:main")));
    assert!(idx.contains_key(&oid("db:analytics")));
    assert!(idx.contains_key(&oid("sch:public")));
    assert!(idx.contains_key(&oid("tbl:orders")));
    assert!(idx.contains_key(&oid("view:order_summary")));
    assert!(idx.contains_key(&oid("mv:order_facts")));
    assert!(idx.contains_key(&oid("col:orders.id")));
}

#[test]
fn data_types_are_vendor_neutral_in_json() {
    let types = [
        DataType::Boolean,
        DataType::Timestamp {
            precision: Some(6),
            with_time_zone: Some(true),
        },
        DataType::Json,
        DataType::Uuid,
        DataType::Other {
            name: "GEOMETRY".into(),
        },
    ];
    for dt in types {
        let v = serde_json::to_value(&dt).unwrap();
        // No vendor brand names required in kind tags
        let kind = v["kind"].as_str().unwrap();
        assert!(!kind.to_lowercase().contains("snowflake"));
        assert!(!kind.to_lowercase().contains("postgres"));
        assert!(!kind.to_lowercase().contains("bigquery"));
    }
}

#[test]
fn model_version_semver_policy() {
    assert!(ModelVersion::new("1.0.0").is_compatible_with_current());
    assert!(ModelVersion::new("1.99.0").is_compatible_with_current());
    assert!(!ModelVersion::new("2.0.0").is_compatible_with_current());
}

#[test]
fn empty_snapshot_validates() {
    let s = Snapshot::new();
    validate_snapshot(&s).unwrap();
    assert_eq!(s.object_count(), 0);
}

#[test]
fn fqn_join_builds_hierarchy() {
    let root = FullyQualifiedName::parse_dotted("main.analytics").unwrap();
    let sch = root.join("public").unwrap();
    let tbl = sch.join("orders").unwrap();
    assert_eq!(tbl.to_dotted(), "main.analytics.public.orders");
}
