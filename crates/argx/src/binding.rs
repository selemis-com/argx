//! Typed binding over raw parser events.
//!
//! Binding is intentionally staged. The raw parser first consumes the complete token stream, then
//! generated code checks scalar occurrence counts and required values and relationships, then
//! converts raw values into destination Rust types. This
//! ordering is part of the public diagnostic contract: an earlier syntax error is not masked by a
//! later duplicate, missing value, or conversion failure.

use crate::{
    __private::{ActionKind, CommandArgs},
    Error,
    argv::{Error as RawError, Event},
    error::display_bytes,
    help,
};

/// Parses already-separated argument references into one derived command value.
///
/// # Errors
///
/// Returns the first raw syntax, typed cardinality, requiredness, relationship, or conversion
/// error according to the binding pipeline's precedence.
///
/// # Panics
///
/// Panics only when an implementation of the hidden generated-code contract exposes parse metadata
/// that it then refuses to bind. Derived implementations cannot violate this invariant.
pub(crate) fn parse_refs<T: CommandArgs>(argv: &[&std::ffi::OsStr]) -> Result<T, Error> {
    parse_refs_inner::<T>(argv, None)
}

/// Parses arguments with machine-readable schema discovery enabled.
pub(crate) fn parse_refs_with_schema<T: CommandArgs>(
    argv: &[&std::ffi::OsStr],
    registry: &crate::__private::SchemaRegistry,
) -> Result<T, Error> {
    parse_refs_inner::<T>(argv, Some(registry))
}

/// Shared typed-binding pipeline with an optional schema-discovery registry.
fn parse_refs_inner<T: CommandArgs>(
    argv: &[&std::ffi::OsStr],
    registry: Option<&crate::__private::SchemaRegistry>,
) -> Result<T, Error> {
    let mut partial = T::start();
    let mut parser: crate::argv::ArgvParser<'static, '_, '_> =
        crate::argv::ArgvParser::new_with_schema(T::COMMAND, argv, registry.is_some());

    while let Some(event) = parser.next_event() {
        let event = match event {
            Ok(Event::Action { action, long: used_long }) => {
                return match action.kind {
                    ActionKind::Help => {
                        let command_path = parser.command_path().collect::<Vec<_>>();
                        Err(Error::DisplayHelp {
                            help: help::render_with_schema(&command_path, registry.is_some()),
                        })
                    }
                    ActionKind::Schema => {
                        let command_path = parser.command_path().collect::<Vec<_>>();
                        Err(crate::schema_discovery::display_schema(
                            &command_path,
                            registry.expect("schema action requires a schema registry"),
                        ))
                    }
                    ActionKind::Version { short, long } => {
                        let version = if used_long { long } else { short };
                        let command = parser
                            .command_path()
                            .last()
                            .expect("parser always has a selected command");
                        Err(Error::DisplayVersion {
                            version: render_version(command.name, version),
                        })
                    }
                };
            }
            Ok(event) => event,
            Err(error) => return Err(raw_error(error)),
        };
        let applied = T::apply(&mut partial, &event);
        assert!(applied, "generated command metadata and binding diverged");
    }

    T::check(&mut partial)?;
    T::finish(partial)
}

/// Converts one raw-parser error into the owned public error type.
fn raw_error(error: RawError<'static, '_>) -> Error {
    match error {
        RawError::UnexpectedActionValue { action } => {
            Error::UnexpectedValue { name: action.diagnostic }
        }
        RawError::UnknownFlag { token } => Error::UnknownFlag { token: token.to_vec() },
        RawError::MissingFlagValue { flag } => Error::MissingValue { name: flag.diagnostic },
        RawError::UnexpectedFlagValue { flag } => Error::UnexpectedValue { name: flag.diagnostic },
        RawError::UnexpectedArg { token } => Error::UnexpectedArgument { token: token.to_vec() },
        RawError::UnknownCommand { token } => Error::UnknownCommand { token: token.to_vec() },
    }
}

/// Renders one successful version action deterministically.
fn render_version(name: &str, version: &str) -> String {
    let version = version.trim_end_matches('\n');
    let mut rendered = String::with_capacity(name.len() + version.len() + 2);
    rendered.push_str(&display_bytes(name.as_bytes()));
    if !version.is_empty() {
        rendered.push(' ');
        rendered.push_str(version);
    }
    rendered.push('\n');
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_command_name_does_not_emit_terminal_controls() {
        let rendered = render_version("tool\n\u{1b}[31m", "1.2.3");
        let body = rendered.strip_suffix('\n').expect("version output ends with one newline");

        assert!(!body.contains('\n'));
        assert!(!body.contains('\u{1b}'));
        assert!(body.starts_with(r"tool\n"));
        assert!(body.ends_with(" 1.2.3"));
    }
}
