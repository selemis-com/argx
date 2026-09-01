//! Parser actions and errors.
//!
//! The `try_parse*` APIs return built-in help, version, schema, and completion actions through
//! [`Error`] along with ordinary parsing failures. [`Error::exit`] applies Argx's normal terminal
//! and exit-code behavior.

use std::{
    borrow::Cow,
    io::{self, IsTerminal as _, Write as _},
    process,
};

/// A built-in parser action or command-line parsing failure.
///
/// Help, version, schema, and completion requests are represented alongside ordinary parsing
/// errors so callers of the `try_parse*` methods can choose how to handle them.
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
    /// Dynamic shell-completion output requested through Argx's private completion protocol.
    #[error("{completion}")]
    DisplayCompletion {
        /// Completion candidates encoded for the generated shell adapter.
        completion: String,
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
        /// Finite values accepted by this option, when known.
        possible_values: &'static [&'static str],
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
        /// Corrective usage for the selected command scope, when available.
        usage: Option<String>,
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
        /// Corrective usage for the selected command scope, when available.
        usage: Option<String>,
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
    /// A `one_of` set had zero or multiple explicitly supplied members.
    #[error("exactly one of {arguments} must be provided")]
    InvalidOneOf {
        /// Comma-separated canonical labels of the participating arguments.
        arguments: String,
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
            Self::DisplayHelp { .. }
            | Self::DisplayVersion { .. }
            | Self::DisplaySchema { .. }
            | Self::DisplayCompletion { .. } => 0,
            _ => 2,
        }
    }

    /// Prints this result to the appropriate stream and terminates the process.
    ///
    /// Help, version, schema, and completion requests are written to standard output and exit
    /// successfully.
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
            | Self::DisplaySchema { schema: text }
            | Self::DisplayCompletion { completion: text } => ExitOutput {
                stream: ExitStream::Stdout,
                text: Cow::Borrowed(text.as_str()),
                code: self.exit_code(),
            },
            _ => ExitOutput {
                stream: ExitStream::Stderr,
                text: Cow::Owned(render_diagnostic(self, diagnostic_styling_enabled())),
                code: self.exit_code(),
            },
        }
    }
}

/// Whether interactive diagnostics should use minimal ANSI emphasis.
fn diagnostic_styling_enabled() -> bool {
    io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Renders one parser diagnostic using the terminal styling policy selected by the caller.
fn render_diagnostic(error: &Error, styled: bool) -> String {
    let error_label = emphasize("error:", styled, false);
    let help = emphasize("--help", styled, false);

    if let Error::MissingRequiredArguments { argument, usage } = error {
        let usage_label = emphasize("Usage:", styled, true);
        let usage = if styled { style_usage_command(usage) } else { usage.clone() };
        return format!(
            "{error_label} the following required arguments were not provided:\n  {argument}\n\n{usage_label} {usage}\n\nFor more information, try '{help}'.\n"
        );
    }

    let message = if styled { styled_diagnostic_message(error) } else { error.to_string() };
    if let Error::MissingValue { possible_values, .. } = error
        && !possible_values.is_empty()
    {
        return format!(
            "{error_label} {message}\n\nPossible values:\n{}\nFor more information, try '{help}'.\n",
            render_possible_values(possible_values),
        );
    }
    if let Some(usage) = structural_usage(error) {
        let usage_label = emphasize("Usage:", styled, true);
        let usage = if styled { style_usage_command(usage) } else { usage.to_owned() };
        return format!(
            "{error_label} {message}\n\n{usage_label} {usage}\n\nFor more information, try '{help}'.\n"
        );
    }
    format!("{error_label} {message}\n\nFor more information, try '{help}'.\n")
}

/// Renders finite values for a missing-value diagnostic.
fn render_possible_values(values: &[&str]) -> String {
    let mut rendered = String::new();
    for value in values {
        rendered.push_str("  - ");
        rendered.push_str(&display_bytes(value.as_bytes()));
        rendered.push('\n');
    }
    rendered
}

/// Returns corrective usage for diagnostics caused by command structure.
fn structural_usage(error: &Error) -> Option<&str> {
    match error {
        Error::UnexpectedArgument { usage, .. } | Error::DuplicateArgument { usage, .. } => {
            usage.as_deref()
        }
        _ => None,
    }
}

/// Renders one diagnostic body, emphasizing only user-facing CLI tokens.
fn styled_diagnostic_message(error: &Error) -> String {
    match error {
        Error::UnknownFlag { token } => {
            format!("unknown flag `{}`", emphasize(&display_bytes(token), true, false))
        }
        Error::MissingValue { name, .. } => {
            format!("missing value for `{}`", emphasize(name, true, false))
        }
        Error::UnexpectedValue { name } => {
            format!("`{}` does not accept a value", emphasize(name, true, false))
        }
        Error::UnexpectedArgument { token, .. } => {
            format!("unexpected argument `{}`", emphasize(&display_bytes(token), true, false))
        }
        Error::UnknownCommand { token } => {
            format!("unknown command `{}`", emphasize(&display_bytes(token), true, false))
        }
        Error::MissingSubcommand { name } => {
            format!("required subcommand `{}` was not provided", emphasize(name, true, false))
        }
        Error::MissingRequired { name } => {
            format!("required argument `{}` was not provided", emphasize(name, true, false))
        }
        Error::DuplicateArgument { name, .. } => {
            format!("argument `{}` cannot be used more than once", emphasize(name, true, false))
        }
        Error::MissingRequirement { name, required_by } => format!(
            "argument `{}` is required when `{}` is used",
            emphasize(name, true, false),
            emphasize(required_by, true, false)
        ),
        Error::ConflictingArguments { name, other } => format!(
            "argument `{}` cannot be used with `{}`",
            emphasize(name, true, false),
            emphasize(other, true, false)
        ),
        Error::InvalidOneOf { arguments } => {
            format!("exactly one of {arguments} must be provided")
        }
        Error::InvalidUtf8 { name, value } => format!(
            "value `{}` for `{}` is not valid UTF-8",
            emphasize(&display_bytes(value), true, false),
            emphasize(name, true, false)
        ),
        Error::InvalidValue { name, value, reason } => format!(
            "invalid value `{}` for `{}`: {}",
            emphasize(&display_bytes(value.as_bytes()), true, false),
            emphasize(name, true, false),
            display_bytes(reason.as_bytes())
        ),
        _ => error.to_string(),
    }
}

/// Applies optional bold or bold-and-underlined terminal emphasis.
fn emphasize(value: &str, styled: bool, underline: bool) -> Cow<'_, str> {
    if !styled {
        return Cow::Borrowed(value);
    }
    let code = if underline { "1;4" } else { "1" };
    Cow::Owned(format!("\x1b[{code}m{value}\x1b[0m"))
}

