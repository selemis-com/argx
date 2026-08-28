//! Typed binding over raw parser events.
//!
//! Binding is intentionally staged. The raw parser first consumes the complete token stream, then
//! generated code checks scalar occurrence counts, applies environment fallbacks, checks required
//! values and relationships, and finally converts raw values into destination Rust types. This
//! ordering is part of the public diagnostic contract: an earlier syntax error is not masked by a
//! later duplicate, missing value, or conversion failure.

use std::{ffi::OsString, fmt, str::FromStr};

use crate::{
    __private::{ActionKind, CommandArgs, RawValue},
    Error, InvalidValue, ValueEnum, ValueEnumError,
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

/// Converts one raw value to UTF-8 text.
///
/// # Errors
///
/// Returns an error when the value is not valid UTF-8.
pub(crate) fn text_value(value: RawValue, name: &'static str) -> Result<String, Error> {
    match value {
        RawValue::Argv(value) => text_bytes(value, name),
        RawValue::Environment { name: environment, value } => {
            environment_text(value, name, environment)
        }
    }
}

/// Converts repeated raw values to UTF-8 text.
///
/// # Errors
///
/// Returns the first invalid UTF-8 value.
pub(crate) fn text_values(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<String>, Error> {
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        parsed.push(text_bytes(value, name)?);
    }
    Ok(parsed)
}

/// Converts one raw value through UTF-8 and [`FromStr`].
///
/// # Errors
///
/// Returns an error when the value is not UTF-8 or the destination rejects the text.
pub(crate) fn parsed_value<T>(value: RawValue, name: &'static str) -> Result<T, Error>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    match value {
        RawValue::Argv(value) => {
            let text = text_bytes(value, name)?;
            T::from_str(&text).map_err(|reason| {
                Error::InvalidValue(Box::new(InvalidValue {
                    name,
                    value: text,
                    reason: reason.to_string(),
                }))
            })
        }
        RawValue::Environment { name: environment, value } => {
            let text = environment_text(value, name, environment)?;
            T::from_str(&text).map_err(|reason| Error::InvalidEnvironmentValue {
                name,
                environment,
                value: OsString::from(text.as_str()),
                reason: reason.to_string(),
            })
        }
    }
}

/// Converts repeated raw values through UTF-8 and [`FromStr`].
///
/// # Errors
///
/// Returns the first UTF-8 or destination conversion failure.
pub(crate) fn parsed_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, Error>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let text = text_bytes(value, name)?;
        parsed.push(T::from_str(&text).map_err(|reason| {
            Error::InvalidValue(Box::new(InvalidValue {
                name,
                value: text,
                reason: reason.to_string(),
            }))
        })?);
    }
    Ok(parsed)
}

/// Converts one raw value through a finite [`trait@ValueEnum`] vocabulary.
///
/// # Errors
///
/// Returns an error when the value is not UTF-8 or is not one of the enum's canonical values.
pub(crate) fn value_enum_value<T>(value: RawValue, name: &'static str) -> Result<T, Error>
where
    T: ValueEnum,
{
    let reason = || ValueEnumError::new(T::VALUES).to_string();
    match value {
        RawValue::Argv(value) => {
            let text = text_bytes(value, name)?;
            T::from_value(&text).ok_or_else(|| {
                Error::InvalidValue(Box::new(InvalidValue { name, value: text, reason: reason() }))
            })
        }
        RawValue::Environment { name: environment, value } => {
            let text = environment_text(value, name, environment)?;
            T::from_value(&text).ok_or_else(|| Error::InvalidEnvironmentValue {
                name,
                environment,
                value: OsString::from(text.as_str()),
                reason: reason(),
            })
        }
    }
}

/// Converts repeated raw values through a finite [`trait@ValueEnum`] vocabulary.
///
/// # Errors
///
/// Returns the first invalid UTF-8 value or value outside the enum's canonical vocabulary.
pub(crate) fn value_enum_values<T>(
    values: Vec<Vec<u8>>,
    name: &'static str,
) -> Result<Vec<T>, Error>
where
    T: ValueEnum,
{
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let text = text_bytes(value, name)?;
        let Some(value) = T::from_value(&text) else {
            return Err(Error::InvalidValue(Box::new(InvalidValue {
                name,
                value: text,
                reason: ValueEnumError::new(T::VALUES).to_string(),
            })));
        };
        parsed.push(value);
    }
    Ok(parsed)
}

/// Converts one raw value to an operating-system-backed destination type.
///
/// # Errors
///
/// Returns an error when argv bytes cannot be reconstructed as an operating-system string.
pub(crate) fn os_value<T>(value: RawValue, name: &'static str) -> Result<T, Error>
where
    T: From<OsString>,
{
    let value = match value {
        RawValue::Argv(value) => os_string(value, name)?,
        RawValue::Environment { value, .. } => value,
    };
    Ok(T::from(value))
}

/// Converts repeated raw values to operating-system-backed destination types.
///
/// # Errors
///
/// Returns the first operating-system string reconstruction failure.
pub(crate) fn os_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, Error>
where
    T: From<OsString>,
{
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        parsed.push(T::from(os_string(value, name)?));
    }
    Ok(parsed)
}

/// Converts encoded bytes into UTF-8 text.
fn text_bytes(value: Vec<u8>, name: &'static str) -> Result<String, Error> {
    String::from_utf8(value).map_err(|bad| Error::InvalidUtf8 { name, value: bad.into_bytes() })
}

/// Converts one environment value to text while preserving its source in failures.
fn environment_text(
    value: OsString,
    name: &'static str,
    environment: &'static str,
) -> Result<String, Error> {
    value.into_string().map_err(|value| Error::InvalidEnvironmentValue {
        name,
        environment,
        value,
        reason: String::from("value is not valid UTF-8"),
    })
}

/// Reconstructs an operating-system string from bytes emitted by the raw argv parser.
fn os_string(value: Vec<u8>, name: &'static str) -> Result<OsString, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt as _;
        let _ = name;
        Ok(OsString::from_vec(value))
    }

    #[cfg(not(unix))]
    {
        match String::from_utf8(value) {
            Ok(text) => Ok(OsString::from(text)),
            Err(bad) => Err(Error::InvalidOsValue { name, value: bad.into_bytes() }),
        }
    }
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
