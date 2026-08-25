//! Typed binding over raw parser events.

use std::{ffi::OsString, fmt, str::FromStr};

use crate::{__private::CommandArgs, Error, InvalidValue, argv::Error as RawError, help};

/// Parses already-separated argument references into one derived command value.
///
/// # Errors
///
/// Returns the first raw syntax, typed cardinality, requiredness, or conversion error.
///
/// # Panics
///
/// Panics only when an implementation of the hidden generated-code contract exposes parse metadata
/// whose keys it then refuses to bind. Derived implementations cannot violate this invariant.
pub(crate) fn parse_refs<T: CommandArgs>(argv: &[&std::ffi::OsStr]) -> Result<T, Error> {
    let mut partial = T::start();
    let mut parser: crate::argv::ArgvParser<'static, '_, '_> =
        crate::argv::ArgvParser::new(T::COMMAND, argv);
    let mut command_path = Vec::new();

    while let Some(event) = parser.next_event() {
        let event = match event {
            Ok(event) => event,
            Err(RawError::DisplayHelp) => {
                let rendered = if command_path.is_empty() {
                    help::render(&[T::COMMAND])
                } else {
                    help::render(&command_path)
                };
                return Err(Error::DisplayHelp { help: rendered });
            }
            Err(error) => return Err(raw_error(error)),
        };
        let applied = T::apply(&mut partial, &event);
        assert!(applied, "generated command metadata and binding keys diverged");
        if let crate::argv::Event::Command { command } = event {
            if command_path.is_empty() {
                command_path.push(T::COMMAND);
            }
            command_path.push(command);
        }
    }

    T::check(&mut partial)?;
    T::finish(partial)
}

/// Converts one raw-parser error into the owned public error type.
fn raw_error(error: RawError<'static, '_>) -> Error {
    match error {
        RawError::DisplayHelp => unreachable!("help requests are handled with command context"),
        RawError::UnknownFlag { token } => Error::UnknownFlag { token: token.to_vec() },
        RawError::MissingFlagValue { flag } => Error::MissingValue { name: flag.name },
        RawError::UnexpectedFlagValue { flag } => Error::UnexpectedValue { name: flag.name },
        RawError::UnexpectedArg { token } => Error::UnexpectedArgument { token: token.to_vec() },
        RawError::UnknownCommand { token } => Error::UnknownCommand { token: token.to_vec() },
    }
}

/// Converts one raw value to UTF-8 text without copying valid bytes.
///
/// # Errors
///
/// Returns an error when the bytes are not valid UTF-8.
pub(crate) fn text_value(value: Vec<u8>, name: &'static str) -> Result<String, Error> {
    String::from_utf8(value).map_err(|bad| Error::InvalidUtf8 { name, value: bad.into_bytes() })
}

/// Converts repeated raw values to UTF-8 text.
///
/// # Errors
///
/// Returns the first invalid UTF-8 value.
pub(crate) fn text_values(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<String>, Error> {
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        parsed.push(text_value(value, name)?);
    }
    Ok(parsed)
}

/// Converts one raw value through UTF-8 and [`FromStr`].
///
/// # Errors
///
/// Returns an error when the bytes are not UTF-8 or the destination rejects the text.
pub(crate) fn parsed_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, Error>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let text = text_value(value, name)?;
    T::from_str(&text).map_err(|reason| {
        Error::InvalidValue(Box::new(InvalidValue {
            name,
            value: text,
            reason: reason.to_string(),
        }))
    })
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
        parsed.push(parsed_value(value, name)?);
    }
    Ok(parsed)
}

/// Converts one raw value to an operating-system string without forcing UTF-8 on Unix.
///
/// # Errors
///
/// Returns an error when this platform cannot safely reconstruct the encoded bytes.
pub(crate) fn os_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, Error>
where
    T: From<OsString>,
{
    os_string(value, name).map(T::from)
}

/// Converts repeated raw values to operating-system strings.
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
        parsed.push(os_value(value, name)?);
    }
    Ok(parsed)
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
