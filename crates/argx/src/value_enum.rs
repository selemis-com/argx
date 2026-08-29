//! Finite command-line value vocabularies.
//!
//! [`trait@ValueEnum`] is the explicit finite alternative to arbitrary `FromStr` parsing. A derived
//! value enum supplies one canonical lexical vocabulary that generated binding, help, and machine
//! contracts can all project without duplicating the accepted values in documentation.

use crate::error::display_bytes;

/// A finite set of canonical command-line values.
///
/// Derive this trait with `#[derive(argx::ValueEnum)]`, then opt a field into the vocabulary with
/// `#[argx(value_enum)]`. Derived variants use Argx's normal kebab-case spelling and parsing is
/// exact and case-sensitive.
pub trait ValueEnum: Sized {
    /// Canonical values accepted from the command line, in declaration order.
    const VALUES: &'static [&'static str];

    /// Parses one canonical value.
    ///
    /// This method is part of the generated-code contract. Applications normally parse through a
    /// derived [`crate::Parser`] field or through the `FromStr` implementation emitted by the
    /// `ValueEnum` derive.
    #[doc(hidden)]
    fn from_value(value: &str) -> Option<Self>;
}

/// Error returned by the `FromStr` implementation generated for a [`trait@ValueEnum`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{}", display_value_enum_values(.values))]
pub struct ValueEnumError {
    /// Canonical values expected by the generated parser.
    values: &'static [&'static str],
}

impl ValueEnumError {
    /// Creates an error describing one finite accepted vocabulary.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(values: &'static [&'static str]) -> Self {
        Self { values }
    }
}

/// Renders the finite accepted vocabulary for a value-enum diagnostic.
fn display_value_enum_values(values: &[&str]) -> String {
    if values.is_empty() {
        return String::from("no values are accepted");
    }

    let mut rendered = String::from("expected one of: ");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            rendered.push_str(", ");
        }
        rendered.push_str(&display_bytes(value.as_bytes()));
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_values_do_not_emit_terminal_controls() {
        static VALUES: &[&str] = &["safe", "bad\n\u{1b}[31m"];
        let rendered = ValueEnumError::new(VALUES).to_string();

        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.contains(r"bad\n"));
    }
}
