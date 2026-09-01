//! JSON Schema projection for Argx invocation values.
//!
//! Invocation schemas describe explicit values a caller can provide to one command context. They
//! model a canonical semantic invocation: named properties use canonical option spellings, parsed
//! primitive values retain their JSON boolean/number types, finite vocabularies and recognized
//! semantic formats are projected where available, and switches are represented by presence with
//! the value `true`. Environment
//! fallbacks and typed defaults remain outside the explicit invocation value object.

use serde_json::{Map, Value};

use super::doc_summary;
use crate::cli::command::{
    Command, ConstraintKind, Flag, Key, MetadataValue, Named, ValueSchema, long as resolve_long,
    short as resolve_short,
};

/// JSON Schema dialect used by Argx invocation schemas.
const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Projects one selected command path, including inherited globals visible in the selected scope.
pub(crate) fn invocation_schema_for_path(path: &[&Command<'_>]) -> schemars::Schema {
    let Some(&command) = path.last() else {
        return schemars::Schema::from(Map::new());
    };
    invocation_schema(command, visible_flags(path), path)
}

/// Projects only values semantically owned by one command.
///
/// Recursive command schemas use this form so inherited globals remain represented once at the
/// selected document root instead of being duplicated at every descendant command object.
pub(crate) fn local_invocation_schema(command: &Command<'_>) -> schemars::Schema {
    let flags =
        command.flags.iter().copied().map(|flag| (flag, flag.diagnostic.to_owned())).collect();
    invocation_schema(command, flags, &[command])
}

/// Builds one invocation object from the supplied visible flag projection.
fn invocation_schema(
    command: &Command<'_>,
    flags: Vec<(&Flag<'_>, String)>,
    constraint_scopes: &[&Command<'_>],
) -> schemars::Schema {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut visible = Vec::new();

    for (flag, name) in flags {
        visible.push((flag.key, name.clone()));
        let item = if flag.takes_value {
            semantic_value_schema(flag.accepted_values, flag.value_schema)
        } else {
            true_switch_schema()
        };
        let mut schema = if flag.repeatable { repeated_schema(item) } else { item };
        set_description(&mut schema, flag.help);
        if flag.required {
            required.push(Value::String(name.clone()));
        }
        properties.insert(name, schema);
    }

    for arg in command.args {
        visible.push((arg.key, arg.name.to_owned()));
        let item = semantic_value_schema(arg.accepted_values, arg.value_schema);
        let mut schema = if arg.variadic { repeated_schema(item) } else { item };
        set_description(&mut schema, arg.help);
        properties.insert(arg.name.to_owned(), schema);
        if arg.required {
            required.push(Value::String(arg.name.to_owned()));
        }
    }

    let mut root = Map::new();
    root.insert("$schema".to_owned(), Value::String(DRAFT_2020_12.to_owned()));
    add_command_header(&mut root, command);
    if !properties.is_empty() {
        root.insert("properties".to_owned(), Value::Object(properties));
    }
    if !required.is_empty() {
        root.insert("required".to_owned(), Value::Array(required));
    }
    root.insert("additionalProperties".to_owned(), Value::Bool(false));
    add_constraints(&mut root, constraint_scopes, &visible);

    schemars::Schema::from(root)
}

/// Adds the common object identity and annotations for one command schema.
pub(super) fn add_command_header(root: &mut Map<String, Value>, command: &Command<'_>) {
    root.insert("title".to_owned(), Value::String(command.name.to_owned()));
    if let Some(description) = command.about.or(command.description).and_then(doc_summary) {
        root.insert("description".to_owned(), Value::String(description.to_owned()));
    }
    root.insert("type".to_owned(), Value::String("object".to_owned()));
    add_command_metadata(root, command);
}

/// Adds application-defined command metadata using a JSON Schema extension keyword.
fn add_command_metadata(root: &mut Map<String, Value>, command: &Command<'_>) {
    if command.metadata.is_empty() {
        return;
    }

    root.insert(
        "x-argx-metadata".to_owned(),
        Value::Object(
            command
                .metadata
                .iter()
                .map(|entry| (entry.key.to_owned(), metadata_value(entry.value)))
                .collect(),
        ),
    );
}

/// Projects one static metadata value into its JSON representation.
fn metadata_value(value: MetadataValue<'_>) -> Value {
    match value {
        MetadataValue::Null => Value::Null,
        MetadataValue::Bool(value) => Value::Bool(value),
        MetadataValue::Integer(value) => Value::Number(value.into()),
        MetadataValue::Float(value) => {
            serde_json::Number::from_f64(value).map(Value::Number).unwrap_or(Value::Null)
        }
        MetadataValue::String(value) => Value::String(value.to_owned()),
        MetadataValue::Array(values) => {
            Value::Array(values.iter().copied().map(metadata_value).collect())
        }
        MetadataValue::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|entry| (entry.key.to_owned(), metadata_value(entry.value)))
                .collect(),
        ),
    }
}

/// Collects canonical flag spellings accepted in the selected command scope.
fn visible_flags<'a>(path: &[&'a Command<'a>]) -> Vec<(&'a Flag<'a>, String)> {
    let Some((&command, ancestors)) = path.split_last() else {
        return Vec::new();
    };
    let mut flags = command
        .flags
        .iter()
        .copied()
        .map(|flag| (flag, flag.diagnostic.to_owned()))
        .collect::<Vec<_>>();

    for (scope, ancestor) in ancestors.iter().enumerate().rev() {
        for &flag in ancestor.flags.iter().filter(|flag| flag.global) {
            if let Some(spelling) = visible_ancestor_spelling(command, ancestors, scope, flag) {
                flags.push((flag, spelling));
            }
        }
    }

    flags
}

