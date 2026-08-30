//! Parser actions and errors.
//!
//! The `try_parse*` APIs return built-in help, version, and schema actions through [`Error`] along
//! with ordinary parsing failures. [`Error::exit`] applies Argx's normal terminal and exit-code
//! behavior.

use std::{
    borrow::Cow,
    io::{self, Write as _},
    process,
};

/// A built-in parser action or command-line parsing failure.
///
/// Help, version, and schema requests are represented alongside ordinary parsing errors so callers
/// of the `try_parse*` methods can choose how to handle them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Generated help requested through the built-in help switch.
    #[error("{help}")]
    DisplayHelp {
        /// Fully rendered help text for the selected command scope.
        help: String,
    },
    /// Version information requested through the built-in version action.
    #[error("{version}")]
    DisplayVersion {
        /// Fully rendered version text for the selected command scope.
        version: String,
    },
    /// Machine-readable command schema requested through built-in discovery.
    #[error("{schema}")]
    DisplaySchema {
        /// Pretty-printed JSON schema document for the selected command scope.
        schema: String,
    },
    /// A flag-like token did not match any declared flag.
    #[error("unknown flag `{}`", display_bytes(.token))]
    UnknownFlag {
        /// Encoded token supplied by the caller.
        token: Vec<u8>,
    },
    /// A flag that consumes a value did not receive one.
    #[error("missing value for `{name}`")]
    MissingValue {
        /// Canonical user-facing argument label.
        name: &'static str,
    },
    /// A value was attached to a switch that does not accept one.
    #[error("`{name}` does not accept a value")]
    UnexpectedValue {
        /// Canonical user-facing argument label.
        name: &'static str,
    },
    /// A word could not be assigned to any positional argument.
    #[error("unexpected argument `{}`", display_bytes(.token))]
    UnexpectedArgument {
        /// Encoded token supplied by the caller.
        token: Vec<u8>,
    },
    /// A word did not match any child command when command selection was required.
    #[error("unknown command `{}`", display_bytes(.token))]
    UnknownCommand {
        /// Encoded token supplied by the caller.
        token: Vec<u8>,
    },
    /// A required subcommand field was not selected.
    #[error("required subcommand `{name}` was not provided")]
    MissingSubcommand {
        /// Canonical field name.
        name: &'static str,
    },
    /// A required field was not supplied during generated binding.
    #[error("required argument `{name}` was not provided")]
    MissingRequired {
        /// Canonical user-facing argument label.
        name: &'static str,
    },
    /// One or more required arguments were missing from a parsed command invocation.
    #[error("the following required arguments were not provided:\n  {argument}\n\nUsage: {usage}")]
    MissingRequiredArguments {
        /// Canonical CLI rendering of the missing argument.
        argument: String,
        /// Usage for the selected command scope.
        usage: String,
    },
    /// A scalar argument was supplied more than once.
    #[error("argument `{name}` cannot be used more than once")]
    DuplicateArgument {
        /// Canonical user-facing argument label.
        name: &'static str,
    },
    /// An argument required by another supplied argument was not available.
    #[error("argument `{name}` is required when `{required_by}` is used")]
    MissingRequirement {
        /// Canonical user-facing label of the missing argument.
        name: &'static str,
        /// Canonical user-facing label of the argument imposing the requirement.
        required_by: &'static str,
    },
    /// Two arguments declared to conflict were supplied together.
    #[error("argument `{name}` cannot be used with `{other}`")]
    ConflictingArguments {
        /// Canonical user-facing label of the argument declaring the conflict.
        name: &'static str,
        /// Canonical user-facing label of the conflicting argument.
        other: &'static str,
    },
    /// A text value was not valid UTF-8.
    #[error("value `{}` for `{name}` is not valid UTF-8", display_bytes(.value))]
    InvalidUtf8 {
        /// Canonical user-facing argument label.
        name: &'static str,
        /// Encoded value supplied by the caller.
        value: Vec<u8>,
    },
    /// A UTF-8 value could not be converted to the field's Rust type.
    #[error(
        "invalid value `{}` for `{name}`: {}",
        display_bytes(.value.as_bytes()),
        display_bytes(.reason.as_bytes())
    )]
    InvalidValue {
        /// Canonical user-facing argument label.
        name: &'static str,
        /// Text supplied on argv.
        value: String,
        /// Conversion failure reported by the target type.
        reason: String,
    },
}

