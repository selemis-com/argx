//! Machine-readable discovery over the statically derived Argx command topology.
//!
//! Handler associations are collected into a short-lived local registry by generated trait calls
//! over the concrete command types. The registry is neither global registration nor linker
//! inventory.

use std::ffi::OsStr;

use serde_json::{Map, Value};

use crate::{
    Error,
    command::model::{Command, Key},
    derive_support::traits::{HandlerSchemaSource, HandlerSchemas},
};

/// One executable command association collected from generated schema topology.
#[derive(Clone, Copy, Debug)]
struct Entry {
    /// Stable semantic identity of the command table.
    key: Key,
    /// Result/error schema generator supplied by the handler association.
    schemas: fn() -> HandlerSchemas,
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
        schemas: fn() -> HandlerSchemas,
    ) {
        self.entries.push(Entry { key: command.key, schemas });
    }

    /// Resolves one selected command's handler schemas.
    fn handler(&self, command: &Command<'_>) -> Option<HandlerSchemas> {
        self.entries.iter().find(|entry| entry.key == command.key).map(|entry| (entry.schemas)())
    }

    /// Reports whether one command is an executable schema leaf.
    fn contains(&self, command: &Command<'_>) -> bool {
        self.entries.iter().any(|entry| entry.key == command.key)
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

/// Builds a successful terminal schema action for one selected command path.
pub(crate) fn display_schema(path: &[&Command<'_>], registry: &Registry) -> Error {
    let command = path.last().copied().expect("schema discovery always has a root command");

    let mut document = Map::new();
    document.insert("command".to_owned(), command_document(path, command, registry));
    document.insert(
        "invocation".to_owned(),
        serde_json::to_value(crate::invocation_schema::invocation_schema_for_path(path))
            .expect("Schemars invocation schemas must serialize"),
    );

    if let Some(handler) = registry.handler(command) {
        document.insert(
            "result".to_owned(),
            serde_json::to_value(handler.result).expect("Schemars result schemas must serialize"),
        );
        document.insert(
            "error".to_owned(),
            serde_json::to_value(handler.error).expect("Schemars error schemas must serialize"),
        );
    }

    if !command.subcommands.is_empty() {
        let parent_path = path.iter().skip(1).map(|command| command.name).collect::<Vec<_>>();
        let children = command
            .subcommands
            .iter()
            .map(|child| subcommand_summary(&parent_path, child, registry))
            .collect();
        document.insert("subcommands".to_owned(), Value::Array(children));
    }

    let mut schema = serde_json::to_string_pretty(&Value::Object(document))
        .expect("schema document must serialize");
    schema.push('\n');
    Error::DisplaySchema { schema }
}

/// Builds stable command metadata for one selected schema document.
fn command_document(path: &[&Command<'_>], command: &Command<'_>, registry: &Registry) -> Value {
    let mut object = Map::new();
    object.insert(
        "path".to_owned(),
        Value::Array(
            path.iter().skip(1).map(|command| Value::String(command.name.to_owned())).collect(),
        ),
    );
    object.insert("name".to_owned(), Value::String(command.name.to_owned()));
    if let Some(about) = command.about {
        object.insert("about".to_owned(), Value::String(about.to_owned()));
    }
    object.insert("invocable".to_owned(), Value::Bool(registry.contains(command)));
    Value::Object(object)
}

/// Builds one immediate-child summary for structural discovery.
fn subcommand_summary(parent: &[&str], command: &Command<'_>, registry: &Registry) -> Value {
    let mut object = Map::new();
    let mut path =
        parent.iter().map(|segment| Value::String((*segment).to_owned())).collect::<Vec<_>>();
    path.push(Value::String(command.name.to_owned()));
    object.insert("path".to_owned(), Value::Array(path));
    object.insert("name".to_owned(), Value::String(command.name.to_owned()));
    if let Some(about) = command.about {
        object.insert("about".to_owned(), Value::String(about.to_owned()));
    }
    object.insert("invocable".to_owned(), Value::Bool(registry.contains(command)));
    Value::Object(object)
}