/// Emphasizes the command prefix while leaving usage arguments unstyled.
fn style_usage_command(usage: &str) -> String {
    let boundary = usage
        .find(" --")
        .into_iter()
        .chain(usage.find(" <"))
        .chain(usage.find(" ["))
        .min()
        .unwrap_or(usage.len());
    let (command, arguments) = usage.split_at(boundary);
    format!("{}{arguments}", emphasize(command, true, false))
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

        let completion = Error::DisplayCompletion { completion: "--help\tPrint help\n".to_owned() };
        let output = completion.exit_output();
        assert_eq!(output.stream, ExitStream::Stdout);
        assert_eq!(output.code, 0);
        assert_eq!(output.text, "--help\tPrint help\n");

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

        let completion = Error::DisplayCompletion { completion: "candidate\n".to_owned() };
        assert_eq!(completion.exit_code(), 0);
        assert_eq!(completion.to_string(), "candidate\n");

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
            Error::MissingValue { name: "--output", possible_values: &[] }.to_string(),
            "missing value for `--output`",
        );
        assert_eq!(
            Error::UnexpectedValue { name: "--verbose" }.to_string(),
            "`--verbose` does not accept a value",
        );
        assert_eq!(
            Error::UnexpectedArgument { token: b"extra".to_vec(), usage: None }.to_string(),
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
            Error::DuplicateArgument { name: "--verbose", usage: None }.to_string(),
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
    fn styled_diagnostics_emphasize_error_tokens_and_help_hint() {
        let rendered = render_diagnostic(&Error::UnknownFlag { token: b"--wat".to_vec() }, true);
        assert_eq!(
            rendered,
            "\x1b[1merror:\x1b[0m unknown flag `\x1b[1m--wat\x1b[0m`\n\nFor more information, try '\x1b[1m--help\x1b[0m'.\n",
        );

        let rendered = render_diagnostic(
            &Error::InvalidValue {
                name: "<VALUE>",
                value: String::from("invalid"),
                reason: String::from("invalid value"),
            },
            true,
        );
        assert_eq!(
            rendered,
            "\x1b[1merror:\x1b[0m invalid value `\x1b[1minvalid\x1b[0m` for `\x1b[1m<VALUE>\x1b[0m`: invalid value\n\nFor more information, try '\x1b[1m--help\x1b[0m'.\n",
        );
    }

    #[test]
    fn styled_missing_required_diagnostic_emphasizes_usage_structure() {
        let rendered = render_diagnostic(
            &Error::MissingRequiredArguments {
                argument: String::from("--required <REQUIRED>"),
                usage: String::from("cli command --required <REQUIRED> --optional <OPTIONAL>"),
            },
            true,
        );
        assert_eq!(
            rendered,
            "\x1b[1merror:\x1b[0m the following required arguments were not provided:\n  --required <REQUIRED>\n\n\x1b[1;4mUsage:\x1b[0m \x1b[1mcli command\x1b[0m --required <REQUIRED> --optional <OPTIONAL>\n\nFor more information, try '\x1b[1m--help\x1b[0m'.\n",
        );
    }

    #[test]
    fn missing_value_diagnostic_lists_known_values() {
        let rendered = render_diagnostic(
            &Error::MissingValue { name: "--output", possible_values: &["text", "json"] },
            false,
        );
        assert_eq!(
            rendered,
            "error: missing value for `--output`\n\nPossible values:\n  - text\n  - json\n\nFor more information, try '--help'.\n",
        );
    }

    #[test]
    fn diagnostic_renderer_keeps_plain_output_free_of_terminal_controls() {
        let rendered = render_diagnostic(
            &Error::MissingValue { name: "--limit", possible_values: &[] },
            false,
        );
        assert_eq!(
            rendered,
            "error: missing value for `--limit`\n\nFor more information, try '--help'.\n",
        );
        assert!(!rendered.contains('\x1b'));
    }

    #[test]
    fn structural_diagnostics_include_corrective_usage() {
        let unexpected = Error::UnexpectedArgument {
            token: b"extra".to_vec(),
            usage: Some(String::from("cli get <ID>")),
        };
        assert_eq!(
            render_diagnostic(&unexpected, false),
            "error: unexpected argument `extra`\n\nUsage: cli get <ID>\n\nFor more information, try '--help'.\n",
        );

        let duplicate = Error::DuplicateArgument {
            name: "--limit",
            usage: Some(String::from("cli list [OPTIONS] <ID>")),
        };
        assert_eq!(
            render_diagnostic(&duplicate, false),
            "error: argument `--limit` cannot be used more than once\n\nUsage: cli list [OPTIONS] <ID>\n\nFor more information, try '--help'.\n",
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