impl Error {
    /// Returns the process status used by [`Self::exit`].
    ///
    /// # Examples
    ///
    /// ```
    /// let help = argx::Error::DisplayHelp { help: String::new() };
    /// let failure = argx::Error::UnknownFlag { token: b"--bad".to_vec() };
    ///
    /// assert_eq!(help.exit_code(), 0);
    /// assert_eq!(failure.exit_code(), 2);
    /// ```
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::DisplayHelp { .. } | Self::DisplayVersion { .. } | Self::DisplaySchema { .. } => {
                0
            }
            _ => 2,
        }
    }

    /// Prints this result to the appropriate stream and terminates the process.
    ///
    /// Help, version, and schema requests are written to standard output and exit successfully.
    /// Parse and binding failures are written to standard error and exit with status 2.
    pub fn exit(&self) -> ! {
        let output = self.exit_output();
        match output.stream {
            ExitStream::Stdout => {
                let mut stdout = io::stdout().lock();
                let _ = stdout.write_all(output.text.as_bytes());
                let _ = stdout.flush();
            }
            ExitStream::Stderr => {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(output.text.as_bytes());
                let _ = stderr.flush();
            }
        }
        process::exit(output.code)
    }

    /// Builds the exact terminal output used by [`Self::exit`].
    ///
    /// Keeping rendering separate from the process boundary lets unit tests pin diagnostic bytes
    /// without spawning a child process or duplicating the production formatter.
    fn exit_output(&self) -> ExitOutput<'_> {
        match self {
            Self::DisplayHelp { help: text }
            | Self::DisplayVersion { version: text }
            | Self::DisplaySchema { schema: text } => ExitOutput {
                stream: ExitStream::Stdout,
                text: Cow::Borrowed(text.as_str()),
                code: self.exit_code(),
            },
            Self::MissingRequiredArguments { argument, usage } => ExitOutput {
                stream: ExitStream::Stderr,
                text: Cow::Owned(format!(
                    "error: the following required arguments were not provided:\n  {argument}\n\nUsage: {usage}\n\nFor more information, try '--help'.\n"
                )),
                code: self.exit_code(),
            },
            _ => ExitOutput {
                stream: ExitStream::Stderr,
                text: Cow::Owned(format!("error: {self}\n\nFor more information, try '--help'.\n")),
                code: self.exit_code(),
            },
        }
    }
}

/// Terminal stream selected by the conventional CLI exit policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitStream {
    /// Successful terminal actions are written to standard output.
    Stdout,
    /// Parse and binding failures are written to standard error.
    Stderr,
}

/// Fully rendered process-boundary output for one parser result.
struct ExitOutput<'a> {
    /// Destination stream.
    stream: ExitStream,
    /// Exact bytes represented as UTF-8 text.
    text: Cow<'a, str>,
    /// Process status to use after writing the output.
    code: i32,
}

