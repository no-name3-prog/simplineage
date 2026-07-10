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

    out.catalogs = union_by_id(base.catalogs.clone(), incoming.catalogs.clone(), |c| {
        c.meta.id.clone()
    });
    out.databases = union_by_id(base.databases.clone(), incoming.databases.clone(), |d| {
        d.meta.id.clone()
    });
    out.schemas = union_by_id(base.schemas.clone(), incoming.schemas.clone(), |s| {
        s.meta.id.clone()
    });
    out.tables = union_by_id(base.tables.clone(), incoming.tables.clone(), |t| {
        t.meta.id.clone()
    });
    out.views = union_by_id(base.views.clone(), incoming.views.clone(), |v| {
        v.meta.id.clone()
    });
    out.materialized_views = union_by_id(
        base.materialized_views.clone(),
        incoming.materialized_views.clone(),
        |m| m.meta.id.clone(),
    );
    out.columns = union_by_id(base.columns.clone(), incoming.columns.clone(), |c| {
        c.meta.id.clone()
    });
    out.relationships = union_by_id(
        base.relationships.clone(),
        incoming.relationships.clone(),
        |r| r.id.clone(),
    );
    out.dependencies = union_by_id(
        base.dependencies.clone(),
        incoming.dependencies.clone(),
        |d| d.id.clone(),
    );

    // Merge attributes (incoming wins on key conflict)
    for (k, v) in &incoming.attributes {
        out.attributes.insert(k.clone(), v.clone());
    }
    out
}

fn union_by_id<T, F>(mut base: Vec<T>, incoming: Vec<T>, id_of: F) -> Vec<T>
where
    F: Fn(&T) -> ObjectId,
{
    let mut seen: HashSet<String> = base.iter().map(|t| id_of(t).to_string()).collect();
    for item in incoming {
        let id = id_of(&item).to_string();
        if seen.insert(id) {
            base.push(item);
        } else {
            // Replace existing with incoming (last-write-wins by id)
            if let Some(pos) = base
                .iter()
                .position(|t| id_of(t).as_str() == id_of(&item).as_str())
            {
                base[pos] = item;
            }
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
