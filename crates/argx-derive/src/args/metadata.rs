//! Shared normalized representation of application-defined command metadata.

/// One application-defined semantic metadata entry attached to a command.
pub(crate) struct MetadataEntry {
    /// Metadata key, preserved exactly as authored.
    pub key: String,
    /// Structured metadata value.
    pub value: MetadataValue,
}

/// JSON-like value supported by command metadata.
pub(crate) enum MetadataValue {
    /// Explicit null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Signed integer value.
    Integer(i64),
    /// Floating-point value.
    Float(f64),
    /// UTF-8 string value.
    String(String),
    /// Ordered collection of metadata values.
    Array(Vec<Self>),
    /// JSON object.
    Object(Vec<MetadataEntry>),
}
