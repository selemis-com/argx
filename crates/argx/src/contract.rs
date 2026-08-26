//! Stable machine-readable invocation contracts.
//!
//! The derive emits a contract projection alongside the runtime parser tables. Discovery walks that
//! static projection directly; it does not instantiate the destination Rust types, parse `argv`, or
//! inspect environment values. Public contract values are owned so callers can serialize or retain
//! a discovery result without depending on the generated static tables.
//!
//! The serialized representation is a versioned protocol. Additive Rust API changes therefore do
//! not implicitly permit wire-format changes: consumers should use [`CONTRACT_VERSION`] when
//! negotiating persisted or remote contract data.

use std::{error, fmt};

use serde::Serialize;

use crate::__private::{
    ArgSpec, Cardinality as StaticCardinality, CommandSpec, ConstraintKind, FlagSpec, Key,
};

/// Current serialized Argx contract protocol version.
pub const CONTRACT_VERSION: u32 = 1;

/// Controls how much descendant detail contract discovery returns.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ContractDepth {
    /// Include the selected command in full and its direct children as summaries.
    #[default]
    Shallow,
    /// Include the selected command and its complete descendant subtree in full.
    Recursive,
}

/// One contract discovery request.
///
/// A request selects a command path relative to the root and controls how deeply child commands
/// are expanded. Path segments may use canonical command names or declared aliases.
///
/// # Examples
///
/// ```
/// use argx::{ContractDepth, ContractRequest};
///
/// let request = ContractRequest::new(["admin", "users"]).recursive();
/// assert_eq!(request.path().len(), 2);
/// assert_eq!(request.path()[0], "admin");
/// assert_eq!(request.path()[1], "users");
/// assert_eq!(request.depth(), ContractDepth::Recursive);
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ContractRequest {
    /// Child-command path relative to the root command.
    path: Vec<String>,
    /// Descendant detail requested by the caller.
    depth: ContractDepth,
}

impl ContractRequest {
    /// Creates a shallow request for the root command.
    #[must_use]
    pub const fn root() -> Self {
        Self { path: Vec::new(), depth: ContractDepth::Shallow }
    }

    /// Creates a shallow request for one command path relative to the root command.
    #[must_use]
    pub fn new<I, S>(path: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self { path: path.into_iter().map(Into::into).collect(), depth: ContractDepth::Shallow }
    }

    /// Requests full recursive detail below the selected command.
    #[must_use]
    pub const fn recursive(mut self) -> Self {
        self.depth = ContractDepth::Recursive;
        self
    }

    /// Returns the requested command path relative to the root command.
    #[must_use]
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Returns the requested descendant detail level.
    #[must_use]
    pub const fn depth(&self) -> ContractDepth {
        self.depth
    }
}

/// One versioned Argx machine contract discovery result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    /// Serialized protocol version.
    pub version: u32,
    /// Canonical root command name.
    pub root: String,
    /// Selected command and requested descendant discovery.
    pub command: CommandContract,
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
    /// let contract = Cli::contract(ContractRequest::root()).unwrap();
    /// let json = contract.to_json().unwrap();
    /// assert!(json.contains(r#""version":1"#));
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
    /// Complete invocation detail when this node is included in full.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation: Option<InvocationContract>,
    /// Discovered child commands.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subcommands: Vec<Self>,
}

/// Complete argv-side contract for one selected command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvocationContract {
    /// Command contexts from the root through the selected command.
    pub contexts: Vec<CommandContextContract>,
}

/// Arguments and relationships owned by one command context on an invocation path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandContextContract {
    /// Canonical child-command path relative to the root command.
    pub path: Vec<String>,
    /// Positional arguments accepted in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<ArgumentContract>,
    /// Named options accepted in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionContract>,
    /// Relationships between arguments and options in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<ConstraintContract>,
}

