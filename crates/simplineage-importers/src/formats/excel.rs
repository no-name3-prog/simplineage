//! Excel (.xlsx / .xls) importer via calamine.

use std::path::Path;

use calamine::{Data, Reader, open_workbook_auto};
use simplineage_core::{Error, Result, Snapshot};

use crate::detect::{has_extension, looks_like_excel};
use crate::intermediate::IntermediateCatalog;
use crate::normalize::intermediate_to_snapshot;
use crate::options::ImportOptions;
use crate::plugin::{DetectConfidence, MetadataImporter};
use crate::tabular::{self, FieldGet};

/// Imports metadata from Excel workbooks.
///
/// Expected sheet names (case-insensitive): `tables`, `columns`,
/// `relationships`, `dependencies`. Extra sheets are ignored.
#[derive(Debug, Default)]
pub struct ExcelImporter;

impl MetadataImporter for ExcelImporter {
    fn id(&self) -> &'static str {
        "excel"
    }

    fn name(&self) -> &str {
        "Excel metadata"
    }

    fn description(&self) -> &str {
        "XLSX/XLS workbook with tables/columns sheets"
    }

    fn extensions(&self) -> &[&str] {
        &["xlsx", "xlsm", "xls"]
    }

    fn detect(&self, path: &Path) -> DetectConfidence {
        if !path.is_file() {
            return DetectConfidence::None;
        }
        let ext = has_extension(path, &["xlsx", "xlsm", "xls"]);
        let magic = looks_like_excel(path);
        match (ext, magic) {
            (true, true) => DetectConfidence::High,
            (true, false) => DetectConfidence::Medium,
            (false, true) => DetectConfidence::Low,
            _ => DetectConfidence::None,
        }
    }

    fn import(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot> {
        let cat = parse_excel(path)?;
        let mut opts = options.clone();
        if opts.source.is_none() {
            opts.source = Some(format!("excel:{}", path.display()));
        }
        intermediate_to_snapshot(&cat, &opts)
    }
}

fn parse_excel(path: &Path) -> Result<IntermediateCatalog> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|e| Error::import(format!("open excel {}: {e}", path.display())))?;

    let mut cat = IntermediateCatalog {
        source: Some(path.display().to_string()),
        ..Default::default()
    };

    let sheet_names = workbook.sheet_names().to_vec();
    for name in sheet_names {
        let lower = name.to_ascii_lowercase();
        let range = workbook
            .worksheet_range(&name)
            .map_err(|e| Error::import(format!("sheet {name}: {e}")))?;

        let mut rows = range.rows();
        let Some(header_cells) = rows.next() else {
            continue;
        };
        let headers: Vec<String> = header_cells
            .iter()
            .map(cell_string)
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if headers.iter().all(|h| h.is_empty()) {
            continue;
        }

        for row in rows {
            let accessor = ExcelRow {
                headers: &headers,
                row,
            };
            if lower.contains("column") {
                let _ = tabular::push_column(&mut cat, &accessor);
            } else if lower.contains("relationship") {
                let _ = tabular::push_relationship(&mut cat, &accessor);
            } else if lower.contains("depend") || lower.contains("lineage") {
                let _ = tabular::push_dependency(&mut cat, &accessor);
            } else if lower.contains("table") || lower == "relations" {
                tabular::push_table(&mut cat, &accessor, None);
            }
        }
    }

    if cat.tables.is_empty() && cat.columns.is_empty() {
        return Err(Error::import(format!(
            "no table/column rows found in excel {}",
            path.display()
        )));
    }
    Ok(cat)
}

fn cell_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => format!("{dt:?}"),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERR{e:?}"),
    }
}

struct ExcelRow<'a> {
    headers: &'a [String],
    row: &'a [Data],
}

impl FieldGet for ExcelRow<'_> {
    fn get(&self, names: &[&str]) -> Option<String> {
        let i = self
            .headers
            .iter()
            .position(|h| names.iter().any(|n| h == n))?;
        self.row
            .get(i)
            .map(cell_string)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}
