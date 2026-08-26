//! Stable machine-readable invocation contracts.

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
    pub contract_version: u32,
    /// Canonical root command name.
    pub root: String,
    /// Selected command and requested descendant discovery.
    pub command: CommandContract,
}

impl Contract {
    /// Serializes this contract as compact JSON.
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
    /// Arguments accepted in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<ArgumentContract>,
    /// Relationships between arguments in this command context.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<ConstraintContract>,
}

/// One named or positional argument in a machine invocation contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentContract {
    /// Opaque identifier used by constraints within this command context.
    pub id: String,
    /// Human-readable semantic argument name.
    pub name: String,
    /// One-line argument description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Command-line syntax for this argument.
    pub syntax: ArgumentSyntax,
    /// Rust value cardinality represented by this argument.
    pub cardinality: ArgumentCardinality,
    /// Whether a value must resolve from argv or environment when no typed default exists.
    pub required: bool,
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

/// Command-line syntax for one argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ArgumentSyntax {
    /// A named flag or option.
    Named {
        /// Canonical long spellings without the leading `--`.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        longs: Vec<String>,
        /// Hidden long aliases without the leading `--`.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        aliases: Vec<String>,
        /// ASCII short spellings without the leading `-`.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        shorts: Vec<char>,
        /// Whether this argument remains in scope after entering descendants.
        global: bool,
    },
    /// A positional value.
    Positional {
        /// Zero-based position within this command context's positional sequence.
        index: usize,
    },
}

/// Rust value cardinality represented by one argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgumentCardinality {
    /// A value-less named boolean switch.
    Switch,
    /// Exactly one resolved value.
    One,
    /// Zero or one value.
    Optional,
    /// Zero or more values.
    Many,
}

/// One normalized relationship between arguments in the same command context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintContract {
    /// Relationship behavior.
    pub kind: ConstraintContractKind,
    /// Opaque argument identifier for the argument declaring the relationship.
    pub source: String,
    /// Opaque argument identifier for the referenced argument.
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
        contract_version: CONTRACT_VERSION,
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

/// Builds one command-context contract and resolves semantic constraint keys to protocol ids.
fn command_context(
    command: &'static CommandSpec<'static>,
    contexts: &[&'static CommandSpec<'static>],
) -> CommandContextContract {
    let path = contexts.iter().skip(1).map(|context| context.name.to_owned()).collect();
    let arguments = command_arguments(command);
    let constraints = command
        .constraints
        .iter()
        .map(|constraint| ConstraintContract {
            kind: match constraint.kind {
                ConstraintKind::Requires => ConstraintContractKind::Requires,
                ConstraintKind::Conflicts => ConstraintContractKind::Conflicts,
            },
            source: argument_id(command, constraint.source),
            target: argument_id(command, constraint.target),
        })
        .collect();

    CommandContextContract { path, arguments, constraints }
}

/// Builds public argument contracts for one command context.
fn command_arguments(command: &CommandSpec<'_>) -> Vec<ArgumentContract> {
    let mut arguments = Vec::with_capacity(command.flags.len() + command.args.len());
    for flag in command.flags {
        arguments.push(flag_contract(flag));
    }
    for (index, arg) in command.args.iter().enumerate() {
        arguments.push(arg_contract(arg, index));
    }
    arguments
}

/// Builds one named public argument contract.
fn flag_contract(flag: &FlagSpec<'_>) -> ArgumentContract {
    ArgumentContract {
        id: flag_id(flag),
        name: flag.name.to_owned(),
        help: flag.help.map(str::to_owned),
        syntax: ArgumentSyntax::Named {
            longs: flag.longs.iter().copied().map(str::to_owned).collect(),
            aliases: flag.aliases.iter().copied().map(str::to_owned).collect(),
            shorts: flag.shorts.iter().copied().map(char::from).collect(),
            global: flag.global,
        },
        cardinality: cardinality(flag.cardinality),
        required: flag.required,
        environment: flag.env.map(str::to_owned),
        has_default: flag.has_default,
        allow_hyphen_values: flag.allow_hyphen_values,
        allow_negative_numbers: flag.allow_negative_numbers,
    }
}

/// Builds one positional public argument contract.
fn arg_contract(arg: &ArgSpec<'_>, index: usize) -> ArgumentContract {
    ArgumentContract {
        id: positional_id(index),
        name: arg.name.to_owned(),
        help: arg.help.map(str::to_owned),
        syntax: ArgumentSyntax::Positional { index },
        cardinality: cardinality(arg.cardinality),
        required: arg.required,
        environment: None,
        has_default: arg.has_default,
        allow_hyphen_values: false,
        allow_negative_numbers: arg.allow_negative_numbers,
    }
}

/// Converts one private cardinality into the stable public protocol vocabulary.
const fn cardinality(value: StaticCardinality) -> ArgumentCardinality {
    match value {
        StaticCardinality::Switch => ArgumentCardinality::Switch,
        StaticCardinality::One => ArgumentCardinality::One,
        StaticCardinality::Optional => ArgumentCardinality::Optional,
        StaticCardinality::Many => ArgumentCardinality::Many,
    }
}

/// Resolves one private semantic argument key to its public context-local identifier.
fn argument_id(command: &CommandSpec<'_>, key: Key) -> String {
    if let Some(flag) = command.flags.iter().copied().find(|flag| flag.key == key) {
        return flag_id(flag);
    }
    if let Some(index) = command.args.iter().position(|arg| arg.key == key) {
        return positional_id(index);
    }
    unreachable!("generated contract constraint key must belong to its command context")
}

/// Returns the opaque public identifier for one named argument.
fn flag_id(flag: &FlagSpec<'_>) -> String {
    if let Some(long) = flag.longs.first() {
        return format!("flag:--{long}");
    }
    if let Some(short) = flag.shorts.first() {
        return format!("flag:-{}", char::from(*short));
    }
    unreachable!("generated named argument must have at least one canonical spelling")
}

/// Returns the opaque public identifier for one positional argument index.
fn positional_id(index: usize) -> String {
    format!("positional:{index}")
}
