//! Machine-readable JSON Schema generation over the statically derived Argx command topology.
//!
//! Handler associations are collected into a short-lived local registry by generated trait calls
//! over the concrete command types. The registry is neither global registration nor linker
//! inventory.

use std::{borrow::Cow, ffi::OsStr, fmt::Write as _};

use schemars::{SchemaGenerator, generate::SchemaSettings};
use serde_json::{Map, Value};

mod invocation;

use crate::{
    Error,
    cli::{
        command::{Command, Key},
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

    Some(display_schema(&path, registry))
}

/// Builds a successful terminal JSON Schema action for one selected command path.
pub(crate) fn display_schema(path: &[&Command<'_>], registry: &Registry) -> Error {
    let schema = command_schema(path, registry, &[]);
    let mut rendered =
        serde_json::to_string_pretty(&schema).expect("schema document must serialize");
    rendered.push('\n');
    Error::DisplaySchema { schema: rendered }
}

/// Builds one schema-compliant command document.
///
/// The selected command's explicit invocation values form the root schema. Handler result and error
/// schemas are bundled under `$defs`, along with any Schemars-generated type definitions.
/// Structural commands bundle child command schemas under `$defs.subcommands.$defs`.
fn command_schema(path: &[&Command<'_>], registry: &Registry, location: &[String]) -> Value {
    let command = path.last().copied().expect("schema discovery always has a root command");
    let mut schema = serde_json::to_value(invocation::invocation_schema_for_path(path))
        .expect("invocation schema must serialize");
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
        let mut subcommands = Map::new();
        for &child in command.subcommands {
            let mut child_path = path.to_vec();
            child_path.push(child);

            let mut child_location = location.to_vec();
            child_location.extend([
                "$defs".to_owned(),
                "subcommands".to_owned(),
                "$defs".to_owned(),
                child.name.to_owned(),
            ]);

            subcommands.insert(
                child.name.to_owned(),
                command_schema(&child_path, registry, &child_location),
            );
        }
        definitions.insert("subcommands".to_owned(), definitions_schema(subcommands));
    }

    if !definitions.is_empty() {
        object.insert("$defs".to_owned(), Value::Object(definitions));
    }

    schema
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