/// One positional argument in a machine invocation contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentContract {
    /// Semantic positional name.
    pub name: String,
    /// One-line argument description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// One-based position within this command context's positional sequence.
    pub position: usize,
    /// Whether this positional must resolve at least one value.
    pub required: bool,
    /// Number of positional values represented by this argument.
    pub value: ValueContract,
    /// Whether negative numbers may bind while ordinary flag parsing remains enabled.
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
    pub global: bool,
    /// Whether this option must resolve from argv or environment when no typed default exists.
    pub required: bool,
    /// Value consumption for each occurrence, omitted for value-less switches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<ValueContract>,
    /// Whether the option may occur more than once.
    pub repeatable: bool,
    /// Environment variable consulted after argv when configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Whether absence is satisfied by a typed Rust default expression.
    pub has_default: bool,
    /// Whether detached values may themselves be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether negative numbers may bind while ordinary flag-like values remain rejected.
    pub allow_negative_numbers: bool,
}

/// Number of values represented by one positional or one option occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueContract {
    /// Minimum number of values.
    pub min_values: usize,
    /// Maximum number of values, or no upper bound when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_values: Option<usize>,
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

/// Failure to resolve one dynamic contract discovery request.
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
                    write!(formatter, "unknown contract command `{segment}`")
                } else {
                    write!(
                        formatter,
                        "unknown contract command `{segment}` below `{}`",
                        path.join(" "),
                    )
                }
            }
        }
    }
}

impl error::Error for ContractError {}

/// Builds one public discovery response from the generated private contract projection.
///
/// Alias resolution happens only while walking the requested path. Once selected, every path in
/// the returned value is rebuilt from canonical command names so aliases cannot leak into the wire
/// representation.
pub(crate) fn discover(
    root: &'static CommandSpec<'static>,
    request: ContractRequest,
) -> Result<Contract, ContractError> {
    let mut selected = root;
    let mut path = Vec::with_capacity(request.path.len());
    let mut contexts = vec![root];

    for segment in request.path {
        let Some(child) = selected.subcommands.iter().copied().find(|child| {
            child.name == segment.as_str() || child.aliases.contains(&segment.as_str())
        }) else {
            return Err(ContractError::UnknownCommand { path, segment });
        };
        path.push(child.name.to_owned());
        contexts.push(child);
        selected = child;
    }

    Ok(Contract {
        version: CONTRACT_VERSION,
        root: root.name.to_owned(),
        command: command_contract(selected, path, &contexts, request.depth, true),
    })
}

/// Builds one command node at the requested discovery depth.
fn command_contract(
    command: &'static CommandSpec<'static>,
    path: Vec<String>,
    contexts: &[&'static CommandSpec<'static>],
    depth: ContractDepth,
    detailed: bool,
) -> CommandContract {
    let invocation = detailed.then(|| InvocationContract {
        contexts: contexts
            .iter()
            .enumerate()
            .map(|(index, context)| command_context(context, &contexts[..=index]))
            .collect(),
    });

    let child_detailed = depth == ContractDepth::Recursive;
    let subcommands = if depth == ContractDepth::Shallow && !detailed {
        Vec::new()
    } else {
        command
            .subcommands
            .iter()
            .copied()
            .map(|child| {
                let mut child_path = path.clone();
                child_path.push(child.name.to_owned());
                let mut child_contexts = contexts.to_vec();
                child_contexts.push(child);
                command_contract(child, child_path, &child_contexts, depth, child_detailed)
            })
            .collect()
    };

    CommandContract {
        path,
        name: command.name.to_owned(),
        about: command.about.map(str::to_owned),
        aliases: command.aliases.iter().copied().map(str::to_owned).collect(),
        invocable: command.subcommands.is_empty(),
        invocation,
        subcommands,
    }
}

