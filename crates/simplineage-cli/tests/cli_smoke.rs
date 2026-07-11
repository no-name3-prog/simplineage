//! End-to-end smoke tests for the Phase 6 CLI.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn bin() -> Command {
    Command::cargo_bin("simplineage").unwrap()
}

fn fixture() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lineage.json")
}

#[test]
fn version_and_help() {
    bin()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));

    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("import"))
        .stdout(predicate::str::contains("upstream"))
        .stdout(predicate::str::contains("impact"));
}

#[test]
fn import_build_search_lineage_export_flow() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let fix = fixture();
    assert!(fix.exists(), "missing fixture {}", fix.display());

    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--no-progress",
            "import",
            fix.to_str().unwrap(),
            "--importer",
            "json",
            "--label",
            "smoke",
        ])
        .assert()
        .success();

    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--no-progress",
            "build",
            "--full",
        ])
        .assert()
        .success();

    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "search",
            "orders",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("orders"));

    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "--no-progress",
            "downstream",
            "table:orders",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("order_facts"));

    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "--no-progress",
            "impact",
            "table:orders",
            "--direction",
            "both",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("total_affected"));

    bin()
        .args(["--data-dir", data.to_str().unwrap(), "--json", "stats"])
        .assert()
        .success()
        .stdout(predicate::str::contains("snapshots"));

    let out = dir.path().join("out.json");
    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--no-progress",
            "export",
            "-f",
            "json-pretty",
            "-o",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(out.exists());

    // validate may fail quality gates; ensure it exits with a code
    let assert = bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--no-progress",
            "validate",
        ])
        .assert();
    let code = assert.get_output().status.code();
    assert!(code == Some(0) || code == Some(1));
}

#[test]
fn compare_two_files() {
    let fix = fixture();
    bin()
        .args([
            "--json",
            "compare",
            fix.to_str().unwrap(),
            fix.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("objects_shared"));
}

#[test]
fn list_importers() {
    bin()
        .args(["import", "--list-importers"])
        .assert()
        .success()
        .stdout(predicate::str::contains("csv"))
        .stdout(predicate::str::contains("json"));
}

#[test]
fn column_level_impact_and_html() {
    let dir = tempdir().unwrap();
    let data = dir.path().join("data");
    let fix = fixture();

    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--no-progress",
            "import",
            fix.to_str().unwrap(),
            "--importer",
            "json",
            "--label",
            "col-smoke",
        ])
        .assert()
        .success();

    // Column FQN resolution + column-level filter
    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "--no-progress",
            "downstream",
            "public.orders.email",
            "--level",
            "column",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("column:order_facts.email"))
        .stdout(predicate::str::contains("\"level\": \"column\""));

    // --column under a parent table
    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "--no-progress",
            "impact",
            "table:orders",
            "--column",
            "id",
            "--level",
            "column",
            "--direction",
            "downstream",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("column:order_facts.order_id"))
        .stdout(predicate::str::contains("\"subject_kind\": \"column\""));

    // Existing relation-only path still works
    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--json",
            "--no-progress",
            "downstream",
            "table:orders",
            "--relations-only",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("table:order_facts"));

    let html = dir.path().join("lineage.html");
    bin()
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--no-progress",
            "export",
            "-f",
            "html",
            "-o",
            html.to_str().unwrap(),
        ])
        .assert()
        .success();
    let body = std::fs::read_to_string(&html).unwrap();
    assert!(
        body.contains("parent_id"),
        "HTML payload should include parent_id for columns"
    );
    assert!(
        body.contains("d-columns"),
        "HTML should include columns panel"
    );
    assert!(body.contains("column:orders.email"));
}
