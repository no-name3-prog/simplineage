//! Language bindings for SimpLineage.
//!
//! This crate is a **placeholder** for future FFI surfaces (Python via PyO3,
//! Node via N-API, etc.). The public Rust API lives in `simplineage-core` and
//! related crates; bindings will wrap those.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

/// Returns a short description of planned binding targets.
pub fn planned_targets() -> &'static [&'static str] {
    &["python", "node"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_python_target() {
        assert!(planned_targets().contains(&"python"));
    }
}
