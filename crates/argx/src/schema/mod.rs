//! Machine-readable JSON Schema for derived command trees and typed handler contracts.
//!
//! Schema-enabled parsers expose their invocation shape through the same static command model used
//! for parsing. Executable leaves can additionally contribute typed result and error schemas
//! through `#[argx(handler = ...)]`. Handler associations are collected into a short-lived local
//! registry by generated trait calls over the concrete command types; Argx uses neither global
//! registration nor linker inventory.

use std::{borrow::Cow, ffi::OsStr, fmt::Write as _};

use schemars::{SchemaGenerator, generate::SchemaSettings};
use serde_json::{Map, Value};

mod invocation;

use crate::{
    Error,
    cli::{
        command::{Command, Key},
        help,
        protocol::{HandlerSchemaSource, HandlerSchemas},
    },
};

/// One executable command association collected from generated schema topology.
#[derive(Clone, Copy, Debug)]
struct Entry {
    /// Stable semantic identity of the command table.
    key: Key,
    /// Result/error schema generator supplied by the handler association.
    schemas: fn(&mut SchemaGenerator) -> HandlerSchemas,
}

/// Ephemeral handler associations for one schema-enabled CLI.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct Registry {
    /// Executable command handlers in static command-tree order.
    entries: Vec<Entry>,
}

impl Registry {
    /// Creates an empty schema registry.
    #[must_use]
    #[doc(hidden)]
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Registers one executable command handler.
    #[doc(hidden)]
    pub fn register(
        &mut self,
        command: &'static Command<'static>,
        schemas: fn(&mut SchemaGenerator) -> HandlerSchemas,
    ) {
        self.entries.push(Entry { key: command.key, schemas });
    }

    /// Resolves one selected command's handler schemas into a shared generator.
    fn handler(
        &self,
        command: &Command<'_>,
        generator: &mut SchemaGenerator,
    ) -> Option<HandlerSchemas> {
        self.entries
            .iter()
            .find(|entry| entry.key == command.key)
            .map(|entry| (entry.schemas)(generator))
    }
}

/// Schema topology contributed by one executable leaf or structural command group.
#[doc(hidden)]
pub trait SchemaCommand {
    /// Registers executable commands represented beneath `command`.
    fn register_schema_commands(command: &'static Command<'static>, registry: &mut Registry);
}

/// Schema topology contributed by one derived `Subcommand` enum.
#[doc(hidden)]
pub trait SchemaSubcommands {
    /// Registers executable commands represented by `commands`.
    fn register_schema_subcommands(
        commands: &'static [&'static Command<'static>],
        registry: &mut Registry,
    );
}

/// Registers one handler-backed executable leaf.
#[doc(hidden)]
pub fn register_handler<T>(command: &'static Command<'static>, registry: &mut Registry)
where
    T: HandlerSchemaSource,
{
    registry.register(command, T::handler_schemas);
}

/// Handles the built-in `schema [COMMAND]...` pseudo-command before ordinary argv binding.
pub(crate) fn pseudo_command(
    root: &'static Command<'static>,
    argv: &[&OsStr],
    registry: &Registry,
) -> Option<Error> {
    if root.subcommands.is_empty() {
        return None;
    }

    let (first, segments) = argv.split_first()?;
    if first.as_encoded_bytes() != b"schema" {
        return None;
    }

    if segments
        .iter()
        .any(|segment| *segment == OsStr::new("-h") || *segment == OsStr::new("--help"))
    {
        return Some(Error::DisplayHelp { help: help::render_schema(root) });
    }

    let (segments, full) = match segments.split_last() {
        Some((last, command_path)) if *last == OsStr::new("--full") => (command_path, true),
        _ => (segments, false),
    };

    let mut command = root;
    let mut path = vec![root];
    for segment in segments {
        let bytes = segment.as_encoded_bytes();
        let Some(child) = command.subcommands.iter().copied().find(|child| {
            child.name.as_bytes() == bytes
                || child.aliases.iter().any(|alias| alias.as_bytes() == bytes)
        }) else {
            return Some(Error::UnknownCommand { token: bytes.to_vec() });
        };
        command = child;
        path.push(child);
    }

    Some(display_schema(&path, registry, full))
}

