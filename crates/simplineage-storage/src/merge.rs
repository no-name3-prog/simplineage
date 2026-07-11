//! Snapshot merge helpers for incremental imports.

use std::collections::HashSet;

use simplineage_core::Snapshot;
use simplineage_core::model::ids::ObjectId;
use uuid::Uuid;

/// How an incoming snapshot is combined with an existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// Store as a brand-new snapshot; do not merge content.
    Replace,
    /// Union objects and edges with a base snapshot (by object/edge id).
    Merge,
}

impl ImportMode {
    /// Wire format name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Merge => "merge",
        }
    }
}

/// Merge `incoming` into `base` (id-stable union). Produces a new snapshot id.
///
/// Clones `base` once, then unions each collection by taking ownership of the
/// base vectors (no second full-vector clone) and cloning only incoming items.
#[must_use]
pub fn merge_snapshots(base: &Snapshot, incoming: &Snapshot) -> Snapshot {
    let mut out = base.clone();
    out.id = ObjectId::from_trusted(Uuid::new_v4().to_string());
    out.label = incoming
        .label
        .clone()
        .or_else(|| base.label.clone())
        .map(|l| format!("{l}+merge"));
    out.source = incoming.source.clone().or_else(|| base.source.clone());
    out.created_at = incoming
        .created_at
        .clone()
        .or_else(|| base.created_at.clone());
    // Prefer newer model version string if same major (caller validates)
    out.model_version = incoming.model_version.clone();

    out.catalogs = union_by_id(std::mem::take(&mut out.catalogs), &incoming.catalogs, |c| {
        &c.meta.id
    });
    out.databases = union_by_id(
        std::mem::take(&mut out.databases),
        &incoming.databases,
        |d| &d.meta.id,
    );
    out.schemas = union_by_id(std::mem::take(&mut out.schemas), &incoming.schemas, |s| {
        &s.meta.id
    });
    out.tables = union_by_id(std::mem::take(&mut out.tables), &incoming.tables, |t| {
        &t.meta.id
    });
    out.views = union_by_id(std::mem::take(&mut out.views), &incoming.views, |v| {
        &v.meta.id
    });
    out.materialized_views = union_by_id(
        std::mem::take(&mut out.materialized_views),
        &incoming.materialized_views,
        |m| &m.meta.id,
    );
    out.columns = union_by_id(std::mem::take(&mut out.columns), &incoming.columns, |c| {
        &c.meta.id
    });
    out.relationships = union_by_id(
        std::mem::take(&mut out.relationships),
        &incoming.relationships,
        |r| &r.id,
    );
    out.dependencies = union_by_id(
        std::mem::take(&mut out.dependencies),
        &incoming.dependencies,
        |d| &d.id,
    );

    // Merge attributes (incoming wins on key conflict)
    for (k, v) in &incoming.attributes {
        out.attributes.insert(k.clone(), v.clone());
    }
    out
}

fn union_by_id<T, F>(mut base: Vec<T>, incoming: &[T], id_of: F) -> Vec<T>
where
    T: Clone,
    F: Fn(&T) -> &ObjectId,
{
    let mut seen: HashSet<String> = base.iter().map(|t| id_of(t).as_str().to_string()).collect();
    for item in incoming {
        let id = id_of(item).as_str();
        if seen.insert(id.to_string()) {
            base.push(item.clone());
        } else if let Some(pos) = base.iter().position(|t| id_of(t).as_str() == id) {
            // Replace existing with incoming (last-write-wins by id)
            base[pos] = item.clone();
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
    use simplineage_core::model::ids::FullyQualifiedName;
    use simplineage_core::model::objects::{ObjectMeta, Table};

    fn oid(s: &str) -> ObjectId {
        ObjectId::from_trusted(s)
    }

    #[test]
    fn merge_unions_tables_and_deps() {
        let mut a = Snapshot::new();
        a.tables.push(Table {
            meta: ObjectMeta::new(
                oid("t1"),
                FullyQualifiedName::parse_dotted("public.a").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
        });
        a.dependencies.push(Dependency {
            id: oid("d1"),
            from_id: oid("t1"),
            to_id: oid("t2"),
            kind: DependencyKind::Manual,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });

        let mut b = Snapshot::new();
        b.tables.push(Table {
            meta: ObjectMeta::new(
                oid("t2"),
                FullyQualifiedName::parse_dotted("public.b").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
        });
        b.dependencies.push(Dependency {
            id: oid("d2"),
            from_id: oid("t2"),
            to_id: oid("t3"),
            kind: DependencyKind::Manual,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });

        let m = merge_snapshots(&a, &b);
        assert_eq!(m.tables.len(), 2);
        assert_eq!(m.dependencies.len(), 2);
        assert_ne!(m.id, a.id);
    }
}
