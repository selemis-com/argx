//! Projection from generated static command metadata into the public contract model.

use super::{
    CONTRACT_VERSION, ActionContract, ActionContractKind, CommandContextContract, CommandContract,
    ConstraintContract, ConstraintContractKind, Contract, ContractDepth, ContractError,
    ContractRequest, ExecutionContract, OptionContract, PositionalContract,
};
use crate::{
    __private::{
        Action as StaticAction, ActionKind as StaticActionKind, Arg as StaticArg,
        Command as StaticCommand, CommandArgs as StaticCommandArgs, CommandExecutionTypes,
        CommandValueTypes, ConstraintKind, Flag as StaticFlag, Key,
        ResolveCommandTypeContract as SemanticCommandContract, TypeResolver,
    },
    type_contract::TypeContractValue,
};

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
        let execution = execution.map(|execution| ExecutionContract {
            success: execution.success,
            error: execution.error,
        });
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
) -> (Vec<CommandContextContract>, Option<CommandExecutionTypes>)
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
    let actions = command.actions.iter().copied().map(action_contract).collect();
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

    CommandContextContract { path, actions, positionals, options, constraints }
}

/// Builds one built-in terminal action contract.
fn action_contract(action: &StaticAction<'_>) -> ActionContract {
    let name = action.diagnostic.to_owned();
    let mut aliases = Vec::with_capacity(action.longs.len() + action.shorts.len());
    aliases.extend(
        action.longs.iter().map(|long| format!("--{long}")).filter(|spelling| spelling != &name),
    );
    aliases.extend(
        action
            .shorts
            .iter()
            .map(|short| format!("-{}", char::from(*short)))
            .filter(|spelling| spelling != &name),
    );

    let kind = match action.kind {
        StaticActionKind::Help => ActionContractKind::Help,
        StaticActionKind::Version { .. } => ActionContractKind::Version,
    };

    ActionContract { name, aliases, kind }
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
