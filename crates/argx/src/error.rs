//! Public parse and typed-binding errors.

use std::{
    fmt,
    io::{self, Write as _},
    process,
};

/// Details for a value that the destination Rust type rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InvalidValue {
    /// Canonical user-facing argument label.
    pub name: &'static str,
    /// Text supplied on argv.
    pub value: String,
    /// Conversion failure reported by the target type.
    pub reason: String,
}

/// A control-flow or failure result while parsing command-line arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Generated help requested through the built-in help switch.
    DisplayHelp {
        /// Fully rendered help text for the selected command scope.
        help: String,
    },
    /// Version information requested through the built-in version action.
    DisplayVersion {
        /// Fully rendered version text for the selected command scope.
        version: String,
    },
    /// A flag-like token did not match any declared flag.
    UnknownFlag {
        /// Encoded token supplied by the caller.
        token: Vec<u8>,
    },
    /// A flag that consumes a value did not receive one.
    MissingValue {
        /// Canonical user-facing argument label.
        name: &'static str,
    },
    /// A value was attached to a switch that does not accept one.
    UnexpectedValue {
        /// Canonical user-facing argument label.
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
        /// Canonical user-facing argument label.
        name: &'static str,
    },
    /// A scalar argument was supplied more than once.
    DuplicateArgument {
        /// Canonical user-facing argument label.
        name: &'static str,
    },
    /// An argument required by another supplied argument was not available.
    MissingRequirement {
        /// Canonical user-facing label of the missing argument.
        name: &'static str,
        /// Canonical user-facing label of the argument imposing the requirement.
        required_by: &'static str,
    },
    /// Two arguments declared to conflict were supplied together.
    ConflictingArguments {
        /// Canonical user-facing label of the argument declaring the conflict.
        name: &'static str,
        /// Canonical user-facing label of the conflicting argument.
        other: &'static str,
    },
    /// A text value was not valid UTF-8.
    InvalidUtf8 {
        /// Canonical user-facing argument label.
        name: &'static str,
        /// Encoded value supplied by the caller.
        value: Vec<u8>,
    },
    /// A UTF-8 value could not be converted to the field's Rust type.
    InvalidValue(Box<InvalidValue>),
    /// A value supplied by an environment variable could not be converted.
    InvalidEnvironmentValue {
        /// Canonical user-facing argument label.
        name: &'static str,
        /// Environment variable that supplied the value.
        environment: &'static str,
        /// Operating-system value read from the environment.
        value: std::ffi::OsString,
        /// Conversion failure reported by Argx or the target type.
        reason: String,
    },
    /// Encoded argument bytes could not be reconstructed as an operating-system string.
    InvalidOsValue {
        /// Canonical user-facing argument label.
        name: &'static str,
        /// Encoded value supplied by the caller.
        value: Vec<u8>,
    },
}

impl Error {
    /// Returns the process status used by [`Self::exit`].
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::DisplayHelp { .. } | Self::DisplayVersion { .. } => 0,
            _ => 2,
        }
    }

    /// Prints this result to the appropriate stream and terminates the process.
    ///
    /// Help and version requests are written to standard output and exit successfully. Parse and
    /// binding failures are written to standard error and exit with status 2.
    pub fn exit(&self) -> ! {
        match self {
            Self::DisplayHelp { help } => {
                let mut stdout = io::stdout().lock();
                let _ = stdout.write_all(help.as_bytes());
                let _ = stdout.flush();
            }
            Self::DisplayVersion { version } => {
                let mut stdout = io::stdout().lock();
                let _ = stdout.write_all(version.as_bytes());
                let _ = stdout.flush();
            }
            _ => {
                let mut stderr = io::stderr().lock();
                let _ = writeln!(stderr, "error: {self}\n\nFor more information, try '--help'.");
                let _ = stderr.flush();
            }
        }
        process::exit(self.exit_code())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DisplayHelp { help } => formatter.write_str(help),
            Self::DisplayVersion { version } => formatter.write_str(version),
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
            Self::MissingRequirement { name, required_by } => {
                write!(formatter, "argument `{name}` is required when `{required_by}` is used")
            }
            Self::ConflictingArguments { name, other } => {
                write!(formatter, "argument `{name}` cannot be used with `{other}`")
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
            Self::InvalidEnvironmentValue { name, environment, value, reason } => write!(
                formatter,
                "invalid value `{}` from environment variable `{environment}` for `{name}`: {}",
                display_bytes(value.as_os_str().as_encoded_bytes()),
                display_bytes(reason.as_bytes()),
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
    use super::{Error, display_bytes};

    #[test]
    fn diagnostic_bytes_do_not_emit_control_characters() {
        let rendered = display_bytes(b"--bad\n\x1b[31m");
        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\x1b'));
        assert!(rendered.contains(r"\n"));
    }

    #[test]
    fn display_actions_use_success_status_and_render_verbatim() {
        let help = Error::DisplayHelp { help: "Usage: tool [OPTIONS]\n".to_owned() };
        assert_eq!(help.exit_code(), 0);
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            help.to_string(),
            snapbox::str![[r#"
Usage: tool [OPTIONS]

"#]],
        );

        let version = Error::DisplayVersion { version: "tool 1.2.3\n".to_owned() };
        assert_eq!(version.exit_code(), 0);
        assert_eq!(version.to_string(), "tool 1.2.3\n");

        let failure = Error::UnknownFlag { token: b"--bad".to_vec() };
        assert_eq!(failure.exit_code(), 2);
    }
}
