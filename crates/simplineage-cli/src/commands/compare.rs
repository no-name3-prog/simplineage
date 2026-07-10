//! `simplineage compare` — diff two snapshots.

use crate::context::{self, AppContext};
use crate::output::{self};

#[derive(Debug)]
pub struct CompareArgs {
    pub left: String,
    pub right: String,
    pub limit: usize,
}

pub fn run_compare(ctx: &AppContext, args: CompareArgs) -> anyhow::Result<()> {
    let style = ctx.style;
    let left = ctx.load_snapshot(Some(&args.left))?;
    let right = ctx.load_snapshot(Some(&args.right))?;
    let diff = context::compare_snapshots(&left, &right);

    if style.json {
        output::print_json(&diff)?;
        return Ok(());
    }

    output::header(style, "Snapshot compare");
    output::kv(style, "left", &diff.left_id);
    output::kv(style, "right", &diff.right_id);
    output::kv(
        style,
        "objects",
        format!(
            "left={} right={} shared={} only_left={} only_right={}",
            diff.left_object_count,
            diff.right_object_count,
            diff.objects_shared,
            diff.objects_only_left.len(),
            diff.objects_only_right.len()
        ),
    );
    output::kv(
        style,
        "edges",
        format!(
            "left={} right={} shared={} only_left={} only_right={}",
            diff.left_edge_count,
            diff.right_edge_count,
            diff.edges_shared,
            diff.edges_only_left.len(),
            diff.edges_only_right.len()
        ),
    );

    if !diff.objects_only_left.is_empty() {
        output::header(
            style,
            &format!("Objects only in left (showing up to {})", args.limit),
        );
        for id in diff.objects_only_left.iter().take(args.limit) {
            output::bullet(style, id);
        }
    }
    if !diff.objects_only_right.is_empty() {
        output::header(
            style,
            &format!("Objects only in right (showing up to {})", args.limit),
        );
        for id in diff.objects_only_right.iter().take(args.limit) {
            output::bullet(style, id);
        }
    }
    if !diff.edges_only_left.is_empty() {
        output::header(
            style,
            &format!("Edges only in left (showing up to {})", args.limit),
        );
        for id in diff.edges_only_left.iter().take(args.limit) {
            output::bullet(style, id);
        }
    }
    if !diff.edges_only_right.is_empty() {
        output::header(
            style,
            &format!("Edges only in right (showing up to {})", args.limit),
        );
        for id in diff.edges_only_right.iter().take(args.limit) {
            output::bullet(style, id);
        }
    }

    Ok(())
}
