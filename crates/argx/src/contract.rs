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
//! definition table contains only definitions referenced by detailed nodes returned in that result.
//!
//! The serialized representation carries [`CONTRACT_VERSION`] so consumers can identify the wire
//! format they received. It is intentionally sparse: optional empty collections and default-false
//! argument properties are omitted where absence is unambiguous, while command `invocable` remains
//! explicit. Positional multiplicity is expressed directly through `required` and
//! `variadic`; a named option's `type` is present exactly when each occurrence consumes one value,
//! while `repeatable` controls occurrence multiplicity. Compatibility guarantees are release-policy
//! concerns rather than inferred from Rust API compatibility.

use std::{error, fmt};

use serde::Serialize;

use crate::{
    __private::{
        Arg as StaticArg, Command as StaticCommand, CommandArgs as StaticCommandArgs,
        CommandValueTypes, ConstraintKind, Flag as StaticFlag, Key,
        ResolveCommandTypeContract as SemanticCommandContract, TypeResolver,
    },
    error::display_bytes,
    type_contract::{TypeContractValue, TypeDefinition},
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
    /// Shared semantic Rust type definitions referenced by invocation and execution contracts.
    ///
    /// References resolve by document-local definition ID. Definition names are descriptive and
    /// are not required to be unique. Omitted when the returned command detail references no named
    /// semantic types.
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

/// Builds one public discovery response from the generated static command metadata.
///
/// Alias resolution happens only while walking the requested path. Once selected, every path in
/// the returned value is rebuilt from canonical command names so aliases cannot leak into the wire
/// representation.
pub(crate) fn discover<T>(request: ContractRequest) -> Result<Contract, ContractError>
where
    T: StaticCommandArgs + SemanticCommandContract,
{
    let root = T::COMMAND;
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

    let mut resolver = TypeResolver::default();
    let command =
        command_contract::<T>(selected, path, &contexts, request.depth, true, &mut resolver);
    let types = resolver.finish();

    Ok(Contract { version: CONTRACT_VERSION, root: root.name.to_owned(), command, types })
}

/// Builds one command node at the requested discovery depth.
fn command_contract<T>(
    command: &'static StaticCommand<'static>,
    path: Vec<String>,
    contexts: &[&'static StaticCommand<'static>],
    depth: ContractDepth,
    detailed: bool,
    resolver: &mut TypeResolver,
) -> CommandContract
where
    T: StaticCommandArgs + SemanticCommandContract,
{
    let (invocation, execution) = if detailed {
        let (invocation, execution) = invocation_contract::<T>(contexts, resolver);
        assert_eq!(
            command.subcommands.is_empty(),
            execution.is_some(),
            "generated invocable command and execution contract projections must remain aligned",
        );
        (Some(invocation), execution)
    } else {
        (None, None)
    };

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
                command_contract::<T>(
                    child,
                    child_path,
                    &child_contexts,
                    depth,
                    child_detailed,
                    resolver,
                )
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
        execution,
        subcommands,
    }
}

/// Builds complete invocation contexts and resolves their semantic Rust value types.
fn invocation_contract<T>(
    contexts: &[&'static StaticCommand<'static>],
    resolver: &mut TypeResolver,
) -> (Vec<CommandContextContract>, Option<ExecutionContract>)
where
    T: StaticCommandArgs + SemanticCommandContract,
{
    let branch_indices = contexts
        .windows(2)
        .map(|pair| {
            let parent = pair[0];
            let child = pair[1];
            parent
                .subcommands
                .iter()
                .position(|candidate| std::ptr::eq(*candidate, child))
                .expect("generated contract contexts must follow the static command topology")
        })
        .collect::<Vec<_>>();
    let last_index = contexts.len().saturating_sub(1);
    let mut selected_execution = None;
    let contexts = contexts
        .iter()
        .enumerate()
        .map(|(index, context)| {
            let semantic = T::contract_types(&branch_indices[..index], resolver)
                .expect("generated contract command path must resolve semantic types");
            if index == last_index {
                selected_execution = semantic.execution;
            }
            command_context(context, &contexts[..=index], semantic.values)
        })
        .collect();

    (contexts, selected_execution)
}

/// Builds one command-context contract and resolves semantic constraint keys to public names.
fn command_context(
    command: &'static StaticCommand<'static>,
    contexts: &[&'static StaticCommand<'static>],
    value_types: CommandValueTypes,
) -> CommandContextContract {
    assert_eq!(
        command.flags.len(),
        value_types.flags.len(),
        "generated flag contract and semantic type projections must remain aligned",
    );
    assert_eq!(
        command.args.len(),
        value_types.args.len(),
        "generated positional contract and semantic type projections must remain aligned",
    );

    let path = contexts.iter().skip(1).map(|context| context.name.to_owned()).collect();
    let positionals = command
        .args
        .iter()
        .zip(value_types.args)
        .map(|(arg, value_type)| arg_contract(arg, value_type))
        .collect();
    let options = command
        .flags
        .iter()
        .copied()
        .zip(value_types.flags)
        .map(|(flag, value_type)| option_contract(flag, value_type))
        .collect();
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

    CommandContextContract { path, positionals, options, constraints }
}

/// Builds one named public option contract.
fn option_contract(flag: &StaticFlag<'_>, value_type: Option<TypeContractValue>) -> OptionContract {
    let name = flag.diagnostic.to_owned();
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

    let value_type = if flag.takes_value {
        Some(value_type.expect("generated value-taking options must expose a semantic value type"))
    } else {
        assert!(value_type.is_none(), "generated switches must not expose a consumed value type",);
        None
    };

    OptionContract {
        name,
        aliases,
        help: flag.help.map(str::to_owned),
        global: flag.global,
        required: flag.required || flag.required_if_env_unset,
        value_type,
        repeatable: flag.repeatable,
        environment: flag.env.map(str::to_owned),
        has_default: flag.has_default,
        allow_hyphen_values: flag.allow_hyphen_values,
        allow_negative_numbers: flag.allow_negative_numbers,
    }
}

/// Builds one positional public argument contract.
fn arg_contract(arg: &StaticArg<'_>, value_type: TypeContractValue) -> PositionalContract {
    PositionalContract {
        name: arg.name.to_owned(),
        help: arg.help.map(str::to_owned),
        required: arg.required,
        variadic: arg.variadic,
        value_type,
        allow_negative_numbers: arg.allow_negative_numbers,
    }
}

/// Resolves one private semantic argument key to its public context-local name.
fn argument_name(command: &StaticCommand<'_>, key: Key) -> String {
    if let Some(flag) = command.flags.iter().copied().find(|flag| flag.key == key) {
        return flag.diagnostic.to_owned();
    }
    if let Some(arg) = command.args.iter().find(|arg| arg.key == key) {
        return arg.name.to_owned();
    }
    unreachable!("generated contract constraint key must belong to its command context")
}
