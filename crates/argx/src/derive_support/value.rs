//! Value-conversion entry points used by generated binding code.
//!
//! The public facade keeps these functions hidden, but generated downstream code needs stable paths
//! to them.

/// Converts one raw value directly into a UTF-8 string.
///
/// # Errors
///
/// Returns an error when the value is not valid UTF-8.
pub fn text_value(value: Vec<u8>, name: &'static str) -> Result<String, crate::Error> {
    crate::binding::text_value(value, name)
}

/// Converts repeated raw values directly into UTF-8 strings.
///
/// # Errors
///
/// Returns the first invalid UTF-8 value.
pub fn text_values(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<String>, crate::Error> {
    crate::binding::text_values(values, name)
}

/// Converts one raw value through UTF-8 and the destination type's `FromStr` implementation.
///
/// # Errors
///
/// Returns an error for invalid UTF-8 or a destination conversion failure.
pub fn parsed_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, crate::Error>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    crate::binding::parsed_value(value, name)
}

/// Converts repeated raw values through UTF-8 and `FromStr`.
///
/// # Errors
///
/// Returns the first conversion failure.
pub fn parsed_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, crate::Error>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    crate::binding::parsed_values(values, name)
}

/// Converts one raw value through a finite [`trait@crate::ValueEnum`] vocabulary.
///
/// # Errors
///
/// Returns an error for invalid UTF-8 or a value outside the enum's canonical vocabulary.
pub fn value_enum_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, crate::Error>
where
    T: crate::ValueEnum,
{
    crate::binding::value_enum_value(value, name)
}

/// Converts repeated raw values through a finite [`trait@crate::ValueEnum`] vocabulary.
///
/// # Errors
///
/// Returns the first invalid UTF-8 value or value outside the enum's canonical vocabulary.
pub fn value_enum_values<T>(
    values: Vec<Vec<u8>>,
    name: &'static str,
) -> Result<Vec<T>, crate::Error>
where
    T: crate::ValueEnum,
{
    crate::binding::value_enum_values(values, name)
}

/// Converts one raw value into an operating-system-backed destination type.
///
/// # Errors
///
/// Returns an error when argv bytes cannot be reconstructed as an operating-system string.
pub fn os_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, crate::Error>
where
    T: From<std::ffi::OsString>,
{
    crate::binding::os_value(value, name)
}

/// Converts repeated raw values into operating-system-backed destination types.
///
/// # Errors
///
/// Returns the first operating-system string reconstruction failure.
pub fn os_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, crate::Error>
where
    T: From<std::ffi::OsString>,
{
    crate::binding::os_values(values, name)
}
