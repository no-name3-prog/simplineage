//! Rich terminal formatting helpers.

use std::io::{self, IsTerminal};

use colored::Colorize;
use serde::Serialize;

/// Output style controlled by global CLI flags.
#[derive(Debug, Clone, Copy)]
pub struct OutputStyle {
    /// Emit machine-readable JSON when possible.
    pub json: bool,
    /// Suppress non-essential decoration.
    pub quiet: bool,
    /// Force-disable color.
    pub no_color: bool,
}

impl OutputStyle {
    /// Whether color should be used on stdout.
    #[must_use]
    pub fn color_enabled(self) -> bool {
        if self.no_color || self.json {
            return false;
        }
        io::stdout().is_terminal()
    }

    /// Configure the `colored` crate based on style.
    pub fn apply_global(self) {
        if !self.color_enabled() {
            colored::control::set_override(false);
        }
    }
}

/// Print a section header.
pub fn header(style: OutputStyle, title: &str) {
    if style.quiet || style.json {
        return;
    }
    println!("{}", title.bold().cyan());
}

/// Print a success line.
pub fn success(style: OutputStyle, msg: &str) {
    if style.quiet || style.json {
        return;
    }
    println!("{} {}", "✔".green().bold(), msg);
}

/// Print an error-styled line to stderr (does not exit).
pub fn error_line(msg: &str) {
    eprintln!("{} {}", "✖".red().bold(), msg.red());
}

/// Key/value row.
pub fn kv(style: OutputStyle, key: &str, value: impl std::fmt::Display) {
    if style.json {
        return;
    }
    if style.quiet {
        println!("{value}");
        return;
    }
    println!("  {:<18} {}", format!("{key}:").dimmed(), value);
}

/// Bullet list item.
pub fn bullet(style: OutputStyle, msg: &str) {
    if style.json {
        return;
    }
    println!("  {} {msg}", "•".dimmed());
}

/// Dim helper text.
pub fn muted(style: OutputStyle, msg: &str) {
    if style.quiet || style.json {
        return;
    }
    println!("{}", msg.dimmed());
}

/// Serialize value as pretty JSON to stdout.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let out = serde_json::to_string_pretty(value)?;
    println!("{out}");
    Ok(())
}

/// Format a severity label with color.
pub fn severity_label(sev: &str) -> String {
    match sev.to_ascii_lowercase().as_str() {
        "error" => "error".red().bold().to_string(),
        "warning" => "warning".yellow().bold().to_string(),
        "info" => "info".blue().to_string(),
        other => other.to_string(),
    }
}

/// Table-like aligned columns (simple fixed padding).
pub fn print_table(style: OutputStyle, headers: &[&str], rows: &[Vec<String>]) {
    if style.json {
        return;
    }
    if rows.is_empty() {
        muted(style, "(no rows)");
        return;
    }
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:width$}", h, width = widths[i]))
        .collect();
    println!("{}", header_line.join("  ").bold());
    let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    println!("{}", sep.join("  ").dimmed());
    for row in rows {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let w = widths.get(i).copied().unwrap_or(c.len());
                format!("{c:w$}")
            })
            .collect();
        println!("{}", line.join("  "));
    }
}
