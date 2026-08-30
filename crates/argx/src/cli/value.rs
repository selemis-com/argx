//! Value conversion used by generated binding code.
//!
//! Generated downstream code names these functions through `argx::__private`, so the conversion
//! implementation lives at that boundary instead of forwarding through a second runtime layer.

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
pub fn os_value<T>(value: Vec<u8>) -> T
where
    T: From<OsString>,
{
    T::from(os_string(value))
}

/// Converts repeated raw values to operating-system-backed destination types.
pub fn os_values<T>(values: Vec<Vec<u8>>) -> Vec<T>
where
    T: From<OsString>,
{
    values.into_iter().map(|value| T::from(os_string(value))).collect()
}

/// Converts encoded bytes into UTF-8 text.
fn text_bytes(value: Vec<u8>, name: &'static str) -> Result<String, Error> {
    String::from_utf8(value).map_err(|bad| Error::InvalidUtf8 { name, value: bad.into_bytes() })
}

/// Reconstructs an operating-system string from raw argv bytes.
fn os_string(value: Vec<u8>) -> OsString {
    use std::os::unix::ffi::OsStringExt as _;

    OsString::from_vec(value)
}