/// Builds one command-context contract and resolves semantic constraint keys to public names.
fn command_context(
    command: &'static CommandSpec<'static>,
    contexts: &[&'static CommandSpec<'static>],
) -> CommandContextContract {
    let path = contexts.iter().skip(1).map(|context| context.name.to_owned()).collect();
    let arguments =
        command.args.iter().enumerate().map(|(index, arg)| arg_contract(arg, index)).collect();
    let options = command.flags.iter().copied().map(option_contract).collect();
    let constraints = command
        .constraints
        .iter()
        .map(|constraint| ConstraintContract {
            kind: match constraint.kind {
                ConstraintKind::Requires => ConstraintContractKind::Requires,
                ConstraintKind::Conflicts => ConstraintContractKind::Conflicts,
            },
            source: argument_name(command, constraint.source),
            target: argument_name(command, constraint.target),
        })
        .collect();

    CommandContextContract { path, arguments, options, constraints }
}

/// Builds one named public option contract.
fn option_contract(flag: &FlagSpec<'_>) -> OptionContract {
    let name = preferred_option_name(flag);
    let mut aliases = Vec::with_capacity(flag.longs.len() + flag.aliases.len() + flag.shorts.len());
    aliases.extend(
        flag.longs.iter().map(|long| format!("--{long}")).filter(|spelling| spelling != &name),
    );
    aliases.extend(
        flag.aliases.iter().map(|alias| format!("--{alias}")).filter(|spelling| spelling != &name),
    );
    aliases.extend(
        flag.shorts
            .iter()
            .map(|short| format!("-{}", char::from(*short)))
            .filter(|spelling| spelling != &name),
    );

    OptionContract {
        name,
        aliases,
        help: flag.help.map(str::to_owned),
        global: flag.global,
        required: flag.required,
        value: option_value(flag.cardinality),
        repeatable: flag.cardinality == StaticCardinality::Many,
        environment: flag.env.map(str::to_owned),
        has_default: flag.has_default,
        allow_hyphen_values: flag.allow_hyphen_values,
        allow_negative_numbers: flag.allow_negative_numbers,
    }
}

/// Builds one positional public argument contract.
fn arg_contract(arg: &ArgSpec<'_>, index: usize) -> ArgumentContract {
    ArgumentContract {
        name: arg.name.to_owned(),
        help: arg.help.map(str::to_owned),
        position: index + 1,
        required: arg.required,
        value: positional_value(arg.cardinality, arg.required),
        allow_negative_numbers: arg.allow_negative_numbers,
    }
}

/// Returns value consumption for one named option occurrence.
const fn option_value(value: StaticCardinality) -> Option<ValueContract> {
    match value {
        StaticCardinality::Switch => None,
        StaticCardinality::One | StaticCardinality::Optional | StaticCardinality::Many => {
            Some(ValueContract { min_values: 1, max_values: Some(1) })
        }
    }
}

/// Returns total positional value multiplicity for one positional binding.
fn positional_value(value: StaticCardinality, required: bool) -> ValueContract {
    match value {
        StaticCardinality::One => ValueContract { min_values: 1, max_values: Some(1) },
        StaticCardinality::Optional => ValueContract { min_values: 0, max_values: Some(1) },
        StaticCardinality::Many => {
            ValueContract { min_values: if required { 1 } else { 0 }, max_values: None }
        }
        StaticCardinality::Switch => {
            unreachable!("generated positional arguments cannot be value-less switches")
        }
    }
}

/// Resolves one private semantic argument key to its public context-local name.
fn argument_name(command: &CommandSpec<'_>, key: Key) -> String {
    if let Some(flag) = command.flags.iter().copied().find(|flag| flag.key == key) {
        return preferred_option_name(flag);
    }
    if let Some(arg) = command.args.iter().find(|arg| arg.key == key) {
        return arg.name.to_owned();
    }
    unreachable!("generated contract constraint key must belong to its command context")
}

/// Returns the preferred canonical command-line spelling for one named option.
fn preferred_option_name(flag: &FlagSpec<'_>) -> String {
    if let Some(long) = flag.longs.first() {
        return format!("--{long}");
    }
    if let Some(short) = flag.shorts.first() {
        return format!("-{}", char::from(*short));
    }
    unreachable!("generated named argument must have at least one canonical spelling")
}
