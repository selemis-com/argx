//! Public parse and typed-binding errors.

use std::{fmt, process};

/// Details for a value that the destination Rust type rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InvalidValue {
    /// Canonical argument name.
    pub name: &'static str,
    /// Text supplied by the caller.
    pub value: String,
    /// Conversion failure reported by the target type.
    pub reason: String,
}

/// A failure while parsing command-line arguments into a typed value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A flag-like token did not match any declared flag.
    UnknownFlag {
        /// Encoded token supplied by the caller.
        token: Vec<u8>,
    },
    /// A flag that consumes a value did not receive one.
    MissingValue {
        /// Canonical argument name.
        name: &'static str,
    },
    /// A value was attached to a switch that does not accept one.
    UnexpectedValue {
        /// Canonical argument name.
        name: &'static str,
    },
    /// A word could not be assigned to any positional argument.
    UnexpectedArgument {
        /// Encoded token supplied by the caller.
        token: Vec<u8>,
    },
    /// A word did not match any child command when command selection was required.
    UnknownCommand {
        /// Encoded token supplied by the caller.
        token: Vec<u8>,
    },
    /// A required subcommand field was not selected.
    MissingSubcommand {
        /// Canonical field name.
        name: &'static str,
    },
    /// A required field was not supplied.
    MissingRequired {
        /// Canonical argument name.
        name: &'static str,
    },
    /// A scalar argument was supplied more than once.
    DuplicateArgument {
        /// Canonical argument name.
        name: &'static str,
    },
    /// A text value was not valid UTF-8.
    InvalidUtf8 {
        /// Canonical argument name.
        name: &'static str,
        /// Encoded value supplied by the caller.
        value: Vec<u8>,
    },
    /// A UTF-8 value could not be converted to the field's Rust type.
    InvalidValue(Box<InvalidValue>),
    /// Encoded argument bytes could not be reconstructed as an operating-system string.
    InvalidOsValue {
        /// Canonical argument name.
        name: &'static str,
        /// Encoded value supplied by the caller.
        value: Vec<u8>,
    },
}

impl Error {
    /// Prints this error to standard error and terminates the process with the conventional
    /// command-line error status.
    pub fn exit(&self) -> ! {
        eprintln!("error: {self}");
        process::exit(2)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFlag { token } => {
                write!(formatter, "unknown flag `{}`", display_bytes(token))
            }
            Self::MissingValue { name } => write!(formatter, "missing value for `{name}`"),
            Self::UnexpectedValue { name } => {
                write!(formatter, "`{name}` does not accept a value")
            }
            Self::UnexpectedArgument { token } => {
                write!(formatter, "unexpected argument `{}`", display_bytes(token))
            }
            Self::UnknownCommand { token } => {
                write!(formatter, "unknown command `{}`", display_bytes(token))
            }
            Self::MissingSubcommand { name } => {
                write!(formatter, "required subcommand `{name}` was not provided")
            }
            Self::MissingRequired { name } => {
                write!(formatter, "required argument `{name}` was not provided")
            }
            Self::DuplicateArgument { name } => {
                write!(formatter, "argument `{name}` cannot be used more than once")
            }
            Self::InvalidUtf8 { name, value } => write!(
                formatter,
                "value `{}` for `{name}` is not valid UTF-8",
                display_bytes(value)
            ),
            Self::InvalidValue(error) => write!(
                formatter,
                "invalid value `{}` for `{}`: {}",
                display_bytes(error.value.as_bytes()),
                error.name,
                display_bytes(error.reason.as_bytes())
            ),
            Self::InvalidOsValue { name, value } => write!(
                formatter,
                "value `{}` for `{name}` cannot be reconstructed as an operating-system string",
                display_bytes(value)
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Lossily renders encoded argument bytes while escaping terminal control characters.
fn display_bytes(value: &[u8]) -> String {
    String::from_utf8_lossy(value).escape_debug().to_string()
}

#[cfg(test)]
mod tests {
    use super::display_bytes;

    #[test]
    fn diagnostic_bytes_do_not_emit_control_characters() {
        let rendered = display_bytes(b"--bad\n\x1b[31m");
        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\x1b'));
        assert!(rendered.contains(r"\n"));
    }
}
