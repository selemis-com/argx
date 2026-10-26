//! Value conversion used by generated binding code.
//!
//! Generated downstream code names these functions through `argx::__private`, so the conversion
//! implementation lives at that boundary instead of forwarding through a second runtime layer.

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt as _;
use std::{ffi::OsString, fmt, str::FromStr};

use crate::{Error, ValueEnum, ValueEnumError};

/// Splits repeated raw values on commas while preserving occurrence order.
///
/// Empty segments are preserved and are subsequently handled by the destination value parser.
pub fn comma_values(values: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut split = Vec::new();
    for value in values {
        split.extend(value.split(|byte| *byte == b',').map(<[u8]>::to_vec));
    }
    split
}

/// Converts one raw value to UTF-8 text.
///
/// # Errors
///
/// Returns an error when the value is not valid UTF-8.
pub fn text_value(value: Vec<u8>, name: &'static str) -> Result<String, Error> {
    text_bytes(value, name)
}

/// Converts repeated raw values to UTF-8 text.
///
/// # Errors
///
/// Returns the first invalid UTF-8 value.
pub fn text_values(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<String>, Error> {
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
pub fn parsed_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, Error>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let text = text_bytes(value, name)?;
    T::from_str(&text).map_err(|reason| Error::InvalidValue {
        name,
        value: text,
        reason: reason.to_string(),
    })
}

/// Converts repeated raw values through UTF-8 and [`FromStr`].
///
/// # Errors
///
/// Returns the first UTF-8 or destination conversion failure.
pub fn parsed_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, Error>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let text = text_bytes(value, name)?;
        parsed.push(T::from_str(&text).map_err(|reason| Error::InvalidValue {
            name,
            value: text,
            reason: reason.to_string(),
        })?);
    }
    Ok(parsed)
}

/// Converts one raw value through a finite [`trait@ValueEnum`] vocabulary.
///
/// # Errors
///
/// Returns an error when the value is not UTF-8 or is not one of the enum's canonical values.
pub fn value_enum_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, Error>
where
    T: ValueEnum,
{
    let text = text_bytes(value, name)?;
    T::from_value(&text).ok_or_else(|| Error::InvalidValue {
        name,
        value: text,
        reason: ValueEnumError::new(T::VALUES).to_string(),
    })
}

/// Converts repeated raw values through a finite [`trait@ValueEnum`] vocabulary.
///
/// # Errors
///
/// Returns the first invalid UTF-8 value or value outside the enum's canonical vocabulary.
pub fn value_enum_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, Error>
where
    T: ValueEnum,
{
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let text = text_bytes(value, name)?;
        let Some(value) = T::from_value(&text) else {
            return Err(Error::InvalidValue {
                name,
                value: text,
                reason: ValueEnumError::new(T::VALUES).to_string(),
            });
        };
        parsed.push(value);
    }
    Ok(parsed)
}

/// Converts one raw value to an operating-system-backed destination type.
///
/// Unix preserves arbitrary bytes. Other platforms require UTF-8, so Windows arguments
/// containing unpaired surrogates are rejected rather than reconstructed unchecked.
///
/// # Errors
///
/// Returns [`Error::InvalidUtf8`] when a non-Unix value is not valid UTF-8.
pub fn os_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, Error>
where
    T: From<OsString>,
{
    os_string(value).map(T::from).map_err(|value| Error::InvalidUtf8 { name, value })
}

/// Converts repeated raw values to operating-system-backed destination types.
///
/// The platform restrictions described by [`os_value`] apply to every value.
///
/// # Errors
///
/// Returns the first [`Error::InvalidUtf8`] when a non-Unix value is not valid UTF-8.
pub fn os_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, Error>
where
    T: From<OsString>,
{
    values.into_iter().map(|value| os_value(value, name)).collect()
}

/// Converts encoded bytes into UTF-8 text.
fn text_bytes(value: Vec<u8>, name: &'static str) -> Result<String, Error> {
    String::from_utf8(value).map_err(|bad| Error::InvalidUtf8 { name, value: bad.into_bytes() })
}

/// Reconstructs an operating-system string without assuming byte provenance.
///
/// The public generated-code helpers accept arbitrary byte vectors, so their contents cannot
/// safely be passed to `from_encoded_bytes_unchecked`. Unix accepts all byte sequences;
/// elsewhere, validated UTF-8 provides a safe conversion at the cost of rejecting unpaired
/// Windows surrogates. On failure the original bytes are retained for diagnostics.
fn os_string(value: Vec<u8>) -> Result<OsString, Vec<u8>> {
    #[cfg(unix)]
    {
        Ok(OsString::from_vec(value))
    }
    #[cfg(not(unix))]
    {
        match String::from_utf8(value) {
            Ok(text) => Ok(OsString::from(text)),
            Err(bad) => Err(bad.into_bytes()),
        }
    }
}
