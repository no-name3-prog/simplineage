//! `simplineage search` — find metadata objects by id, FQN, or name.

use serde::Serialize;

use crate::context::{self, AppContext};
use crate::output::{self};

#[derive(Debug)]
pub struct SearchArgs {
    pub query: String,
    pub kind: Option<String>,
    pub limit: usize,
    pub snapshot: Option<String>,
}

#[derive(Debug, Serialize)]
struct SearchHit {
    id: String,
    kind: String,
    fqn: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

pub fn run_search(ctx: &AppContext, args: SearchArgs) -> anyhow::Result<()> {
    let style = ctx.style;
    let snapshot = ctx.load_snapshot(args.snapshot.as_deref())?;
    let hits = context::search_objects(&snapshot, &args.query, args.kind.as_deref(), args.limit);

    let rows: Vec<SearchHit> = hits
        .into_iter()
        .map(|h| SearchHit {
            id: h.id.to_string(),
            kind: h.kind,
            fqn: h.fqn,
            name: h.name,
            description: h.description,
        })
        .collect();

    if style.json {
        output::print_json(&serde_json::json!({
            "query": args.query,
            "count": rows.len(),
            "hits": rows,
        }))?;
        return Ok(());
    }

    output::header(
        style,
        &format!("Search: {:?} ({} hits)", args.query, rows.len()),
    );
    if rows.is_empty() {
        output::muted(style, "No matching objects.");
        return Ok(());
    }

    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|h| vec![h.kind.clone(), h.fqn.clone(), h.id.clone()])
        .collect();
    output::print_table(style, &["KIND", "FQN", "ID"], &table_rows);
    Ok(())
}