/// Returns one canonical spelling that remains visible for an inherited global flag.
fn visible_ancestor_spelling(
    command: &Command<'_>,
    ancestors: &[&Command<'_>],
    scope: usize,
    flag: &Flag<'_>,
) -> Option<String> {
    for &long in flag.longs {
        if matches!(
            resolve_long(command, ancestors, long.as_bytes()),
            Some(Named::Flag { flag: resolved, scope: resolved_scope })
                if resolved_scope == scope && std::ptr::eq(resolved, flag)
        ) {
            return Some(format!("--{long}"));
        }
    }
    for &short in flag.shorts {
        if matches!(
            resolve_short(command, ancestors, short),
            Some(Named::Flag { flag: resolved, scope: resolved_scope })
                if resolved_scope == scope && std::ptr::eq(resolved, flag)
        ) {
            return Some(format!("-{}", char::from(short)));
        }
    }
    None
}

/// Builds the schema for one parsed CLI value, optionally constrained to a finite vocabulary.
fn semantic_value_schema(accepted_values: &[&str], value_schema: ValueSchema) -> Value {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String(value_schema.json_type().to_owned()));
    if let Some(format) = value_schema.format() {
        schema.insert("format".to_owned(), Value::String(format.to_owned()));
    }
    if let Some((minimum, maximum)) = integer_bounds(value_schema) {
        if let Some(minimum) = minimum {
            schema.insert("minimum".to_owned(), Value::Number(minimum));
        }
        if let Some(maximum) = maximum {
            schema.insert("maximum".to_owned(), Value::Number(maximum));
        }
    }
    if !accepted_values.is_empty() {
        schema.insert(
            "enum".to_owned(),
            Value::Array(
                accepted_values.iter().map(|value| Value::String((*value).to_owned())).collect(),
            ),
        );
    }
    Value::Object(schema)
}

/// Returns exact Rust primitive integer bounds when `serde_json` can represent them losslessly.
fn integer_bounds(
    value_schema: ValueSchema,
) -> Option<(Option<serde_json::Number>, Option<serde_json::Number>)> {
    use serde_json::Number;

    let signed =
        |minimum: i128, maximum: i128| (Number::from_i128(minimum), Number::from_i128(maximum));
    let unsigned = |maximum: u128| (Some(Number::from(0)), Number::from_u128(maximum));

    Some(match value_schema {
        ValueSchema::I8 => signed(i8::MIN.into(), i8::MAX.into()),
        ValueSchema::I16 => signed(i16::MIN.into(), i16::MAX.into()),
        ValueSchema::I32 => signed(i32::MIN.into(), i32::MAX.into()),
        ValueSchema::I64 => signed(i64::MIN.into(), i64::MAX.into()),
        ValueSchema::I128 => signed(i128::MIN, i128::MAX),
        ValueSchema::Isize => signed(isize::MIN as i128, isize::MAX as i128),
        ValueSchema::U8 => unsigned(u8::MAX.into()),
        ValueSchema::U16 => unsigned(u16::MAX.into()),
        ValueSchema::U32 => unsigned(u32::MAX.into()),
        ValueSchema::U64 => unsigned(u64::MAX.into()),
        ValueSchema::U128 => unsigned(u128::MAX),
        ValueSchema::Usize => unsigned(usize::MAX as u128),
        ValueSchema::Lexical
        | ValueSchema::Boolean
        | ValueSchema::Number
        | ValueSchema::Date
        | ValueSchema::DateTime
        | ValueSchema::Uuid
        | ValueSchema::Url => return None,
    })
}