/// Renders the schema for a selected command path as a display error.
pub(crate) fn display_schema(path: &[&Command<'_>], registry: &Registry, full: bool) -> Error {
    let command = path.last().copied().expect("schema discovery always has a root command");
    let effective_full = full || command.subcommands.is_empty();
    let schema = if effective_full {
        full_command_schema(path, registry, &[])
    } else {
        concise_command_schema(path)
    };
    let mut rendered =
        serde_json::to_string_pretty(&schema).expect("schema document must serialize");
    rendered.push('\n');
    Error::DisplaySchema { schema: rendered }
}

/// Builds the compact schema document shown by default.
///
/// The selected command is fully described. Immediate child commands are real referenced object
/// properties, but their definitions remain deliberately open projection boundaries. This keeps
/// ordinary schema discovery self-contained and standards-validating without pretending to know
/// descendant invocation details that were not projected. `--full` recursively closes those
/// boundaries.
fn concise_command_schema(path: &[&Command<'_>]) -> Value {
    let command = path.last().copied().expect("schema discovery always has a root command");
    let mut schema = serde_json::to_value(invocation::invocation_schema_for_path(path))
        .expect("invocation schema must serialize");
    let object = schema.as_object_mut().expect("invocation schemas are objects");

    if !command.subcommands.is_empty() {
        let mut commands = Map::new();
        for &child in command.subcommands {
            let mut stub = Map::new();
            stub.insert("title".to_owned(), Value::String(child.name.to_owned()));
            if let Some(description) = child.about.or(child.description).and_then(doc_summary) {
                stub.insert("description".to_owned(), Value::String(description.to_owned()));
            }
            stub.insert("type".to_owned(), Value::String("object".to_owned()));
            invocation::add_command_metadata(&mut stub, child);
            commands.insert(child.name.to_owned(), Value::Object(stub));
        }

        add_subcommand_properties(object, command, &[]);
        object.insert(
            "$defs".to_owned(),
            Value::Object(Map::from_iter([("commands".to_owned(), definitions_schema(commands))])),
        );
    }

    schema
}

/// Returns the first non-empty paragraph from command documentation.
pub(super) fn doc_summary(documentation: &str) -> Option<&str> {
    documentation.split("\n\n").map(str::trim).find(|paragraph| !paragraph.is_empty())
}

/// Builds a complete validating command document.
///
/// The selected command root includes globals inherited from its command path. Descendants contain
/// only fields owned by that command, so one semantic option is represented at exactly one object
/// level. Child command selection is expressed through ordinary `properties`, local `$ref`s, and
/// `required`/`oneOf`; `$defs` is used only to bundle referenced command and handler schemas.
fn full_command_schema(path: &[&Command<'_>], registry: &Registry, location: &[String]) -> Value {
    let command = path.last().copied().expect("schema discovery always has a root command");
    let schema = serde_json::to_value(invocation::invocation_schema_for_path(path))
        .expect("invocation schema must serialize");
    complete_command_schema(schema, command, registry, location)
}

/// Recursively completes one command schema whose invocation fields are already projected.
fn complete_command_schema(
    mut schema: Value,
    command: &Command<'_>,
    registry: &Registry,
    location: &[String],
) -> Value {
    let object = schema.as_object_mut().expect("invocation schemas are objects");
    if !location.is_empty() {
        object.remove("$schema");
    }

    let mut definitions = Map::new();

    let mut generator = schema_generator(location);
    if let Some(handler) = registry.handler(command, &mut generator) {
        definitions.insert("result".to_owned(), schema_value(handler.result));
        definitions.insert("error".to_owned(), schema_value(handler.error));

        let types = generator.take_definitions(false);
        if !types.is_empty() {
            definitions.insert("types".to_owned(), definitions_schema(types));
        }
    }

    if !command.subcommands.is_empty() {
        let mut commands = Map::new();
        for &child in command.subcommands {
            let child_location = child_location(location, child.name);
            let child_schema = serde_json::to_value(invocation::local_invocation_schema(child))
                .expect("invocation schema must serialize");
            commands.insert(
                child.name.to_owned(),
                complete_command_schema(child_schema, child, registry, &child_location),
            );
        }

        add_subcommand_properties(object, command, location);
        definitions.insert("commands".to_owned(), definitions_schema(commands));
    }

    if !definitions.is_empty() {
        object.insert("$defs".to_owned(), Value::Object(definitions));
    }

    schema
}

/// Adds canonical subcommand properties and exact-one selection constraints to one command object.
fn add_subcommand_properties(
    object: &mut Map<String, Value>,
    command: &Command<'_>,
    location: &[String],
) {
    let properties = object
        .entry("properties".to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("invocation properties must be an object");

    for &child in command.subcommands {
        assert!(
            !properties.contains_key(child.name),
            "schema subcommand name must not collide with an invocation property",
        );
        let child_location = child_location(location, child.name);
        properties.insert(child.name.to_owned(), reference_schema(&child_location));
    }

    if command.subcommands.len() == 1 {
        append_required(object, command.subcommands[0].name);
    } else if !command.subcommands.is_empty() {
        object.insert(
            "oneOf".to_owned(),
            Value::Array(
                command.subcommands.iter().map(|child| required_schema(child.name)).collect(),
            ),
        );
    }
}

/// Returns the bundled location of one child command schema.
fn child_location(location: &[String], child: &str) -> Vec<String> {
    let mut child_location = location.to_vec();
    child_location.extend([
        "$defs".to_owned(),
        "commands".to_owned(),
        "$defs".to_owned(),
        child.to_owned(),
    ]);
    child_location
}

/// Builds a local `$ref` to one bundled command schema.
fn reference_schema(location: &[String]) -> Value {
    let pointer =
        location.iter().map(|segment| pointer_token(segment)).collect::<Vec<_>>().join("/");
    let mut schema = Map::new();
    schema.insert("$ref".to_owned(), Value::String(format!("#/{pointer}")));
    Value::Object(schema)
}

/// Appends one required property while preserving existing invocation requirements.
fn append_required(object: &mut Map<String, Value>, property: &str) {
    let required = object
        .entry("required".to_owned())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("invocation required must be an array");
    required.push(Value::String(property.to_owned()));
}

/// Builds one branch requiring a selected subcommand property.
fn required_schema(property: &str) -> Value {
    let mut schema = Map::new();
    schema.insert("required".to_owned(), Value::Array(vec![Value::String(property.to_owned())]));
    Value::Object(schema)
}

/// Creates a Draft 2020-12 generator whose references target this command's bundled type schemas.
fn schema_generator(location: &[String]) -> SchemaGenerator {
    SchemaSettings::draft2020_12()
        .with(|settings| {
            settings.meta_schema = None;
            settings.definitions_path = Cow::Owned(definitions_path(location));
        })
        .into_generator()
}

/// Returns the absolute JSON Pointer used for Schemars-generated type definitions.
fn definitions_path(location: &[String]) -> String {
    let mut segments = location.iter().map(|segment| pointer_token(segment)).collect::<Vec<_>>();
    segments.extend(["$defs".to_owned(), "types".to_owned(), "$defs".to_owned()]);
    format!("/{}", segments.join("/"))
}

/// Escapes one JSON Pointer token for use inside a URI fragment reference.
fn pointer_token(token: &str) -> String {
    let escaped = token.replace('~', "~0").replace('/', "~1");
    let mut encoded = String::with_capacity(escaped.len());
    for byte in escaped.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'$') {
            encoded.push(char::from(byte));
        } else {
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

/// Wraps one definition map in a schema-valued `$defs` container.
fn definitions_schema(definitions: Map<String, Value>) -> Value {
    let mut schema = Map::new();
    schema.insert("$defs".to_owned(), Value::Object(definitions));
    Value::Object(schema)
}

/// Converts one generated schema into its JSON representation.
fn schema_value(schema: schemars::Schema) -> Value {
    schema.to_value()
}
