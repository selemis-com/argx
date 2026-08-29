//! Built-in structured output selection for Argx command invocations.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::Error;

/// Output representation selected through `-O`/`--output`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    /// Application-defined human-readable output.
    #[default]
    Text,
    /// Structured JSON output.
    Json,
}

/// Built-in output options collected from `-O`/`--output` and `-F`/`--fields`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    /// Selected output format.
    format: OutputFormat,
    /// Whether the output format was explicitly set by the user.
    format_set: bool,
    /// Normalized field selectors in first-seen order.
    fields: Vec<String>,
    /// Schema for the selected handler result, when available.
    schema: Option<Value>,
}

impl Output {
    /// Returns the selected output format.
    #[must_use]
    pub const fn format(&self) -> OutputFormat {
        self.format
    }

    /// Returns normalized field selectors in first-seen order.
    #[must_use]
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Serializes and projects one successful handler value.
    ///
    /// With no selected fields this returns the ordinary serialized value. When `--fields` is
    /// present, only those schema-validated paths are retained.
    ///
    /// # Errors
    ///
    /// Returns an output error when serialization fails or the selected command has no typed
    /// result schema.
    pub fn value<T: Serialize>(&self, value: &T) -> Result<Value, OutputError> {
        let value = serde_json::to_value(value).map_err(OutputError::Serialize)?;
        if self.fields.is_empty() {
            return Ok(value);
        }
        if self.schema.is_none() {
            return Err(OutputError::UnavailableFields);
        }
        Ok(project(value, &self.fields))
    }

    /// Renders one successful handler value as compact JSON.
    ///
    /// # Errors
    ///
    /// Returns an output error when serialization or projection fails.
    pub fn render_json<T: Serialize>(&self, value: &T) -> Result<String, OutputError> {
        serde_json::to_string(&self.value(value)?).map_err(OutputError::Serialize)
    }

    /// Sets the output format from a raw command-line value.
    pub(crate) fn set_format(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.format_set {
            return Err(Error::DuplicateArgument { name: "--output" });
        }
        let value = std::str::from_utf8(bytes)
            .map_err(|_| Error::InvalidUtf8 { name: "--output", value: bytes.to_vec() })?;
        self.format = match value {
            "text" => OutputFormat::Text,
            "json" => OutputFormat::Json,
            _ => {
                return Err(Error::InvalidValue(Box::new(crate::InvalidValue {
                    name: "--output",
                    value: value.to_owned(),
                    reason: "expected one of: text, json".to_owned(),
                })));
            }
        };
        self.format_set = true;
        Ok(())
    }

    /// Adds comma-delimited field selectors from a raw command-line value.
    pub(crate) fn push_fields(&mut self, bytes: &[u8]) -> Result<(), Error> {
        for value in crate::cli::value::comma_values(vec![bytes.to_vec()]) {
            let field = std::str::from_utf8(&value)
                .map_err(|_| Error::InvalidUtf8 { name: "--fields", value: value.clone() })?;
            let field = field.trim();
            if field.is_empty() {
                return Err(Error::InvalidValue(Box::new(crate::InvalidValue {
                    name: "--fields",
                    value: String::from_utf8_lossy(bytes).into_owned(),
                    reason: "field selectors must not be empty".to_owned(),
                })));
            }
            if !self.fields.iter().any(|existing| existing == field) {
                self.fields.push(field.to_owned());
            }
        }
        Ok(())
    }

    /// Finalizes output selection against the selected handler schema.
    pub(crate) fn finish(&mut self, schema: Option<Value>) -> Result<(), Error> {
        if self.fields.is_empty() {
            return Ok(());
        }
        if self.format != OutputFormat::Json {
            return Err(Error::MissingRequirement {
                name: "--output json",
                required_by: "--fields",
            });
        }
        let Some(schema) = schema else {
            return Err(Error::OutputFieldsUnavailable);
        };
        for field in &self.fields {
            if !valid_path(&schema, field) {
                return Err(Error::InvalidOutputField { field: field.clone() });
            }
        }
        self.schema = Some(schema);
        Ok(())
    }
}