/// Builds the schema for a value-less switch, where presence means `true`.
fn true_switch_schema() -> Value {
    let mut schema = Map::new();
    schema.insert("const".to_owned(), Value::Bool(true));
    Value::Object(schema)
}

/// Wraps one per-occurrence schema in a non-empty repeated-value array.
fn repeated_schema(item: Value) -> Value {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String("array".to_owned()));
    schema.insert("items".to_owned(), item);
    schema.insert("minItems".to_owned(), Value::from(1));
    Value::Object(schema)
}

/// Attaches generated CLI help to one property schema when documentation is available.
fn set_description(schema: &mut Value, description: Option<&str>) {
    let (Value::Object(schema), Some(description)) = (schema, description.and_then(doc_summary))
    else {
        return;
    };
    schema.insert("description".to_owned(), Value::String(description.to_owned()));
}

/// Projects normalized Argx relationships whose participants are visible in this schema root.
///
/// Selected-path schemas hoist inherited globals into a new semantic root. Constraints declared by
/// ancestor commands therefore remain applicable when every participating property is visible in
/// that projection. Recursive child schemas pass only their owning command here, preserving the
/// one-scope ownership used by full command documents.
fn add_constraints(
    root: &mut Map<String, Value>,
    scopes: &[&Command<'_>],
    visible: &[(Key, String)],
) {
    let mut dependent_required = Map::new();
    let mut all_of = Vec::new();

    for command in scopes {
        for group in command.one_of {
            let members = group
                .members
                .iter()
                .map(|key| visible_name(visible, *key))
                .collect::<Option<Vec<_>>>();
            let Some(members) = members else {
                continue;
            };
            let constraint = relationship_schema("oneOf", &members);
            if !all_of.contains(&constraint) {
                all_of.push(constraint);
            }
        }
        for group in command.any_of {
            let members = group
                .members
                .iter()
                .map(|key| visible_name(visible, *key))
                .collect::<Option<Vec<_>>>();
            let Some(members) = members else {
                continue;
            };
            let constraint = relationship_schema("anyOf", &members);
            if !all_of.contains(&constraint) {
                all_of.push(constraint);
            }
        }
        for constraint in command.constraints {
            let Some(source) = visible_name(visible, constraint.source) else {
                continue;
            };
            let Some(target) = visible_name(visible, constraint.target) else {
                continue;
            };

            match constraint.kind {
                ConstraintKind::Requires if requires_explicit_value(scopes, constraint.target) => {
                    let targets = dependent_required
                        .entry(source.to_owned())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    if let Value::Array(targets) = targets
                        && !targets.iter().any(|value| value.as_str() == Some(target))
                    {
                        targets.push(Value::String(target.to_owned()));
                    }
                }
                ConstraintKind::Requires => {}
                ConstraintKind::Conflicts => {
                    let conflict = conflict_schema(source, target);
                    if !all_of.contains(&conflict) {
                        all_of.push(conflict);
                    }
                }
            }
        }
    }

    if !dependent_required.is_empty() {
        root.insert("dependentRequired".to_owned(), Value::Object(dependent_required));
    }
    if !all_of.is_empty() {
        root.insert("allOf".to_owned(), Value::Array(all_of));
    }
}

/// Builds a schema fragment requiring members according to one relationship keyword.
fn relationship_schema(keyword: &str, members: &[&str]) -> Value {
    let branches = members
        .iter()
        .map(|member| {
            let mut branch = Map::new();
            branch.insert(
                "required".to_owned(),
                Value::Array(vec![Value::String((*member).to_owned())]),
            );
            Value::Object(branch)
        })
        .collect();
    let mut schema = Map::new();
    schema.insert(keyword.to_owned(), Value::Array(branches));
    Value::Object(schema)
}

/// Resolves one normalized semantic key to its visible invocation property spelling.
fn visible_name(visible: &[(Key, String)], key: Key) -> Option<&str> {
    visible.iter().find(|(candidate, _)| *candidate == key).map(|(_, name)| name.as_str())
}

/// Returns whether satisfying one required target necessarily needs an explicit invocation value.
fn requires_explicit_value(scopes: &[&Command<'_>], key: Key) -> bool {
    scopes
        .iter()
        .flat_map(|command| command.flags.iter())
        .find(|flag| flag.key == key)
        .is_none_or(|flag| !flag.has_default)
}

/// Builds a schema fragment that rejects simultaneous presence of two invocation properties.
fn conflict_schema(source: &str, target: &str) -> Value {
    let (first, second) = if source <= target { (source, target) } else { (target, source) };
    let mut required = Map::new();
    required.insert(
        "required".to_owned(),
        Value::Array(vec![Value::String(first.to_owned()), Value::String(second.to_owned())]),
    );

    let mut schema = Map::new();
    schema.insert("not".to_owned(), Value::Object(required));
    Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::command::{AnyOf, Arg, Constraint, OneOf};

    #[test]
    fn projects_command_metadata_verbatim() {
        let nested = [crate::cli::command::MetadataEntry {
            key: "owner",
            value: MetadataValue::String("knowledge"),
        }];
        let metadata = [
            crate::cli::command::MetadataEntry {
                key: "readOnly",
                value: MetadataValue::Bool(true),
            },
            crate::cli::command::MetadataEntry {
                key: "required_scopes",
                value: MetadataValue::Array(&[MetadataValue::String("objects:read")]),
            },
            crate::cli::command::MetadataEntry {
                key: "policy",
                value: MetadataValue::Object(&nested),
            },
        ];
        let command = Command { name: "get", metadata: &metadata, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(schema["x-argx-metadata"]["readOnly"], true);
        assert_eq!(
            schema["x-argx-metadata"]["required_scopes"],
            serde_json::json!(["objects:read"])
        );
        assert_eq!(schema["x-argx-metadata"]["policy"]["owner"], "knowledge");
    }

    #[test]
    fn projects_one_of_as_exactly_one_property_constraint() {
        let user = Flag {
            key: 10,
            name: "user_id",
            diagnostic: "--user-id",
            longs: &["user-id"],
            ..Flag::VALUE
        };
        let group = Flag {
            key: 11,
            name: "group_id",
            diagnostic: "--group-id",
            longs: &["group-id"],
            ..Flag::VALUE
        };
        let flags = [&user, &group];
        let members = [10, 11];
        let one_of = [OneOf { members: &members }];
        let command = Command { name: "grant", flags: &flags, one_of: &one_of, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(
            schema["allOf"][0]["oneOf"],
            serde_json::json!([
                { "required": ["--user-id"] },
                { "required": ["--group-id"] },
            ]),
        );
    }

    #[test]
    fn projects_any_of_relationships() {
        let name =
            Flag { key: 20, name: "name", diagnostic: "--name", longs: &["name"], ..Flag::VALUE };
        let description = Flag {
            key: 21,
            name: "description",
            diagnostic: "--description",
            longs: &["description"],
            ..Flag::VALUE
        };
        let flags = [&name, &description];
        let members = [20, 21];
        let any_of = [AnyOf { members: &members }];
        let command = Command { name: "update", flags: &flags, any_of: &any_of, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(
            schema["allOf"][0]["anyOf"],
            serde_json::json!([
                { "required": ["--name"] },
                { "required": ["--description"] },
            ]),
        );
    }

    #[test]
    fn projects_normalized_argv_semantics_into_json_schema() {
        let verbose = Flag {
            key: 1,
            name: "verbose",
            diagnostic: "--verbose",
            help: Some("Enable verbose output."),
            longs: &["verbose"],
            ..Flag::BOOL
        };
        let mode = Flag {
            key: 2,
            name: "mode",
            diagnostic: "--mode",
            longs: &["mode"],
            accepted_values: &["fast", "safe"],
            ..Flag::VALUE
        };
        let tag = Flag {
            key: 3,
            name: "tag",
            diagnostic: "--tag",
            longs: &["tag"],
            repeatable: true,
            ..Flag::VALUE
        };
        let limit = Flag {
            key: 10,
            name: "limit",
            diagnostic: "--limit",
            longs: &["limit"],
            value_schema: ValueSchema::I64,
            ..Flag::VALUE
        };
        let small = Flag {
            key: 13,
            name: "small",
            diagnostic: "--small",
            longs: &["small"],
            value_schema: ValueSchema::I8,
            ..Flag::VALUE
        };
        let offset = Flag {
            key: 14,
            name: "offset",
            diagnostic: "--offset",
            longs: &["offset"],
            value_schema: ValueSchema::U16,
            ..Flag::VALUE
        };
        let ratio = Flag {
            key: 11,
            name: "ratio",
            diagnostic: "--ratio",
            longs: &["ratio"],
            value_schema: ValueSchema::Number,
            ..Flag::VALUE
        };
        let pinned = Flag {
            key: 12,
            name: "pinned",
            diagnostic: "--pinned",
            longs: &["pinned"],
            value_schema: ValueSchema::Boolean,
            ..Flag::VALUE
        };
        let config = Flag {
            key: 4,
            name: "config",
            diagnostic: "--config",
            longs: &["config"],
            ..Flag::VALUE
        };
        let output = Flag {
            key: 5,
            name: "destination",
            diagnostic: "--target",
            longs: &["target"],
            ..Flag::VALUE
        };
        let force =
            Flag { key: 6, name: "force", diagnostic: "--force", longs: &["force"], ..Flag::BOOL };
        let dry_run = Flag {
            key: 7,
            name: "dry_run",
            diagnostic: "--dry-run",
            longs: &["dry-run"],
            ..Flag::BOOL
        };
        let input = Arg { key: 8, name: "input", ..Arg::REQUIRED };
        let rest = Arg { key: 9, name: "rest", required: false, variadic: true, ..Arg::REQUIRED };
        let flags = [
            &verbose, &mode, &tag, &limit, &small, &offset, &ratio, &pinned, &config, &output,
            &force, &dry_run,
        ];
        let args = [&input, &rest];
        let constraints = [
            Constraint { kind: ConstraintKind::Requires, source: 5, target: 4 },
            Constraint { kind: ConstraintKind::Conflicts, source: 6, target: 7 },
        ];
        let command = Command {
            name: "run",
            about: Some("Run one operation."),
            flags: &flags,
            args: &args,
            constraints: &constraints,
            ..Command::EMPTY
        };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(schema["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(schema["title"], "run");
        assert_eq!(schema["description"], "Run one operation.");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"]["--verbose"]["const"], true);
        assert_eq!(schema["properties"]["--verbose"]["description"], "Enable verbose output.");
        assert_eq!(schema["properties"]["--mode"]["enum"], serde_json::json!(["fast", "safe"]));
        assert_eq!(schema["properties"]["--tag"]["type"], "array");
        assert_eq!(schema["properties"]["--tag"]["items"]["type"], "string");
        assert_eq!(schema["properties"]["--tag"]["minItems"], 1);
        assert_eq!(schema["properties"]["--limit"]["type"], "integer");
        assert_eq!(schema["properties"]["--limit"]["minimum"], i64::MIN);
        assert_eq!(schema["properties"]["--limit"]["maximum"], i64::MAX);
        assert_eq!(schema["properties"]["--small"]["minimum"], i8::MIN);
        assert_eq!(schema["properties"]["--small"]["maximum"], i8::MAX);
        assert_eq!(schema["properties"]["--offset"]["minimum"], 0);
        assert_eq!(schema["properties"]["--offset"]["maximum"], u16::MAX);
        assert_eq!(schema["properties"]["--ratio"]["type"], "number");
        assert_eq!(schema["properties"]["--pinned"]["type"], "boolean");
        assert_eq!(schema["properties"]["--target"]["type"], "string");
        assert!(schema["properties"].get("destination").is_none());
        assert_eq!(schema["properties"]["input"]["type"], "string");
        assert_eq!(schema["properties"]["rest"]["type"], "array");
        assert_eq!(schema["required"], serde_json::json!(["input"]));
        assert_eq!(schema["dependentRequired"]["--target"], serde_json::json!(["--config"]));
        assert_eq!(
            schema["allOf"][0]["not"]["required"],
            serde_json::json!(["--dry-run", "--force"]),
        );
    }

    #[test]
    fn duplicate_projected_constraints_are_deduplicated() {
        let source = Flag {
            key: 30,
            name: "source",
            diagnostic: "--source",
            longs: &["source"],
            global: true,
            ..Flag::BOOL
        };
        let target = Flag {
            key: 31,
            name: "target",
            diagnostic: "--target",
            longs: &["target"],
            global: true,
            ..Flag::VALUE
        };
        let flags = [&source, &target];
        let constraints = [
            Constraint { kind: ConstraintKind::Requires, source: 30, target: 31 },
            Constraint { kind: ConstraintKind::Conflicts, source: 30, target: 31 },
            Constraint { kind: ConstraintKind::Conflicts, source: 31, target: 30 },
        ];
        let root = Command { flags: &flags, constraints: &constraints, ..Command::EMPTY };
        let child = Command { flags: &flags, constraints: &constraints, ..Command::EMPTY };
        let scopes = [&root, &child];
        let visible = [(30, "--source".to_owned()), (31, "--target".to_owned())];
        let mut schema = Map::new();

        add_constraints(&mut schema, &scopes, &visible);

        assert_eq!(schema["dependentRequired"]["--source"], serde_json::json!(["--target"]));
        assert_eq!(schema["allOf"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn selected_scope_includes_visible_inherited_globals() {
        let root_verbose = Flag {
            key: 20,
            name: "verbose",
            diagnostic: "--verbose",
            longs: &["verbose"],
            shorts: b"v",
            global: true,
            ..Flag::BOOL
        };
        let profile = Flag {
            key: 21,
            name: "profile",
            diagnostic: "--profile",
            longs: &["profile"],
            global: true,
            ..Flag::VALUE
        };
        let region = Flag {
            key: 22,
            name: "region",
            diagnostic: "--region",
            longs: &["region"],
            global: true,
            ..Flag::VALUE
        };
        let local_verbose = Flag {
            key: 23,
            name: "verbose",
            diagnostic: "--verbose",
            longs: &["verbose"],
            ..Flag::BOOL
        };
        let root_flags = [&root_verbose, &profile];
        let mid_flags = [&region];
        let leaf_flags = [&local_verbose];
        let leaf = Command { name: "leaf", flags: &leaf_flags, ..Command::EMPTY };
        let mid = Command { name: "outer", flags: &mid_flags, ..Command::EMPTY };
        let root = Command { name: "tool", flags: &root_flags, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&root, &mid, &leaf]))
            .expect("schema should serialize");

        assert_eq!(schema["properties"]["--verbose"]["const"], true);
        assert_eq!(schema["properties"]["-v"]["const"], true);
        assert_eq!(schema["properties"]["--profile"]["type"], "string");
        assert_eq!(schema["properties"]["--region"]["type"], "string");
    }
    #[cfg(feature = "chrono")]
    #[test]
    fn chrono_datetime_values_expose_the_date_time_format() {
        let at = Flag {
            key: 29,
            name: "at",
            diagnostic: "--at",
            longs: &["at"],
            value_schema: ValueSchema::DateTime,
            ..Flag::VALUE
        };
        let flags = [&at];
        let command = Command { name: "show", flags: &flags, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(schema["properties"]["--at"]["format"], "date-time");
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn chrono_date_values_expose_only_standard_formats() {
        let date = Flag {
            key: 30,
            name: "date",
            diagnostic: "--date",
            longs: &["date"],
            value_schema: ValueSchema::Date,
            ..Flag::VALUE
        };
        let local_time = Flag {
            key: 31,
            name: "local-time",
            diagnostic: "--local-time",
            longs: &["local-time"],
            value_schema: ValueSchema::Lexical,
            ..Flag::VALUE
        };
        let local_datetime = Flag {
            key: 32,
            name: "local-datetime",
            diagnostic: "--local-datetime",
            longs: &["local-datetime"],
            value_schema: ValueSchema::Lexical,
            ..Flag::VALUE
        };
        let flags = [&date, &local_time, &local_datetime];
        let command = Command { name: "show", flags: &flags, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(schema["properties"]["--date"]["format"], "date");
        assert!(schema["properties"]["--local-time"].get("format").is_none());
        assert!(schema["properties"]["--local-datetime"].get("format").is_none());
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn uuid_values_expose_the_uuid_format() {
        let id = Arg { key: 30, name: "id", value_schema: ValueSchema::Uuid, ..Arg::REQUIRED };
        let args = [&id];
        let command = Command { name: "show", args: &args, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(schema["properties"]["id"]["format"], "uuid");
    }

    #[cfg(feature = "url")]
    #[test]
    fn url_values_expose_the_uri_format() {
        let endpoint = Flag {
            key: 31,
            name: "endpoint",
            diagnostic: "--endpoint",
            longs: &["endpoint"],
            value_schema: ValueSchema::Url,
            ..Flag::VALUE
        };
        let flags = [&endpoint];
        let command = Command { name: "call", flags: &flags, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(schema["properties"]["--endpoint"]["format"], "uri");
    }
}
