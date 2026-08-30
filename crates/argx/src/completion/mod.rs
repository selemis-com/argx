//! Dynamic shell completion for Bash, Fish, Nushell, and Zsh.
//!
//! Generate an adapter with [`crate::Parser::render_completion`]. The generated
//! adapter asks the running application for candidates, so completions stay in sync with the
//! derived command interface. Fields marked `#[argx(value_enum)]` complete from their declared
//! finite values.
//!
//! [`crate::Parser::parse`] and [`crate::Parser::try_parse`] handle completion requests from the
//! current process automatically. Explicit-argv entry points such as [`crate::Parser::parse_from`]
//! and [`crate::Parser::try_parse_from`] do not inspect this process-level protocol.

mod engine;
mod script;

use std::{fmt, str::FromStr};

pub(crate) use engine::process_request;

/// Environment marker used only by generated completion adapters.
const PROTOCOL_ENV: &str = "ARGX_COMPLETE";
/// Current private completion-protocol version.
const PROTOCOL_VERSION: &str = "1";
/// Private argv marker used after the protocol environment variable selects completion mode.
const PROTOCOL_COMMAND: &str = "__argx_complete__";
/// Environment variable carrying the shell command line through the cursor.
const PROTOCOL_LINE_ENV: &str = "ARGX_COMPLETE_LINE";
/// Environment variable carrying Nushell's already-tokenized completion spans as JSON.
const PROTOCOL_WORDS_ENV: &str = "ARGX_COMPLETE_WORDS";

/// Shells for which Argx can generate dynamic completion adapters.
///
/// [`FromStr`] accepts the canonical lower-case spellings `bash`, `fish`, `nushell`, and `zsh`.
/// The type also implements [`crate::ValueEnum`], so a CLI field can opt into that vocabulary
/// with `#[argx(value_enum)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Shell {
    /// GNU Bash.
    Bash,
    /// Fish shell.
    Fish,
    /// Nushell.
    Nushell,
    /// Z shell.
    Zsh,
}

impl Shell {
    /// Returns the canonical lower-case shell spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Fish => "fish",
            Self::Nushell => "nushell",
            Self::Zsh => "zsh",
        }
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl crate::ValueEnum for Shell {
    const VALUES: &'static [&'static str] = &["bash", "fish", "nushell", "zsh"];

    fn from_value(value: &str) -> Option<Self> {
        match value {
            "bash" => Some(Self::Bash),
            "fish" => Some(Self::Fish),
            "nushell" => Some(Self::Nushell),
            "zsh" => Some(Self::Zsh),
            _ => None,
        }
    }
}

impl FromStr for Shell {
    type Err = crate::ValueEnumError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        <Self as crate::ValueEnum>::from_value(value)
            .ok_or_else(|| crate::ValueEnumError::new(<Self as crate::ValueEnum>::VALUES))
    }
}

/// Failure while generating a shell completion adapter.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ScriptError {
    /// The executable name cannot be represented safely as one shell command word.
    #[error(
        "cannot generate completions for `{name}`: the command name must be one plain shell word, must not start with `-`, and may contain only ASCII letters, digits, `-`, `_`, `.`, or `+`"
    )]
    InvalidCommandName {
        /// Rejected executable name.
        name: String,
    },
}

/// Generates a dynamic completion adapter for the given command name and shell.
pub(crate) fn script(command: &str, shell: Shell) -> Result<String, ScriptError> {
    script::render(command, shell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_names_parse_and_render() {
        assert_eq!("bash".parse(), Ok(Shell::Bash));
        assert_eq!(Shell::Fish.to_string(), "fish");
        assert_eq!("nushell".parse(), Ok(Shell::Nushell));
        assert_eq!(Shell::Nushell.to_string(), "nushell");
        assert_eq!(<Shell as crate::ValueEnum>::VALUES, &["bash", "fish", "nushell", "zsh"],);
        assert!("nu".parse::<Shell>().is_err());
    }
}
