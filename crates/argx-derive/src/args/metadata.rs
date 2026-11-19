//! Shared normalized representation of application-defined command metadata.

/// One application-defined semantic metadata entry attached to a command.
pub(crate) struct MetadataEntry {
    /// Metadata key, preserved exactly as authored.
    pub key: String,
    /// Structured JSON metadata value.
    pub value: MetadataValue,
}

/// A metadata value accepted by Argx attributes and projected into static command metadata.
pub(crate) enum MetadataValue {
    /// A missing JSON value.
    Null,
    /// Boolean literal.
    Bool(bool),
    /// Signed integer literal.
    Integer(i64),
    /// Finite floating-point literal.
    Float(f64),
    /// String literal.
    String(String),
    /// Ordered list of values.
    Array(Vec<Self>),
    /// Object keys in canonical order, matching the attribute parser's previous JSON map.
    Object(std::collections::BTreeMap<String, Self>),
}
