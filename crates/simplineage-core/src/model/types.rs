//! Vendor-agnostic logical data types for columns.
//!
//! Physical / dialect-specific types (e.g. `NUMBER(38,0)`, `NVARCHAR2`) belong in
//! optional attributes or `raw_type`, never as required enum variants.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Logical data type used across catalogs — independent of any SQL dialect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataType {
    /// Boolean / bit.
    Boolean,
    /// Integer with optional bit width hint.
    Integer {
        /// Optional bit width (e.g. 16, 32, 64).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bits: Option<u16>,
    },
    /// Arbitrary-precision decimal / numeric.
    Decimal {
        /// Total precision, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        precision: Option<u16>,
        /// Scale, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<u16>,
    },
    /// Binary floating point.
    Float {
        /// Optional bit width (e.g. 32, 64).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bits: Option<u16>,
    },
    /// Character / string data.
    String {
        /// Max length if bounded.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_length: Option<u64>,
        /// Whether length is in characters (vs bytes), when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        is_char_length: Option<bool>,
    },
    /// Opaque binary.
    Binary {
        /// Max length if bounded.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_length: Option<u64>,
    },
    /// Calendar date (no time-of-day).
    Date,
    /// Time of day.
    Time {
        /// Fractional seconds precision, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        precision: Option<u16>,
        /// Whether timezone is present.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        with_time_zone: Option<bool>,
    },
    /// Timestamp / datetime.
    Timestamp {
        /// Fractional seconds precision, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        precision: Option<u16>,
        /// Whether timezone is present.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        with_time_zone: Option<bool>,
    },
    /// JSON / semi-structured document (vendor-neutral).
    Json,
    /// Structured array type.
    Array {
        /// Optional element type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        element: Option<Box<DataType>>,
    },
    /// Structured map / dictionary.
    Map {
        /// Optional key type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<Box<DataType>>,
        /// Optional value type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Box<DataType>>,
    },
    /// Structured record / struct / row.
    Struct {
        /// Optional named fields.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fields: Vec<StructField>,
    },
    /// Geography / geometry (kept abstract).
    Spatial,
    /// Universally unique identifier.
    Uuid,
    /// Type could not be mapped; preserve the source spelling.
    Other {
        /// Free-form type name from the source system.
        name: String,
    },
    /// Explicitly unknown.
    Unknown,
}

/// Field inside a [`DataType::Struct`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructField {
    /// Field name.
    pub name: String,
    /// Field type.
    pub data_type: DataType,
    /// Whether the field is nullable.
    #[serde(default = "default_true")]
    pub nullable: bool,
}

fn default_true() -> bool {
    true
}

/// Free-form key/value attributes for vendor-specific or experimental metadata.
///
/// Prefer structured fields on objects; use this map only when the core model
/// has no first-class place for a property.
pub type Attributes = BTreeMap<String, serde_json::Value>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_type_json_roundtrip() {
        let dt = DataType::Decimal {
            precision: Some(38),
            scale: Some(9),
        };
        let json = serde_json::to_string(&dt).unwrap();
        let back: DataType = serde_json::from_str(&json).unwrap();
        assert_eq!(dt, back);
        assert!(json.contains("decimal"));
    }
}
