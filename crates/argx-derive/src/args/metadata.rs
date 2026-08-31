//! Shared normalized representation of application-defined command metadata.

/// One application-defined semantic metadata entry attached to a command.
pub(crate) struct MetadataEntry {
    /// Metadata key, preserved exactly as authored.
    pub key: String,
    /// Structured JSON metadata value.
    pub value: serde_json::Value,
}
