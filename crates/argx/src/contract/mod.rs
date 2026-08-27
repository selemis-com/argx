//! Machine-readable invocation and execution contracts.
//!
//! Discovery projects invocation structure directly from the same static command tables used by
//! parsing and help generation. A separate lazy semantic projection supplies Rust value types, and
//! explicit execution bindings attach success/error types to invocable command identities.
//! Discovery does not instantiate destination Rust types, parse `argv`, inspect environment values,
//! or infer an application's serialization format.
//! Public contract values are owned so callers can serialize or retain a discovery result without
//! depending on generated static tables.
//!
//! Shallow discovery includes the selected command in full and direct children as summaries. A
//! summary still exposes command identity, aliases, and invocability, but omits invocation and
//! execution detail. Recursive discovery expands the complete selected subtree. The shared type
//! table contains only definitions referenced by detailed nodes returned in that result.
//!
//! All serialized Argx contract documents carry [`crate::CONTRACT_VERSION`]. CLI contracts and
//! standalone [`crate::TypeContract`] values share one version because they use the same semantic
//! type model and evolve as one public contract surface. The representation is intentionally
//! sparse: optional empty collections and default-false argument properties are omitted where
//! absence is unambiguous, while command `invocable` remains
//! explicit. Each detailed command context also exposes the built-in terminal actions accepted in
//! that lexical scope. Positional multiplicity is expressed directly through `required` and
//! `variadic`; a named option's `type` is present exactly when each occurrence consumes one value,
//! while `repeatable` controls occurrence multiplicity. Compatibility guarantees are release-policy
//! concerns rather than inferred from Rust API compatibility.

use std::{error, fmt};

use serde::Serialize;

use crate::{
    error::display_bytes,
    type_contract::{TypeContractValue, TypeDefinition},
};

mod discovery;

pub(crate) use discovery::discover;

/// One contract discovery request.
///
/// A request selects a command path relative to the root. Discovery is shallow by default;
/// [`Self::recursive`] expands the complete selected subtree. Path segments may use canonical
/// command names or declared aliases.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ContractRequest {
    /// Child-command path relative to the root command.
    path: Vec<String>,
    /// Whether descendants below the selected command are returned in full.
    recursive: bool,
}

impl ContractRequest {
    /// Creates a shallow request for the root command.
    #[must_use]
    pub const fn root() -> Self {
        Self { path: Vec::new(), recursive: false }
    }

    /// Creates a shallow request for one command path relative to the root command.
    #[must_use]
    pub fn new<I, S>(path: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self { path: path.into_iter().map(Into::into).collect(), recursive: false }
    }

    /// Requests full recursive detail below the selected command.
    #[must_use]
    pub const fn recursive(mut self) -> Self {
        self.recursive = true;
        self
    }
}

/// One versioned Argx machine contract discovery result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    /// Serialized Argx contract protocol version; currently [`crate::CONTRACT_VERSION`].
    pub version: u32,
    /// Canonical root command name.
    pub root: String,
    /// Selected command and requested descendant discovery.
    pub command: CommandContract,
    /// Shared semantic Rust type definitions referenced by invocation and execution contracts.
    ///
    /// References resolve by zero-based index into this table. Definition names are descriptive
    /// and are not required to be unique. Omitted when the returned command detail references no
    /// named semantic types.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<TypeDefinition>,
}

impl Contract {
    /// Serializes this contract as compact JSON.
    ///
    /// # Examples
    ///
    /// ```
    /// use argx::{ContractRequest, Parser as _};
    ///
    /// #[derive(argx::Parser)]
    /// struct Cli {
    ///     #[argx(long)]
    ///     verbose: bool,
    /// }
    ///
    /// #[derive(argx::Contract)]
    /// struct RunOutput {
    ///     verbose: bool,
    /// }
    ///
    /// #[derive(argx::Contract)]
    /// enum RunError {
    ///     Failed,
    /// }
    ///
    /// #[argx::contract(Cli)]
    /// fn run(cli: Cli) -> Result<RunOutput, RunError> {
    ///     Ok(RunOutput { verbose: cli.verbose })
    /// }
    ///
    /// let contract = Cli::contract(ContractRequest::root()).unwrap();
    /// let json = contract.to_json().unwrap();
    /// assert_eq!(contract.version, argx::CONTRACT_VERSION);
    /// assert!(json.contains(r#""options""#));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes this contract as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// One command node in a discovered Argx contract tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandContract {
    /// Canonical child-command path relative to the root command.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
    /// Canonical command name.
    pub name: String,
    /// One-line command description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    /// Hidden command aliases accepted for dynamic lookup and argv parsing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Whether this command can be invoked without selecting another subcommand.
    pub invocable: bool,
    /// Root-to-selected command contexts when this node is included in full.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation: Option<Vec<CommandContextContract>>,
    /// Declared semantic execution result when this invocable node is included in full.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionContract>,
    /// Discovered child commands.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subcommands: Vec<Self>,
}