/// Lossily renders encoded argument bytes while escaping terminal control characters.
pub(crate) fn display_bytes(value: &[u8]) -> String {
    String::from_utf8_lossy(value).escape_debug().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_bytes_do_not_emit_control_characters() {
        let rendered = display_bytes(b"--bad\n\x1b[31m");
        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\x1b'));
        assert!(rendered.contains(r"\n"));
    }

    #[test]
    fn exit_output_uses_the_same_renderer_for_success_and_failure_policy() {
        let help = Error::DisplayHelp { help: "Usage: tool [OPTIONS]\n".to_owned() };
        let output = help.exit_output();
        assert_eq!(output.stream, ExitStream::Stdout);
        assert_eq!(output.code, 0);
        assert_eq!(output.text, "Usage: tool [OPTIONS]\n");

        let version = Error::DisplayVersion { version: "tool 1.2.3\n".to_owned() };
        let output = version.exit_output();
        assert_eq!(output.stream, ExitStream::Stdout);
        assert_eq!(output.code, 0);
        assert_eq!(output.text, "tool 1.2.3\n");

        let schema = Error::DisplaySchema { schema: "{\"command\":{}}\n".to_owned() };
        let output = schema.exit_output();
        assert_eq!(output.stream, ExitStream::Stdout);
        assert_eq!(output.code, 0);
        assert_eq!(output.text, "{\"command\":{}}\n");

        let failure = Error::UnknownFlag { token: b"--bad\nflag".to_vec() };
        let output = failure.exit_output();
        assert_eq!(output.stream, ExitStream::Stderr);
        assert_eq!(output.code, 2);
        assert_eq!(
            output.text,
            "error: unknown flag `--bad\\nflag`\n\nFor more information, try '--help'.\n",
        );
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

        let schema = Error::DisplaySchema { schema: "{}\n".to_owned() };
        assert_eq!(schema.exit_code(), 0);
        assert_eq!(schema.to_string(), "{}\n");

        let failure = Error::UnknownFlag { token: b"--bad".to_vec() };
        assert_eq!(failure.exit_code(), 2);
    }

    #[test]
    fn syntax_and_cardinality_errors_render_actionable_diagnostics() {
        assert_eq!(
            Error::UnknownFlag { token: b"--bad\nflag".to_vec() }.to_string(),
            r"unknown flag `--bad\nflag`",
        );
        assert_eq!(
            Error::MissingValue { name: "--output" }.to_string(),
            "missing value for `--output`",
        );
        assert_eq!(
            Error::UnexpectedValue { name: "--verbose" }.to_string(),
            "`--verbose` does not accept a value",
        );
        assert_eq!(
            Error::UnexpectedArgument { token: b"extra".to_vec() }.to_string(),
            "unexpected argument `extra`",
        );
        assert_eq!(
            Error::UnknownCommand { token: b"deploy".to_vec() }.to_string(),
            "unknown command `deploy`",
        );
        assert_eq!(
            Error::MissingSubcommand { name: "command" }.to_string(),
            "required subcommand `command` was not provided",
        );
        assert_eq!(
            Error::MissingRequired { name: "--output" }.to_string(),
            "required argument `--output` was not provided",
        );
        assert_eq!(
            Error::MissingRequiredArguments {
                argument: String::from("--output <OUTPUT>"),
                usage: String::from("tool [OPTIONS] --output <OUTPUT>"),
            }
            .to_string(),
            "the following required arguments were not provided:\n  --output <OUTPUT>\n\nUsage: tool [OPTIONS] --output <OUTPUT>",
        );
        assert_eq!(
            Error::DuplicateArgument { name: "--verbose" }.to_string(),
            "argument `--verbose` cannot be used more than once",
        );
    }

    #[test]
    fn relationship_errors_name_both_participating_arguments() {
        assert_eq!(
            Error::MissingRequirement { name: "--token", required_by: "--endpoint" }.to_string(),
            "argument `--token` is required when `--endpoint` is used",
        );
        assert_eq!(
            Error::ConflictingArguments { name: "--output", other: "--stdout" }.to_string(),
            "argument `--output` cannot be used with `--stdout`",
        );
    }

    #[test]
    fn conversion_errors_escape_values_and_reasons() {
        assert_eq!(
            Error::InvalidUtf8 { name: "input", value: b"bad\nvalue".to_vec() }.to_string(),
            r"value `bad\nvalue` for `input` is not valid UTF-8",
        );
        assert_eq!(
            Error::InvalidValue {
                name: "--port",
                value: String::from("bad\nvalue"),
                reason: String::from("invalid\nnumber"),
            }
            .to_string(),
            r"invalid value `bad\nvalue` for `--port`: invalid\nnumber",
        );
    }
}
