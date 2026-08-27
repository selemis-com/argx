//! Dynamic shell completion driven by Argx's static command model.
//!
//! Generated shell adapters send the cursor state back to the executable. Bash, Fish, and Zsh
//! send the command line through the cursor, while Nushell sends the tokenized spans it already
//! provides to external completers. Argx normalizes either transport, walks completed argv words
//! through the same raw parser used for normal invocation, and returns candidates for the cursor
//! position. This keeps command, option, value, scope, alias, and lexical behavior in one
//! implementation rather than reproducing the parser in shell code.
//!
//! Completion currently targets Bash, Fish, Nushell, and Zsh. Fields declared with
//! `#[argx(value_enum)]` complete from the same canonical finite vocabulary used by parsing, help,
//! and contracts. Argx intentionally does not infer possible values from arbitrary `FromStr`
//! implementations, enumerate filesystem paths, or expose custom value completers.
//!
//! Conflict suppression is likewise intentionally lexical: arguments already supplied on argv are
//! considered, while environment fallbacks and typed defaults remain the binding layer's concern.
//! Because the shell invokes the application for each completion request, applications should let
//! [`crate::Parser::parse`] or [`crate::Parser::handle_completion`] run before expensive startup or
//! writing application output.

mod engine;
mod script;

use std::{fmt, str::FromStr};

pub(crate) use engine::handle_process;

use crate::type_contract::{
    TypeContractValue, TypeDefinitionKind, TypeVariantContract, TypeVariantKind,
    resolve::{TypeContractSource, TypeKey, TypeResolver},
};

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

impl TypeContractSource for Shell {
    fn resolve_type(resolver: &mut TypeResolver) -> TypeContractValue {
        resolver.named(
            Self::type_key(),
            "Shell",
            Some(
                "Shell completion target. CLI spellings are `bash`, `fish`, `nushell`, and `zsh`.",
            ),
            |_resolver| TypeDefinitionKind::Enum {
                variants: [
                    ("Bash", "GNU Bash."),
                    ("Fish", "Fish shell."),
                    ("Nushell", "Nushell."),
                    ("Zsh", "Z shell."),
                ]
                .into_iter()
                .map(|(name, description)| TypeVariantContract {
                    name: name.to_owned(),
                    description: Some(description.to_owned()),
                    kind: TypeVariantKind::Unit,
                })
                .collect(),
            },
        )
    }

    fn type_key() -> TypeKey {
        TypeKey::new::<Self>(Vec::new(), Vec::new())
    }
}

impl crate::ContractType for Shell {}

/// Failure while generating a shell completion adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScriptError {
    /// The executable name cannot be represented safely as one shell command word.
    InvalidCommandName {
        /// Rejected executable name.
        name: String,
    },
}

impl fmt::Display for ScriptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCommandName { name } => write!(
                formatter,
                "cannot generate completions for `{name}`: the command name must be one plain shell word, must not start with `-`, and may contain only ASCII letters, digits, `-`, `_`, `.`, or `+`",
            ),
        }
    }
}

impl std::error::Error for ScriptError {}

/// Generates a dynamic completion adapter for `command` and `shell`.
///
/// The returned script calls `command` during completion and lets Argx answer from the same static
/// command metadata used by normal parsing. The command name is intentionally explicit so callers
/// can generate completions for the installed executable name even when it differs from a parser
/// type's configured root name.
///
/// # Errors
///
/// Returns [`ScriptError::InvalidCommandName`] when `command` cannot be safely registered and
/// invoked as one command word by all supported shell adapters.
pub fn script(command: &str, shell: Shell) -> Result<String, ScriptError> {
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

    #[test]
    fn shell_has_a_semantic_contract() {
        let contract = <Shell as crate::ContractType>::type_contract();
        assert_eq!(contract.types.len(), 1);
        assert_eq!(contract.types[0].name, "Shell");
        assert_eq!(
            contract.types[0].description.as_deref(),
            Some(
                "Shell completion target. CLI spellings are `bash`, `fish`, `nushell`, and `zsh`.",
            ),
        );
    }
}