/// Semantic execution result contract for one directly invocable command.
///
/// The success and error values describe the Rust types in the bound handler's concrete
/// `Result<Success, Error>`. They do not describe a transport encoding or inspect `serde`
/// attributes. A [`TypeContractValue::Unit`] branch explicitly means that outcome carries no
/// semantic payload; it is not an unknown or unspecified result. An absent
/// [`CommandContract::execution`] instead means that execution detail was not included for that
/// command node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionContract {
    /// Semantic Rust type returned when command execution succeeds.
    pub success: TypeContractValue,
    /// Semantic Rust type returned when command execution fails.
    pub error: TypeContractValue,
}

/// Arguments and relationships owned by one command context on an invocation path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandContextContract {
    /// Canonical child-command path relative to the root command.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
    /// Built-in terminal actions accepted in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionContract>,
    /// Positional arguments accepted in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub positionals: Vec<PositionalContract>,
    /// Named options accepted in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionContract>,
    /// Relationships between arguments and options in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<ConstraintContract>,
}

/// One built-in terminal action accepted in a command context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionContract {
    /// Preferred canonical command-line spelling, including its leading dash characters.
    pub name: String,
    /// Other accepted spellings, including their leading dash characters.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Terminal behavior triggered by this action.
    pub kind: ActionContractKind,
}

/// Built-in terminal behaviors exposed through machine invocation contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionContractKind {
    /// Render generated help for this command scope.
    Help,
    /// Render configured version information for this command scope.
    Version,
}

/// One positional argument in a machine invocation contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionalContract {
    /// Semantic positional name.
    pub name: String,
    /// One-line argument description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Whether this positional must resolve at least one value.
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
    /// Whether this positional consumes every remaining positional value in its command context.
    #[serde(skip_serializing_if = "is_false")]
    pub variadic: bool,
    /// Semantic Rust type produced from each consumed value.
    ///
    /// This describes the bound Rust value, not a custom `FromStr` or OS-string lexical encoding.
    #[serde(rename = "type")]
    pub value_type: TypeContractValue,
    /// Whether negative numbers may bind while ordinary flag parsing remains enabled.
    #[serde(skip_serializing_if = "is_false")]
    pub allow_negative_numbers: bool,
}

/// One named option in a machine invocation contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionContract {
    /// Preferred canonical command-line spelling, including its leading dashes.
    pub name: String,
    /// Other accepted spellings, including their leading dashes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// One-line option description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Whether this option remains in scope after entering descendants.
    #[serde(skip_serializing_if = "is_false")]
    pub global: bool,
    /// Whether this option must resolve from argv or environment when no typed default exists.
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
    /// Semantic Rust type consumed by each occurrence; omitted for value-less switches.
    ///
    /// Argx named options consume exactly one value per occurrence when this field is present. The
    /// field describes the bound Rust value, not a custom `FromStr` or OS-string lexical encoding.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub value_type: Option<TypeContractValue>,
    /// Whether the option may occur more than once.
    #[serde(skip_serializing_if = "is_false")]
    pub repeatable: bool,
    /// Environment variable consulted after argv when configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Whether absence is satisfied by a typed Rust default expression.
    #[serde(skip_serializing_if = "is_false")]
    pub has_default: bool,
    /// Whether detached values may themselves be flag-like.
    #[serde(skip_serializing_if = "is_false")]
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may bind while ordinary flag-like values remain rejected.
    #[serde(skip_serializing_if = "is_false")]
    pub allow_negative_numbers: bool,
}

/// One normalized relationship between arguments in the same command context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintContract {
    /// Relationship behavior.
    pub kind: ConstraintContractKind,
    /// Public argument or option name declaring the relationship.
    pub source: String,
    /// Public argument or option name referenced by the relationship.
    pub target: String,
}

/// Supported machine-contract argument relationship kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintContractKind {
    /// Supplying the source requires the target to resolve a value.
    Requires,
    /// Supplying both source and target is invalid.
    Conflicts,
}

/// Returns whether a boolean value is false.
const fn is_false(value: &bool) -> bool {
    !*value
}

/// Failure to resolve one dynamic contract discovery request.
///
/// Requested path segments are preserved in the error value. [`std::fmt::Display`] escapes control
/// characters before rendering caller-provided segments for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContractError {
    /// One requested child-command spelling does not exist in its parent scope.
    UnknownCommand {
        /// Canonical path resolved successfully before the failing segment.
        path: Vec<String>,
        /// Requested canonical name or alias that did not resolve.
        segment: String,
    },
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand { path, segment } => {
                if path.is_empty() {
                    write!(
                        formatter,
                        "unknown contract command `{}`",
                        display_bytes(segment.as_bytes()),
                    )
                } else {
                    write!(
                        formatter,
                        "unknown contract command `{}` below `{}`",
                        display_bytes(segment.as_bytes()),
                        path.join(" "),
                    )
                }
            }
        }
    }
}

impl error::Error for ContractError {}
