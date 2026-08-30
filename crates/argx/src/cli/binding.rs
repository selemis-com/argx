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
    cli::{
        argv::{Error as RawError, Event},
        help,
    },
    error::display_bytes,
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

/// Parses arguments with schema discovery enabled.
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
    let mut supplied = Vec::new();
    let mut parser: crate::cli::argv::ArgvParser<'static, '_, '_> =
        crate::cli::argv::ArgvParser::new_with_schema(T::COMMAND, argv, registry.is_some());

    while let Some(event) = parser.next_event() {
        let event = match event {
            Ok(Event::Action { action, long: used_long }) => {
                return match action.kind {
                    ActionKind::Help => {
                        let command_path = parser.command_path().collect::<Vec<_>>();
                        Err(Error::DisplayHelp {
                            help: help::render_with_schema(
                                &command_path,
                                registry.is_some(),
                                if used_long {
                                    help::HelpStyle::Long
                                } else {
                                    help::HelpStyle::Short
                                },
                            ),
                        })
                    }
                    ActionKind::Schema => {
                        let command_path = parser.command_path().collect::<Vec<_>>();
                        let full = matches!(
                            parser.remaining_args(),
                            [arg] if *arg == std::ffi::OsStr::new("--full")
                        );
                        Err(crate::schema::display_schema(
                            &command_path,
                            registry.expect("schema action requires a schema registry"),
                            full,
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
        match &event {
            Event::Flag { flag, .. } => supplied.push(flag.key),
            Event::Arg { arg, .. } => supplied.push(arg.key),
            Event::Command { .. } | Event::Action { .. } => {}
        }
        let applied = T::apply(&mut partial, &event);
        assert!(applied, "generated command metadata and binding diverged");
    }

    if let Err(error) = T::check(&mut partial) {
        let command_path = parser.command_path().collect::<Vec<_>>();
        return match error {
            Error::MissingSubcommand { .. } => Err(Error::DisplayHelp {
                help: help::render_with_schema(
                    &command_path,
                    registry.is_some(),
                    help::HelpStyle::Short,
                ),
            }),
            Error::MissingRequired { name } => {
                let arguments = help::missing_required_labels(&command_path, &supplied);
                let argument = if arguments.is_empty() {
                    help::missing_required_label(&command_path, name)
                        .unwrap_or_else(|| name.to_owned())
                } else {
                    arguments.join("\n  ")
                };
                Err(Error::MissingRequiredArguments {
                    argument,
                    usage: help::render_required_usage(&command_path),
                })
            }
            error => Err(error),
        };
    }
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