/// Failure while converting a typed handler result into structured output.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OutputError {
    /// Handler result serialization failed.
    #[error("failed to serialize handler output: {0}")]
    Serialize(serde_json::Error),
    /// Fields were requested for a command without a typed result schema.
    #[error("field selection is unavailable for this command")]
    UnavailableFields,
}

/// Parsed command value together with Argx's built-in output options.
#[derive(Debug)]
pub struct Invocation<T> {
    /// Parsed application command.
    pub command: T,
    /// Built-in structured-output options for this invocation.
    pub output: Output,
}

impl<T> Invocation<T> {
    /// Splits the parsed command from its output context.
    #[must_use]
    pub fn into_parts(self) -> (T, Output) {
        (self.command, self.output)
    }
}

/// Projects a JSON value to the selected field paths.
fn project(value: Value, fields: &[String]) -> Value {
    let mut tree = FieldTree::default();
    for field in fields {
        insert_path(&mut tree, field);
    }
    project_tree(value, &tree)
}

#[derive(Default)]
/// Tree representation of dotted field selectors.
struct FieldTree<'a>(BTreeMap<&'a str, Self>);

/// Inserts one dotted field path into a projection tree.
fn insert_path<'a>(tree: &mut FieldTree<'a>, path: &'a str) {
    let mut current = tree;
    for segment in path.split('.') {
        current = current.0.entry(segment).or_default();
    }
}

/// Recursively applies a field projection tree to a JSON value.
fn project_tree(value: Value, tree: &FieldTree<'_>) -> Value {
    match value {
        Value::Object(object) => {
            let mut projected = Map::new();
            for (field, children) in &tree.0 {
                if let Some(value) = object.get(*field) {
                    let value = if children.0.is_empty() {
                        value.clone()
                    } else {
                        project_tree(value.clone(), children)
                    };
                    projected.insert((*field).to_owned(), value);
                }
            }
            Value::Object(projected)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(|value| project_tree(value, tree)).collect())
        }
        value => value,
    }
}

/// Reports whether a dotted field path exists in the handler result schema.
fn valid_path(schema: &Value, path: &str) -> bool {
    let mut current = schema.get("$defs").and_then(|defs| defs.get("result"));
    for segment in path.split('.') {
        current = current.and_then(|current| property_schema(current, segment, schema));
        if current.is_none() {
            return false;
        }
    }
    current.is_some()
}

/// Resolves one object property through direct, union, or referenced schema nodes.
fn property_schema<'a>(schema: &'a Value, name: &str, root: &'a Value) -> Option<&'a Value> {
    let schema = dereference(schema, root)?;
    if let Some(items) = schema.get("items") {
        return property_schema(items, name, root);
    }
    if let Some(property) = schema.get("properties").and_then(|properties| properties.get(name)) {
        return Some(property);
    }
    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = schema.get(keyword).and_then(Value::as_array)
            && let Some(property) =
                branches.iter().find_map(|branch| property_schema(branch, name, root))
        {
            return Some(property);
        }
    }
    None
}

/// Resolves a local JSON Schema reference against the root schema document.
fn dereference<'a>(schema: &'a Value, root: &'a Value) -> Option<&'a Value> {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return Some(schema);
    };
    let pointer = reference.strip_prefix('#')?;
    root.pointer(pointer)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn fields_split_trim_deduplicate_and_project_nested_arrays() {
        let mut output = Output::default();
        output.push_fields(b"id, items.name").unwrap();
        output.push_fields(b"items.name,items.id").unwrap();
        assert_eq!(output.fields(), ["id", "items.name", "items.id"]);

        let value = json!({
            "id": 7,
            "ignored": true,
            "items": [
                {"id": 1, "name": "one", "ignored": 1},
                {"id": 2, "name": "two", "ignored": 2}
            ]
        });
        assert_eq!(
            project(value, output.fields()),
            json!({"id": 7, "items": [{"id": 1, "name": "one"}, {"id": 2, "name": "two"}]})
        );
    }

    #[test]
    fn empty_field_is_rejected() {
        let mut output = Output::default();
        assert!(output.push_fields(b"id,").is_err());
    }
}
