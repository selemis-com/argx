//! JSON Schema projection for Argx invocation values.
//!
//! Invocation schemas describe explicit values a caller can provide to one command context. They
//! intentionally model the public CLI rather than destination Rust values: named properties use
//! canonical option spellings, argv values are strings with finite vocabularies and recognized
//! semantic formats projected where available, and switches are represented by presence with the
//! value `true`. Environment
//! fallbacks and typed defaults remain outside the explicit invocation value object.

use serde_json::{Map, Value};

use crate::cli::command::{
    Command, ConstraintKind, Flag, Key, Named, long as resolve_long, short as resolve_short,
};

/// JSON Schema dialect used by Argx invocation schemas.
const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Projects one selected command path, including inherited globals visible in the selected scope.
pub(crate) fn invocation_schema_for_path(path: &[&Command<'_>]) -> schemars::Schema {
    let Some(&command) = path.last() else {
        return schemars::Schema::from(Map::new());
    };
    let mut properties = Map::new();
    let mut required = Vec::new();

    for (flag, name) in visible_flags(path) {
        let item = if flag.takes_value {
            lexical_value_schema(flag.accepted_values, flag.value_schema.format())
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
        let item = lexical_value_schema(arg.accepted_values, arg.value_schema.format());
        let mut schema = if arg.variadic { repeated_schema(item) } else { item };
        set_description(&mut schema, arg.help);
        properties.insert(arg.name.to_owned(), schema);
        if arg.required {
            required.push(Value::String(arg.name.to_owned()));
        }
    }

    let mut root = Map::new();
    root.insert("$schema".to_owned(), Value::String(DRAFT_2020_12.to_owned()));
    root.insert("title".to_owned(), Value::String(command.name.to_owned()));
    if let Some(description) = command.description.or(command.about).and_then(summary) {
        root.insert("description".to_owned(), Value::String(description.to_owned()));
    }
    root.insert("type".to_owned(), Value::String("object".to_owned()));
    if !properties.is_empty() {
        root.insert("properties".to_owned(), Value::Object(properties));
    }
    if !required.is_empty() {
        root.insert("required".to_owned(), Value::Array(required));
    }
    root.insert("additionalProperties".to_owned(), Value::Bool(false));
    add_constraints(&mut root, command);

    schemars::Schema::from(root)
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

/// Builds the schema for one argv value, optionally constrained to a finite vocabulary.
fn lexical_value_schema(accepted_values: &[&str], format: Option<&str>) -> Value {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String("string".to_owned()));
    if let Some(format) = format {
        schema.insert("format".to_owned(), Value::String(format.to_owned()));
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
    let (Value::Object(schema), Some(description)) = (schema, description.and_then(summary)) else {
        return;
    };
    schema.insert("description".to_owned(), Value::String(description.to_owned()));
}

/// Returns the first non-empty documentation paragraph for machine-readable discovery.
fn summary(documentation: &str) -> Option<&str> {
    documentation.split("\n\n").map(str::trim).find(|paragraph| !paragraph.is_empty())
}

/// Projects normalized Argx relationships into native JSON Schema object constraints.
fn add_constraints(root: &mut Map<String, Value>, command: &Command<'_>) {
    let mut dependent_required = Map::new();
    let mut conflicts = Vec::new();

    for constraint in command.constraints {
        let Some(source) = argument_name(command, constraint.source) else {
            continue;
        };
        let Some(target) = argument_name(command, constraint.target) else {
            continue;
        };

        match constraint.kind {
            ConstraintKind::Requires if requires_explicit_value(command, constraint.target) => {
                let targets = dependent_required
                    .entry(source.to_owned())
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(targets) = targets {
                    targets.push(Value::String(target.to_owned()));
                }
            }
            ConstraintKind::Requires => {}
            ConstraintKind::Conflicts => conflicts.push(conflict_schema(source, target)),
        }
    }

    if !dependent_required.is_empty() {
        root.insert("dependentRequired".to_owned(), Value::Object(dependent_required));
    }
    if !conflicts.is_empty() {
        root.insert("allOf".to_owned(), Value::Array(conflicts));
    }
}

/// Resolves one normalized semantic key to the corresponding invocation property name.
fn argument_name<'a>(command: &'a Command<'_>, key: Key) -> Option<&'a str> {
    command
        .flags
        .iter()
        .find(|flag| flag.key == key)
        .map(|flag| flag.diagnostic)
        .or_else(|| command.args.iter().find(|arg| arg.key == key).map(|arg| arg.name))
}

/// Returns whether satisfying one required target necessarily needs an explicit invocation value.
fn requires_explicit_value(command: &Command<'_>, key: Key) -> bool {
    command.flags.iter().find(|flag| flag.key == key).is_none_or(|flag| !flag.has_default)
}

/// Builds a schema fragment that rejects simultaneous presence of two invocation properties.
fn conflict_schema(source: &str, target: &str) -> Value {
    let mut required = Map::new();
    required.insert(
        "required".to_owned(),
        Value::Array(vec![Value::String(source.to_owned()), Value::String(target.to_owned())]),
    );

    let mut schema = Map::new();
    schema.insert("not".to_owned(), Value::Object(required));
    Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::command::{Arg, Constraint};

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
        let flags = [&verbose, &mode, &tag, &config, &output, &force, &dry_run];
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
        assert_eq!(schema["properties"]["--target"]["type"], "string");
        assert!(schema["properties"].get("destination").is_none());
        assert_eq!(schema["properties"]["input"]["type"], "string");
        assert_eq!(schema["properties"]["rest"]["type"], "array");
        assert_eq!(schema["required"], serde_json::json!(["input"]));
        assert_eq!(schema["dependentRequired"]["--target"], serde_json::json!(["--config"]));
        assert_eq!(
            schema["allOf"][0]["not"]["required"],
            serde_json::json!(["--force", "--dry-run"]),
        );
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
            value_schema: crate::cli::command::ValueSchema::DateTime,
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
            value_schema: crate::cli::command::ValueSchema::Date,
            ..Flag::VALUE
        };
        let local_time = Flag {
            key: 31,
            name: "local-time",
            diagnostic: "--local-time",
            longs: &["local-time"],
            value_schema: crate::cli::command::ValueSchema::Lexical,
            ..Flag::VALUE
        };
        let local_datetime = Flag {
            key: 32,
            name: "local-datetime",
            diagnostic: "--local-datetime",
            longs: &["local-datetime"],
            value_schema: crate::cli::command::ValueSchema::Lexical,
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
        let id = Arg {
            key: 30,
            name: "id",
            value_schema: crate::cli::command::ValueSchema::Uuid,
            ..Arg::REQUIRED
        };
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
            value_schema: crate::cli::command::ValueSchema::Url,
            ..Flag::VALUE
        };
        let flags = [&endpoint];
        let command = Command { name: "call", flags: &flags, ..Command::EMPTY };

        let schema = serde_json::to_value(invocation_schema_for_path(&[&command]))
            .expect("schema should serialize");

        assert_eq!(schema["properties"]["--endpoint"]["format"], "uri");
    }
}
