//! Typed binding over raw parser events.

use std::{ffi::OsString, fmt, str::FromStr};

use crate::{
    __private::{ActionKind, CommandArgs, RawValue},
    Error, InvalidValue,
    argv::{Error as RawError, Event},
    help,
};

/// Parses already-separated argument references into one derived command value.
///
/// # Errors
///
/// Returns the first raw syntax, typed cardinality, requiredness, or conversion error.
///
/// # Panics
///
/// Panics only when an implementation of the hidden generated-code contract exposes parse metadata
/// that it then refuses to bind. Derived implementations cannot violate this invariant.
pub(crate) fn parse_refs<T: CommandArgs>(argv: &[&std::ffi::OsStr]) -> Result<T, Error> {
    let mut partial = T::start();
    let mut parser: crate::argv::ArgvParser<'static, '_, '_> =
        crate::argv::ArgvParser::new(T::COMMAND, argv);

    while let Some(event) = parser.next_event() {
        let event = match event {
            Ok(Event::Action { action, long: used_long }) => {
                return match action.kind {
                    ActionKind::Help => {
                        let command_path = parser.command_path().collect::<Vec<_>>();
                        Err(Error::DisplayHelp { help: help::render(&command_path) })
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
    rendered.push_str(name);
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
