//! Code generation for `Parser` and `Args` structs.
//!
//! One normalized command produces two coordinated artifacts: static command metadata and typed
//! partial-state binding code. Flattened `Args` children are composed into the static metadata at
//! compile time while retaining nested Rust values in the typed state. Partial binding state is
//! wrapped in generated nominal witnesses so private composed types do not leak through the public
//! derive ABI. This is intentionally verbose generation: semantics
//! should be decided in `model`, not rediscovered from emitted tables.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Generics, parse_quote};

use super::option_str;
use crate::{crate_name, key, model};

/// Generated static table expressions for one command declaration.
#[derive(Debug)]
struct Tables {
    /// Declarations required to compose flattened child tables.
    decls: TokenStream,
    /// Final flag slice expression stored on the command.
    flags: TokenStream,
    /// Final positional slice expression stored on the command.
    args: TokenStream,
    /// Final normalized constraint slice expression stored on the command.
    constraints: TokenStream,
    /// Final flattened help-group slice expression stored on the command.
    help_groups: TokenStream,
}

/// Generated semantic type projection for one command declaration.
#[derive(Debug)]
struct SemanticProjection {
    /// Helper declarations emitted alongside the derived type.
    declarations: TokenStream,
    /// Partial-state type exposed through `CommandArgs`.
    partial: TokenStream,
    /// Nominal constructor used when private composed types require an opaque partial witness.
    partial_constructor: Option<proc_macro2::Ident>,
    /// Type-level resolver for values owned by this command context.
    fields: TokenStream,
    /// Type-level resolver for this command's execution result when directly invocable.
    execution: TokenStream,
    /// Type-level resolver for the nested subcommand field, when present.
    subcommands: TokenStream,
}

/// Generates static parse metadata and typed binding for one command struct.
pub(crate) fn command(command: &model::Command) -> TokenStream {
    let facade = crate_name::facade_path();
    let ident = &command.binding.ident;
    let binding_generics = binding_generics(command, &facade);
    let (impl_generics, ty_generics, where_clause) = binding_generics.split_for_impl();
    let (semantic_impl_generics, semantic_ty_generics, semantic_where_clause) =
        command.binding.generics.split_for_impl();

    // Preserve source field indices while partitioning CLI arguments. Generated partial state is
    // still one tuple in Rust declaration order, whereas static tables are grouped by CLI kind.
    let flags = command
        .fields
        .iter()
        .enumerate()
        .filter(|(_, field)| {
            matches!(
                &field.semantics,
                model::FieldSemantics::Argument(model::Argument {
                    kind: model::ArgumentKind::Flag { .. },
                    ..
                })
            )
        })
        .collect::<Vec<_>>();
    let args = command
        .fields
        .iter()
        .enumerate()
        .filter(|(_, field)| {
            matches!(
                &field.semantics,
                model::FieldSemantics::Argument(model::Argument {
                    kind: model::ArgumentKind::Positional,
                    ..
                })
            )
        })
        .collect::<Vec<_>>();
    let subcommand = command
        .fields
        .iter()
        .enumerate()
        .find(|(_, field)| matches!(&field.semantics, model::FieldSemantics::Subcommand));
    let keys = key::constants(&facade, &command.binding.fingerprint, flags.len(), args.len());
    let command_key = key::ident("COMMAND", None);

    // Static command metadata is generated once from the normalized argument model and is shared
    // by parsing, help generation, and machine-contract discovery.
    let flag_tables = flags.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_FLAG_{index}");
        let key = key::ident("FLAG", Some(index));
        let Some(argument) = field.argument() else {
            unreachable!("flag list only contains argument fields");
        };
        let model::ArgumentKind::Flag { longs, aliases, shorts } = &argument.kind else {
            unreachable!("flag list only contains named arguments");
        };
        let name = &field.binding.name;
        let diagnostic = &argument.diagnostic;
        let help = option_str(argument.help.as_deref());
        let global = argument.global;
        let env = option_str(argument.env.as_deref());
        let takes_value = !field.is_switch();
        let repeatable = argument.shape == model::Shape::Many;
        let required = argument.shape == model::Shape::Required
            && argument.env.is_none()
            && !argument.has_default;
        let required_if_env_unset = argument.shape == model::Shape::Required
            && argument.env.is_some()
            && !argument.has_default;
        let has_default = argument.has_default;
        let allow_hyphen_values = argument.allow_hyphen_values;
        let allow_negative_numbers = argument.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Flag<'static> = #facade::__private::Flag {
                key: #key,
                name: #name,
                diagnostic: #diagnostic,
                help: #help,
                longs: &[#(#longs),*],
                aliases: &[#(#aliases),*],
                shorts: &[#(#shorts),*],
                global: #global,
                env: #env,
                takes_value: #takes_value,
                repeatable: #repeatable,
                required: #required,
                required_if_env_unset: #required_if_env_unset,
                has_default: #has_default,
                allow_hyphen_values: #allow_hyphen_values,
                allow_negative_numbers: #allow_negative_numbers,
            };
        }
    });

    let arg_tables = args.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_ARG_{index}");
        let key = key::ident("ARG", Some(index));
        let Some(argument) = field.argument() else {
            unreachable!("positional list only contains argument fields");
        };
        let name = &field.binding.name;
        let help = option_str(argument.help.as_deref());
        let required = matches!(argument.shape, model::Shape::Bool | model::Shape::Required);
        let variadic = argument.shape == model::Shape::Many;
        let allow_negative_numbers = argument.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Arg<'static> =
                #facade::__private::Arg {
                    key: #key,
                    name: #name,
                    help: #help,
                    required: #required,
                    variadic: #variadic,
                    allow_negative_numbers: #allow_negative_numbers,
                };
        }
    });
    let constraint_count = command
        .fields
        .iter()
        .filter_map(model::Field::argument)
        .map(|argument| argument.requires.len() + argument.conflicts.len())
        .sum();
    let tables = command_tables(command, &facade, flags.len(), args.len(), constraint_count);
    let table_decls = &tables.decls;
    let command_flags = &tables.flags;
    let command_args = &tables.args;
    let command_constraints = &tables.constraints;
    let command_help_groups = &tables.help_groups;
    let constraint_tables = constraint_tables(command, &facade, command_flags, command_args);
    let semantic_projection = semantic_projection(command, &facade);
    let semantic_declarations = &semantic_projection.declarations;
    let partial_type = &semantic_projection.partial;
    let semantic_fields = &semantic_projection.fields;
    let semantic_execution = &semantic_projection.execution;
    let semantic_subcommands = &semantic_projection.subcommands;
    let invocable_contract_impl = subcommand.is_none().then(|| {
        quote! {
            impl #impl_generics #facade::__private::InvocableCommandContract
                for #ident #ty_generics #where_clause
            {}
        }
    });

    // Partial state mirrors Rust field order. Direct arguments accumulate raw bytes, flattened
    // declarations retain their own partial state, and subcommands retain branch-selection state.
    let partial_start = command
        .fields
        .iter()
        .map(|field| match &field.semantics {
            model::FieldSemantics::Flatten => {
                let ty = &field.binding.ty;
                quote!(<#ty as #facade::__private::CommandArgs>::start())
            }
            model::FieldSemantics::Subcommand => {
                let ty = &field.binding.ty;
                quote!(<#ty as #facade::__private::Subcommands>::start())
            }
            _ if field.argument().is_some_and(|argument| argument.shape == model::Shape::Many) => {
                quote!(::std::vec::Vec::new())
            }
            _ if field.is_switch() => quote!((false, false)),
            _ => quote!((::std::option::Option::None, false)),
        })
        .collect::<Vec<_>>();
    let partial_value = semantic_projection.partial_constructor.as_ref().map_or_else(
        || {
            if command.fields.is_empty() {
                TokenStream::new()
            } else {
                quote!((#(#partial_start,)*))
            }
        },
        |partial_constructor| quote!(#partial_constructor(#(#partial_start,)*)),
    );

    // Event ownership is key-based rather than spelling-based. Spelling resolution already happened
    // in the raw parser, which keeps aliases and lexical shadowing out of typed binding.
    let flag_apply = flags.iter().enumerate().map(|(index, (field_index, field))| {
        let table = format_ident!("ARGX_FLAG_{index}");
        let key = key::ident("FLAG", Some(index));
        apply_arm(field, *field_index, &table, &key, true, &facade)
    });
    let arg_apply = args.iter().enumerate().map(|(index, (field_index, field))| {
        let table = format_ident!("ARGX_ARG_{index}");
        let key = key::ident("ARG", Some(index));
        apply_arm(field, *field_index, &table, &key, false, &facade)
    });
    let flattened_apply = command.fields.iter().enumerate().filter_map(|(field_index, field)| {
        if !matches!(&field.semantics, model::FieldSemantics::Flatten) {
            return None;
        }
        let ty = &field.binding.ty;
        let slot = syn::Index::from(field_index);
        Some(quote! {
            if <#ty as #facade::__private::CommandArgs>::apply(&mut partial.#slot, event) {
                return true;
            }
        })
    });
    let selected_subcommand_apply = subcommand.map(|(field_index, field)| {
        let ty = &field.binding.ty;
        let slot = syn::Index::from(field_index);
        quote! {
            if <#ty as #facade::__private::Subcommands>::selected(&partial.#slot)
                && <#ty as #facade::__private::Subcommands>::apply(
                    &mut partial.#slot,
                    event,
                )
            {
                return true;
            }
        }
    });
    let subcommand_apply = subcommand.map(|(field_index, field)| {
        let ty = &field.binding.ty;
        let slot = syn::Index::from(field_index);
        quote! {
            if !<#ty as #facade::__private::Subcommands>::selected(&partial.#slot)
                && <#ty as #facade::__private::Subcommands>::apply(
                    &mut partial.#slot,
                    event,
                )
            {
                return true;
            }
        }
    });

    // Environment fallbacks are generated only for eligible direct scalar flags. Flattened and
    // selected subcommand states recurse into their own independently generated fallback logic.
    let own_env_apply = command.fields.iter().enumerate().filter_map(|(field_index, field)| {
        let argument = field.argument()?;
        let env = argument.env.as_deref()?;
        let slot = syn::Index::from(field_index);
        Some(quote! {
            if partial.#slot.0.is_none() {
                if let ::std::option::Option::Some(value) = ::std::env::var_os(#env) {
                    partial.#slot.0 = ::std::option::Option::Some(
                        #facade::__private::RawValue::Environment {
                            name: #env,
                            value,
                        },
                    );
                }
            }
        })
    });
    let flattened_env_apply =
        command.fields.iter().enumerate().filter_map(|(field_index, field)| {
            if !matches!(&field.semantics, model::FieldSemantics::Flatten) {
                return None;
            }
            let ty = &field.binding.ty;
            let slot = syn::Index::from(field_index);
            Some(quote! {
                <#ty as #facade::__private::CommandArgs>::apply_env(&mut partial.#slot);
            })
        });
    let subcommand_env_apply = subcommand.map(|(field_index, field)| {
        let ty = &field.binding.ty;
        let slot = syn::Index::from(field_index);
        quote! {
            <#ty as #facade::__private::Subcommands>::apply_env(&mut partial.#slot);
        }
    });

    let env_partial = if command.fields.iter().any(|field| {
        field.argument().is_some_and(|argument| argument.env.is_some())
            || matches!(
                &field.semantics,
                model::FieldSemantics::Flatten | model::FieldSemantics::Subcommand
            )
    }) {
        quote!(partial)
    } else {
        quote!(_partial)
    };

    let flag_state = flags.iter().enumerate().map(|(index, (field_index, field))| {
        let key = key::ident("FLAG", Some(index));
        argument_state_branch(field, *field_index, &key, &facade)
    });
    let arg_state = args.iter().enumerate().map(|(index, (field_index, field))| {
        let key = key::ident("ARG", Some(index));
        argument_state_branch(field, *field_index, &key, &facade)
    });
    let flattened_state = command.fields.iter().enumerate().filter_map(|(field_index, field)| {
        if !matches!(&field.semantics, model::FieldSemantics::Flatten) {
            return None;
        }
        let ty = &field.binding.ty;
        let slot = syn::Index::from(field_index);
        Some(quote! {
            if let ::std::option::Option::Some(state) =
                <#ty as #facade::__private::CommandArgs>::argument_state(&partial.#slot, key)
            {
                return ::std::option::Option::Some(state);
            }
        })
    });
    let subcommand_constraint_check = subcommand.map(|(field_index, field)| {
        let ty = &field.binding.ty;
        let slot = syn::Index::from(field_index);
        quote! {
            <#ty as #facade::__private::Subcommands>::check_constraints(&partial.#slot)?;
        }
    });

    // Validation is emitted in the same staged order used by `CommandArgs::check`: occurrence
    // policy first, then source fallback, requiredness, relationships, and finally conversion.
    let occurrence_checks = command
        .fields
        .iter()
        .enumerate()
        .filter_map(|(field_index, field)| match &field.semantics {
            model::FieldSemantics::Argument(argument)
                if matches!(&argument.kind, model::ArgumentKind::Flag { .. })
                    && argument.shape != model::Shape::Many =>
            {
                let slot = syn::Index::from(field_index);
                let name = &argument.diagnostic;
                Some(quote! {
                    if partial.#slot.1 {
                        return ::std::result::Result::Err(
                            #facade::Error::DuplicateArgument { name: #name },
                        );
                    }
                })
            }
            model::FieldSemantics::Flatten => {
                let ty = &field.binding.ty;
                let slot = syn::Index::from(field_index);
                Some(quote! {
                    <#ty as #facade::__private::CommandArgs>::check_occurrences(
                        &mut partial.#slot,
                    )?;
                })
            }
            model::FieldSemantics::Subcommand => {
                let ty = &field.binding.ty;
                let slot = syn::Index::from(field_index);
                Some(quote! {
                    <#ty as #facade::__private::Subcommands>::check_occurrences(
                        &mut partial.#slot,
                    )?;
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let required_checks = command
        .fields
        .iter()
        .enumerate()
        .filter_map(|(field_index, field)| match &field.semantics {
            model::FieldSemantics::Flatten => {
                let ty = &field.binding.ty;
                let slot = syn::Index::from(field_index);
                Some(quote! {
                    <#ty as #facade::__private::CommandArgs>::check_required(
                        &mut partial.#slot,
                    )?;
                })
            }
            model::FieldSemantics::Subcommand => {
                let ty = &field.binding.ty;
                let slot = syn::Index::from(field_index);
                let name = &field.binding.name;
                Some(quote! {
                    if !<#ty as #facade::__private::Subcommands>::selected(&partial.#slot) {
                        return ::std::result::Result::Err(
                            #facade::Error::MissingSubcommand { name: #name },
                        );
                    }
                    <#ty as #facade::__private::Subcommands>::check_required(
                        &mut partial.#slot,
                    )?;
                })
            }
            model::FieldSemantics::Argument(argument)
                if !field.is_switch()
                    && matches!(argument.shape, model::Shape::Bool | model::Shape::Required)
                    && !argument.has_default =>
            {
                let slot = syn::Index::from(field_index);
                let name = &argument.diagnostic;
                Some(quote! {
                    if partial.#slot.0.is_none() {
                        return ::std::result::Result::Err(
                            #facade::Error::MissingRequired { name: #name },
                        );
                    }
                })
            }
            model::FieldSemantics::Argument(_) => None,
        })
        .collect::<Vec<_>>();

    let apply_partial = if command.fields.is_empty() { quote!(_partial) } else { quote!(partial) };
    let occurrence_partial =
        if occurrence_checks.is_empty() { quote!(_partial) } else { quote!(partial) };
    let required_partial =
        if required_checks.is_empty() { quote!(_partial) } else { quote!(partial) };
    let finish_partial = if command.fields.is_empty() { quote!(_partial) } else { quote!(partial) };

    let built_fields = command.fields.iter().enumerate().map(|(field_index, field)| {
        let ident = &field.binding.ident;
        let value = finish_field(field, field_index, &facade);
        quote!(#ident: #value)
    });
    let built =
        if command.binding.unit { quote!(Self) } else { quote!(Self { #(#built_fields),* }) };

    // These invariants can only be checked after child `Args` tables have been composed. Keep them
    // as const assertions so invalid flattening fails during compilation rather than at runtime.
    let flattened_checks = command.fields.iter().any(model::Field::is_flatten).then(|| {
        quote! {
            const _: () = ::core::assert!(
                #facade::__private::command_keys_unique(ARGX_COMMAND.flags, ARGX_COMMAND.args),
                "flattened command contains duplicate argument keys",
            );
            const _: () = ::core::assert!(
                #facade::__private::flag_spellings_unique(ARGX_COMMAND.flags),
                "flattened command contains duplicate long or short flag spellings",
            );
            const _: () = ::core::assert!(
                #facade::__private::positional_layout_valid(ARGX_COMMAND.args),
                "flattened command has an invalid positional layout",
            );
        }
    });

    let name = &command.semantics.name;
    let about = option_str(command.semantics.about.as_deref());
    let description = option_str(command.semantics.description.as_deref());
    let help_sections = command.semantics.help_sections.iter().map(|section| {
        let heading = &section.heading;
        let body = &section.body;
        quote! {
            #facade::__private::HelpSection {
                heading: #heading,
                body: #body,
            }
        }
    });
    let aliases = &command.semantics.aliases;
    let short_version =
        command.semantics.version.as_ref().or(command.semantics.long_version.as_ref());
    let long_version =
        command.semantics.long_version.as_ref().or(command.semantics.version.as_ref());
    let version_action = short_version.zip(long_version).map(|(short, long)| {
        quote! {
            static ARGX_VERSION_ACTION: #facade::__private::Action<'static> =
                #facade::__private::Action {
                    name: "version",
                    diagnostic: "--version",
                    help: "Print version",
                    longs: &["version"],
                    shorts: b"V",
                    kind: #facade::__private::ActionKind::Version {
                        short: #short,
                        long: #long,
                    },
                };
        }
    });
    let command_actions = if version_action.is_some() {
        quote!(&[&#facade::__private::HELP_ACTION, &ARGX_VERSION_ACTION])
    } else {
        quote!(&[&#facade::__private::HELP_ACTION])
    };
    let command_subcommands = subcommand.map_or_else(
        || quote!(&[]),
        |(_, field)| {
            let ty = &field.binding.ty;
            quote!(<#ty as #facade::__private::Subcommands>::COMMANDS)
        },
    );
    let parser_impl = command.binding.root.then(|| {
        quote! {
            impl #impl_generics #facade::Parser for #ident #ty_generics #where_clause {}
        }
    });
    let args_impl = (!command.binding.root).then(|| {
        quote! {
            impl #impl_generics #facade::Args
                for #ident #ty_generics #where_clause
            {}
        }
    });

    // A private const namespace keeps all generated statics and assertions local to the derived
    // declaration while still allowing trait associated constants to point at `'static` tables.
    quote! {
        #semantic_declarations

        #[doc(hidden)]
        const _: () = {
            #keys
            #(#flag_tables)*
            #(#arg_tables)*
            #table_decls
            #(#constraint_tables)*
            #version_action

            static ARGX_COMMAND: #facade::__private::Command<'static> =
                #facade::__private::Command {
                    name: #name,
                    about: #about,
                    description: #description,
                    help_sections: &[#(#help_sections),*],
                    help_groups: #command_help_groups,
                    aliases: &[#(#aliases),*],
                    actions: #command_actions,
                    flags: #command_flags,
                    args: #command_args,
                    constraints: #command_constraints,
                    subcommands: #command_subcommands,
                    key: #command_key,
                };

            const _: () = ::core::assert!(
                #facade::__private::action_flag_spellings_disjoint(
                    ARGX_COMMAND.actions,
                    ARGX_COMMAND.flags,
                ),
                "command contains a flag spelling reserved by a built-in action",
            );
            #flattened_checks

            impl #semantic_impl_generics #facade::__private::CommandTypeContract
                for #ident #semantic_ty_generics #semantic_where_clause
            {
                type Fields = #semantic_fields;
                type Execution = #semantic_execution;
                type Subcommands = #semantic_subcommands;
            }

            #invocable_contract_impl

            impl #impl_generics #facade::__private::CommandArgs
                for #ident #ty_generics #where_clause
            {
                type Partial = #partial_type;

                const COMMAND: &'static #facade::__private::Command<'static> = &ARGX_COMMAND;

                fn start() -> Self::Partial {
                    #partial_value
                }

                fn apply(
                    #apply_partial: &mut Self::Partial,
                    event: &#facade::__private::Event<'_, '_>,
                ) -> bool {
                    #selected_subcommand_apply
                    let matched = match *event {
                        #facade::__private::Event::Flag { flag, value } => {
                            let _ = value;
                            match flag.key {
                                #(#flag_apply)*
                                _ => false,
                            }
                        },
                        #facade::__private::Event::Arg { arg, value } => {
                            let _ = value;
                            match arg.key {
                                #(#arg_apply)*
                                _ => false,
                            }
                        },
                        #facade::__private::Event::Command { .. } => false,
                        #facade::__private::Event::Action { .. } => false,
                    };
                    if matched {
                        return true;
                    }
                    #(#flattened_apply)*
                    #subcommand_apply
                    false
                }

                fn apply_env(#env_partial: &mut Self::Partial) {
                    #(#own_env_apply)*
                    #(#flattened_env_apply)*
                    #subcommand_env_apply
                }

                fn argument_state(
                    partial: &Self::Partial,
                    key: #facade::__private::Key,
                ) -> ::std::option::Option<#facade::__private::ArgumentState> {
                    let _ = (partial, key);
                    #(#flag_state)*
                    #(#arg_state)*
                    #(#flattened_state)*
                    ::std::option::Option::None
                }

                fn check_constraints(
                    partial: &Self::Partial,
                ) -> ::std::result::Result<(), #facade::Error> {
                    for constraint in Self::COMMAND.constraints {
                        let source = Self::argument_state(partial, constraint.source)
                            .expect("generated constraint source must belong to this command");
                        if !source.given {
                            continue;
                        }
                        let target = Self::argument_state(partial, constraint.target)
                            .expect("generated constraint target must belong to this command");
                        match constraint.kind {
                            #facade::__private::ConstraintKind::Requires if !target.satisfied => {
                                return ::std::result::Result::Err(
                                    #facade::Error::MissingRequirement {
                                        name: target.diagnostic,
                                        required_by: source.diagnostic,
                                    },
                                );
                            }
                            #facade::__private::ConstraintKind::Conflicts if target.given => {
                                return ::std::result::Result::Err(
                                    #facade::Error::ConflictingArguments {
                                        name: source.diagnostic,
                                        other: target.diagnostic,
                                    },
                                );
                            }
                            _ => {}
                        }
                    }
                    #subcommand_constraint_check
                    ::std::result::Result::Ok(())
                }

                fn check_occurrences(
                    #occurrence_partial: &mut Self::Partial,
                ) -> ::std::result::Result<(), #facade::Error> {
                    #(#occurrence_checks)*
                    ::std::result::Result::Ok(())
                }

                fn check_required(
                    #required_partial: &mut Self::Partial,
                ) -> ::std::result::Result<(), #facade::Error> {
                    #(#required_checks)*
                    ::std::result::Result::Ok(())
                }

                fn finish(
                    #finish_partial: Self::Partial,
                ) -> ::std::result::Result<Self, #facade::Error> {
                    ::std::result::Result::Ok(#built)
                }
            }

            #parser_impl
            #args_impl
        };
    }
}

/// Generates normalized relationship tables for arguments declared directly on this command.
fn constraint_tables(
    command: &model::Command,
    facade: &TokenStream,
    flags: &TokenStream,
    args: &TokenStream,
) -> Vec<TokenStream> {
    let mut generated = Vec::new();
    let mut flag_index = 0_usize;
    let mut arg_index = 0_usize;
    let mut constraint_index = 0_usize;

    for field in &command.fields {
        let Some(argument) = field.argument() else {
            continue;
        };
        let source = match &argument.kind {
            model::ArgumentKind::Flag { .. } => {
                let key = key::ident("FLAG", Some(flag_index));
                flag_index += 1;
                key
            }
            model::ArgumentKind::Positional => {
                let key = key::ident("ARG", Some(arg_index));
                arg_index += 1;
                key
            }
        };

        for (kind, target) in argument
            .requires
            .iter()
            .map(|target| (quote!(#facade::__private::ConstraintKind::Requires), target))
            .chain(
                argument
                    .conflicts
                    .iter()
                    .map(|target| (quote!(#facade::__private::ConstraintKind::Conflicts), target)),
            )
        {
            let table = format_ident!("ARGX_CONSTRAINT_{constraint_index}");
            constraint_index += 1;
            let target = constraint_target(command, target, facade, flags, args);
            generated.push(quote! {
                const #table: #facade::__private::Constraint = #facade::__private::Constraint {
                    kind: #kind,
                    source: #source,
                    target: #target,
                };
            });
        }
    }

    generated
}

/// Resolves a relationship target to a direct semantic key or a composed flattened lookup.
fn constraint_target(
    command: &model::Command,
    target: &str,
    facade: &TokenStream,
    flags: &TokenStream,
    args: &TokenStream,
) -> TokenStream {
    let mut flag_index = 0_usize;
    let mut arg_index = 0_usize;
    for field in &command.fields {
        let Some(argument) = field.argument() else {
            continue;
        };
        let resolved = match &argument.kind {
            model::ArgumentKind::Flag { .. } => {
                let resolved = key::ident("FLAG", Some(flag_index));
                flag_index += 1;
                resolved
            }
            model::ArgumentKind::Positional => {
                let resolved = key::ident("ARG", Some(arg_index));
                arg_index += 1;
                resolved
            }
        };
        if field.binding.name == target {
            return quote!(#resolved);
        }
    }

    quote!(#facade::__private::argument_key_by_name(#flags, #args, #target))
}

/// Builds one flat parse table from this declaration's own fields and flattened children.
fn command_tables(
    command: &model::Command,
    facade: &TokenStream,
    flag_count: usize,
    arg_count: usize,
    constraint_count: usize,
) -> Tables {
    if !command.fields.iter().any(model::Field::is_flatten) {
        let flags = (0..flag_count).map(|index| {
            let table = format_ident!("ARGX_FLAG_{index}");
            quote!(&#table)
        });
        let args = (0..arg_count).map(|index| {
            let table = format_ident!("ARGX_ARG_{index}");
            quote!(&#table)
        });
        let constraints = (0..constraint_count).map(|index| {
            let table = format_ident!("ARGX_CONSTRAINT_{index}");
            quote!(#table)
        });
        return Tables {
            decls: TokenStream::new(),
            flags: quote!(&[#(#flags),*]),
            args: quote!(&[#(#args),*]),
            constraints: quote!(&[#(#constraints),*]),
            help_groups: quote!(&[]),
        };
    }

    let mut flag_groups = Vec::new();
    let mut arg_groups = Vec::new();
    let mut constraint_groups = Vec::new();
    let mut help_group_groups = Vec::new();
    let mut help_group_decls = Vec::new();
    let mut own_flags = Vec::new();
    let mut own_args = Vec::new();
    let mut own_constraints = Vec::new();
    let mut flag_at = 0_usize;
    let mut arg_at = 0_usize;
    let mut constraint_at = 0_usize;
    let mut flatten_checks = Vec::new();

    fn flush_flags(own: &mut Vec<usize>, groups: &mut Vec<TokenStream>) {
        if own.is_empty() {
            return;
        }
        let refs = own.iter().map(|index| {
            let table = format_ident!("ARGX_FLAG_{index}");
            quote!(&#table)
        });
        groups.push(quote!(&[#(#refs),*]));
        own.clear();
    }

    fn flush_args(own: &mut Vec<usize>, groups: &mut Vec<TokenStream>) {
        if own.is_empty() {
            return;
        }
        let refs = own.iter().map(|index| {
            let table = format_ident!("ARGX_ARG_{index}");
            quote!(&#table)
        });
        groups.push(quote!(&[#(#refs),*]));
        own.clear();
    }

    fn flush_constraints(own: &mut Vec<usize>, groups: &mut Vec<TokenStream>) {
        if own.is_empty() {
            return;
        }
        let entries = own.iter().map(|index| {
            let table = format_ident!("ARGX_CONSTRAINT_{index}");
            quote!(#table)
        });
        groups.push(quote!(&[#(#entries),*]));
        own.clear();
    }

    for field in &command.fields {
        match &field.semantics {
            model::FieldSemantics::Argument(
                argument @ model::Argument { kind: model::ArgumentKind::Flag { .. }, .. },
            ) => {
                own_flags.push(flag_at);
                flag_at += 1;
                for _ in argument.requires.iter().chain(&argument.conflicts) {
                    own_constraints.push(constraint_at);
                    constraint_at += 1;
                }
            }
            model::FieldSemantics::Argument(
                argument @ model::Argument { kind: model::ArgumentKind::Positional, .. },
            ) => {
                own_args.push(arg_at);
                arg_at += 1;
                for _ in argument.requires.iter().chain(&argument.conflicts) {
                    own_constraints.push(constraint_at);
                    constraint_at += 1;
                }
            }
            model::FieldSemantics::Flatten => {
                let ty = &field.binding.ty;
                flush_flags(&mut own_flags, &mut flag_groups);
                flush_args(&mut own_args, &mut arg_groups);
                flush_constraints(&mut own_constraints, &mut constraint_groups);
                flag_groups.push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.flags));
                arg_groups.push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.args));
                constraint_groups
                    .push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.constraints));
                if let Some(heading) = field.help_heading.as_deref() {
                    let group = format_ident!("ARGX_HELP_GROUP_{}", help_group_decls.len());
                    help_group_decls.push(quote! {
                        static #group: #facade::__private::HelpGroup<'static> =
                            #facade::__private::HelpGroup {
                                heading: #heading,
                                flags: <#ty as #facade::__private::CommandArgs>::COMMAND.flags,
                                args: <#ty as #facade::__private::CommandArgs>::COMMAND.args,
                            };
                    });
                    help_group_groups.push(quote!(&[&#group]));
                } else {
                    help_group_groups.push(
                        quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.help_groups),
                    );
                }
                flatten_checks.push(quote! {
                    const _: () = ::core::assert!(
                        <#ty as #facade::__private::CommandArgs>::COMMAND.subcommands.is_empty(),
                        "flattened Args cannot declare subcommands",
                    );
                });
            }
            model::FieldSemantics::Subcommand => {}
        }
    }
    flush_flags(&mut own_flags, &mut flag_groups);
    flush_args(&mut own_args, &mut arg_groups);
    flush_constraints(&mut own_constraints, &mut constraint_groups);

    debug_assert_eq!(flag_at, flag_count);
    debug_assert_eq!(arg_at, arg_count);
    debug_assert_eq!(constraint_at, constraint_count);

    Tables {
        decls: quote! {
            #(#flatten_checks)*
            #(#help_group_decls)*
            const ARGX_FLAG_GROUPS: &[&[&#facade::__private::Flag<'static>]] =
                &[#(#flag_groups),*];
            const ARGX_ARG_GROUPS: &[&[&#facade::__private::Arg<'static>]] =
                &[#(#arg_groups),*];
            static ARGX_FLAGS: [&#facade::__private::Flag<'static>;
                #facade::__private::table_len(ARGX_FLAG_GROUPS)] =
                #facade::__private::concat_flags(ARGX_FLAG_GROUPS);
            static ARGX_ARGS: [&#facade::__private::Arg<'static>;
                #facade::__private::table_len(ARGX_ARG_GROUPS)] =
                #facade::__private::concat_args(ARGX_ARG_GROUPS);
            const ARGX_HELP_GROUP_GROUPS: &[&[&#facade::__private::HelpGroup<'static>]] =
                &[#(#help_group_groups),*];
            static ARGX_HELP_GROUPS: [&#facade::__private::HelpGroup<'static>;
                #facade::__private::table_len(ARGX_HELP_GROUP_GROUPS)] =
                #facade::__private::concat_help_groups(ARGX_HELP_GROUP_GROUPS);
            const ARGX_CONSTRAINT_GROUPS: &[&[#facade::__private::Constraint]] =
                &[#(#constraint_groups),*];
            static ARGX_CONSTRAINTS: [#facade::__private::Constraint;
                #facade::__private::table_len(ARGX_CONSTRAINT_GROUPS)] =
                #facade::__private::concat_constraints(ARGX_CONSTRAINT_GROUPS);
        },
        flags: quote!(&ARGX_FLAGS),
        args: quote!(&ARGX_ARGS),
        constraints: quote!(&ARGX_CONSTRAINTS),
        help_groups: quote!(&ARGX_HELP_GROUPS),
    }
}

/// Builds privacy-safe semantic value resolvers for this command declaration.
fn semantic_projection(command: &model::Command, facade: &TokenStream) -> SemanticProjection {
    let ident = &command.binding.ident;
    let visibility = &command.binding.visibility;
    let suffix = ident.to_string().trim_start_matches("r#").to_owned();
    let declaration = key::declaration_hash(&command.binding.fingerprint);
    let fields_ident = format_ident!("__ArgxContractFieldsFor{}H{}", suffix, declaration);
    let subcommands_ident = format_ident!("__ArgxContractSubcommandsFor{}H{}", suffix, declaration);
    let execution_ident = format_ident!("__ArgxContractExecutionFor{}H{}", suffix, declaration);
    let partial_ident = partial_type_constructor(command);
    let shape_ident = format_ident!("__ArgxContractShapeFor{}H{}", suffix, declaration);
    let (impl_generics, ty_generics, where_clause) = command.binding.generics.split_for_impl();

    let mut shape_declarations = Vec::new();
    let mut shape_definitions = Vec::new();
    let mut associated_fields = vec![None; command.fields.len()];
    let mut field_bounds = Vec::new();
    let mut field_steps = Vec::new();
    let mut has_fields = false;
    let mut needs_shape = false;

    for (index, field) in command.fields.iter().enumerate() {
        match &field.semantics {
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Flag { .. },
                ..
            }) if field.is_switch() => {
                has_fields = true;
                field_steps.push(quote!(values.flags.push(::std::option::Option::None);));
            }
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Flag { .. },
                ..
            }) => {
                has_fields = true;
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.value_binding().ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated.clone());
                field_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::TypeContractSource
                ));
                field_steps.push(quote! {
                    values.flags.push(::std::option::Option::Some(
                        <<T as #shape_ident>::#associated as
                            #facade::__private::TypeContractSource>::resolve_type(resolver),
                    ));
                });
            }
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Positional,
                ..
            }) => {
                has_fields = true;
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.value_binding().ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated.clone());
                field_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::TypeContractSource
                ));
                field_steps.push(quote! {
                    values.args.push(
                        <<T as #shape_ident>::#associated as
                            #facade::__private::TypeContractSource>::resolve_type(resolver),
                    );
                });
            }
            model::FieldSemantics::Flatten => {
                has_fields = true;
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.binding.ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated.clone());
                field_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::CommandTypeContract
                ));
                field_bounds.push(quote!(
                    <<T as #shape_ident>::#associated as
                        #facade::__private::CommandTypeContract>::Fields:
                        #facade::__private::ResolveValueFields
                ));
                field_steps.push(quote! {
                    <<<T as #shape_ident>::#associated as
                        #facade::__private::CommandTypeContract>::Fields as
                        #facade::__private::ResolveValueFields>::append(resolver, values);
                });
            }
            model::FieldSemantics::Subcommand => {
                needs_shape = true;
                let associated = format_ident!("Field{index}");
                let ty = &field.binding.ty;
                shape_declarations.push(quote!(type #associated;));
                shape_definitions.push(quote!(type #associated = #ty;));
                associated_fields[index] = Some(associated);
            }
        }
    }

    let subcommand_projection = command
        .fields
        .iter()
        .enumerate()
        .find(|(_, field)| matches!(&field.semantics, model::FieldSemantics::Subcommand))
        .map(|(index, _)| {
            associated_fields[index]
                .clone()
                .expect("subcommand fields always have a generated shape association")
        });

    let shape = needs_shape.then(|| {
        quote! {
            trait #shape_ident {
                #(#shape_declarations)*
            }

            impl #impl_generics #shape_ident for #ident #ty_generics #where_clause {
                #(#shape_definitions)*
            }
        }
    });

    // `CommandArgs::Partial` is a public associated type because derives must implement the hidden
    // runtime trait downstream. Keep concrete flattened/subcommand types behind private fields of a
    // nominal witness so a public parser may itself contain private implementation types.
    let mut partial_bounds = Vec::new();
    let partial_types = command
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| match &field.semantics {
            model::FieldSemantics::Flatten => {
                let associated = associated_fields[index]
                    .as_ref()
                    .expect("flatten fields always have a generated shape association");
                partial_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::CommandArgs
                ));
                quote!(
                    <<T as #shape_ident>::#associated as
                        #facade::__private::CommandArgs>::Partial
                )
            }
            model::FieldSemantics::Subcommand => {
                let associated = associated_fields[index]
                    .as_ref()
                    .expect("subcommand fields always have a generated shape association");
                partial_bounds.push(quote!(
                    <T as #shape_ident>::#associated: #facade::__private::Subcommands
                ));
                quote!(
                    <<T as #shape_ident>::#associated as
                        #facade::__private::Subcommands>::Partial
                )
            }
            _ if field.argument().is_some_and(|argument| argument.shape == model::Shape::Many) => {
                quote!(::std::vec::Vec<::std::vec::Vec<u8>>)
            }
            _ if field.is_switch() => quote!((bool, bool)),
            _ => quote!((::std::option::Option<#facade::__private::RawValue>, bool)),
        })
        .collect::<Vec<_>>();
    let has_nested_partial = command.fields.iter().any(|field| {
        matches!(
            &field.semantics,
            model::FieldSemantics::Flatten | model::FieldSemantics::Subcommand
        )
    });
    let (partial, partial_constructor, partial_declaration) = if has_nested_partial {
        (
            quote!(#partial_ident<Self>),
            Some(partial_ident.clone()),
            quote! {
                #[doc = "Argx-generated opaque parser partial state."]
                #[doc(hidden)]
                #[allow(
                    missing_copy_implementations,
                    missing_debug_implementations,
                    private_bounds,
                    unreachable_pub,
                    unnameable_types,
                    reason = "generated partial witness hides private composed command types"
                )]
                #visibility struct #partial_ident<T>(#(#partial_types,)*)
                where
                    T: #shape_ident,
                    #(#partial_bounds,)*;
            },
        )
    } else {
        (quote!((#(#partial_types,)*)), None, TokenStream::new())
    };

    let fields = if has_fields {
        quote!(#fields_ident<Self>)
    } else {
        quote!(#facade::__private::NoTypeProjection)
    };
    let field_declaration = has_fields.then(|| {
        let field_where = if needs_shape {
            quote! {
                where
                    T: #shape_ident,
                    #(#field_bounds,)*
            }
        } else {
            TokenStream::new()
        };
        let privacy_allow = needs_shape.then(|| {
            quote! {
                #[allow(
                    private_bounds,
                    reason = "generated contract witnesses intentionally hide concrete value types"
                )]
            }
        });
        quote! {
            #[doc = "Argx-generated semantic contract witness."]
            #[doc(hidden)]
            #[derive(Debug, Clone, Copy)]
            #[allow(
                unreachable_pub,
                unnameable_types,
                reason = "generated witness is exposed only through Argx's hidden projection trait"
            )]
            #visibility struct #fields_ident<T>(::core::marker::PhantomData<fn() -> T>);

            #privacy_allow
            impl<T> #facade::__private::ResolveValueFields for #fields_ident<T>
            #field_where
            {
                fn append(
                    resolver: &mut #facade::__private::TypeResolver,
                    values: &mut #facade::__private::CommandValueTypes,
                ) {
                    #(#field_steps)*
                }
            }
        }
    });

    let directly_invocable = subcommand_projection.is_none();
    let (subcommands, subcommand_declaration) = subcommand_projection.map_or_else(
        || (quote!(#facade::__private::NoTypeProjection), None),
        |associated| {
            let projection = quote!(#subcommands_ident<Self>);
            let declaration = quote! {
                #[doc = "Argx-generated semantic contract witness."]
                #[doc(hidden)]
                #[derive(Debug, Clone, Copy)]
                #[allow(
                    unreachable_pub,
                    unnameable_types,
                    reason = "generated witness is exposed only through Argx's hidden projection trait"
                )]
                #visibility struct #subcommands_ident<T>(::core::marker::PhantomData<fn() -> T>);

                #[allow(
                    private_bounds,
                    reason = "generated contract witnesses intentionally hide concrete value types"
                )]
                impl<T> #facade::__private::ResolveSubcommands for #subcommands_ident<T>
                where
                    T: #shape_ident,
                    <T as #shape_ident>::#associated:
                        #facade::__private::SubcommandTypeContract,
                    <<T as #shape_ident>::#associated as
                        #facade::__private::SubcommandTypeContract>::Commands:
                        #facade::__private::ResolveSubcommandTree,
                {
                    fn resolve(
                        index: usize,
                        rest: &[usize],
                        resolver: &mut #facade::__private::TypeResolver,
                    ) -> ::std::option::Option<#facade::__private::CommandTypes> {
                        <<<T as #shape_ident>::#associated as
                            #facade::__private::SubcommandTypeContract>::Commands as
                            #facade::__private::ResolveSubcommandTree>::resolve(
                                index,
                                rest,
                                resolver,
                            )
                    }
                }
            };
            (projection, Some(declaration))
        },
    );

    let (execution, execution_declaration) = if directly_invocable {
        (
            quote!(#execution_ident<Self>),
            Some(quote! {
                #[doc = "Argx-generated execution contract witness."]
                #[doc(hidden)]
                #[derive(Debug, Clone, Copy)]
                #[allow(
                    unreachable_pub,
                    unnameable_types,
                    reason = "generated witness is exposed only through Argx's hidden projection trait"
                )]
                #visibility struct #execution_ident<T>(::core::marker::PhantomData<fn() -> T>);

                impl<T> #facade::__private::ResolveExecutionContract for #execution_ident<T>
                where
                    T: #facade::__private::ExecutionContractSource,
                {
                    fn resolve(
                        resolver: &mut #facade::__private::TypeResolver,
                    ) -> ::std::option::Option<#facade::__private::CommandExecutionTypes> {
                        ::std::option::Option::Some(
                            <T as #facade::__private::ExecutionContractSource>::resolve_execution(
                                resolver,
                            ),
                        )
                    }
                }
            }),
        )
    } else {
        (quote!(#facade::__private::NoTypeProjection), None)
    };

    SemanticProjection {
        declarations: quote! {
            #shape
            #partial_declaration
            #field_declaration
            #subcommand_declaration
            #execution_declaration
        },
        partial,
        partial_constructor,
        fields,
        execution,
        subcommands,
    }
}

/// Returns the deterministic generated partial-state constructor for one command declaration.
fn partial_type_constructor(command: &model::Command) -> proc_macro2::Ident {
    let suffix = command.binding.ident.to_string().trim_start_matches("r#").to_owned();
    let declaration = key::declaration_hash(&command.binding.fingerprint);
    format_ident!("__ArgxPartialFor{}H{}", suffix, declaration)
}

/// Adds the conversion bounds required by generated typed binding.
fn binding_generics(command: &model::Command, facade: &TokenStream) -> Generics {
    let mut generics = command.binding.generics.clone();
    let mut bounded = Vec::new();
    for field in &command.fields {
        match &field.semantics {
            model::FieldSemantics::Flatten => {
                let ty = &field.binding.ty;
                generics.make_where_clause().predicates.push(parse_quote!(#ty: #facade::Args));
                continue;
            }
            model::FieldSemantics::Subcommand => {
                let ty = &field.binding.ty;
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: #facade::__private::Subcommands));
                continue;
            }
            _ => {}
        }
        if field.is_switch() {
            continue;
        }
        let value = field.value_binding();
        let ty = &value.ty;
        let rendered = quote!(#ty).to_string();
        if bounded.contains(&rendered) {
            continue;
        }
        bounded.push(rendered);

        match value.conversion {
            model::ValueConversion::Text => {}
            model::ValueConversion::Os => {
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: ::std::convert::From<::std::ffi::OsString>));
            }
            model::ValueConversion::FromStr => {
                let where_clause = generics.make_where_clause();
                where_clause.predicates.push(parse_quote!(#ty: ::std::str::FromStr));
                where_clause.predicates.push(parse_quote!(
                    <#ty as ::std::str::FromStr>::Err: ::std::fmt::Display
                ));
            }
        }
    }
    generics
}

/// Generates one key-selected, exact-metadata event-dispatch arm for a field.
///
/// The key selects the candidate arm directly; pointer identity keeps a hash collision harmless.
fn apply_arm(
    field: &model::Field,
    field_index: usize,
    table: &proc_macro2::Ident,
    key: &proc_macro2::Ident,
    flag: bool,
    facade: &TokenStream,
) -> TokenStream {
    let slot = syn::Index::from(field_index);
    let event_meta = if flag { quote!(flag) } else { quote!(arg) };

    if field.is_switch() {
        return quote! {
            #key if ::core::ptr::eq(#event_meta, &#table) => {
                if partial.#slot.0 {
                    partial.#slot.1 = true;
                } else {
                    partial.#slot.0 = true;
                }
                true
            },
        };
    }

    let raw_value = if flag {
        quote! {
            let ::std::option::Option::Some(value) = value else {
                unreachable!("value-taking flag event did not contain a value");
            };
        }
    } else {
        TokenStream::new()
    };

    if field.argument().is_some_and(|argument| argument.shape == model::Shape::Many) {
        quote! {
            #key if ::core::ptr::eq(#event_meta, &#table) => {
                #raw_value
                partial.#slot.push(value.to_vec());
                true
            },
        }
    } else {
        quote! {
            #key if ::core::ptr::eq(#event_meta, &#table) => {
                #raw_value
                if partial.#slot.0.is_some() {
                    partial.#slot.1 = true;
                } else {
                    partial.#slot.0 = ::std::option::Option::Some(
                        #facade::__private::RawValue::Argv(value.to_vec()),
                    );
                }
                true
            },
        }
    }
}

/// Generates one semantic argument-presence lookup branch.
fn argument_state_branch(
    field: &model::Field,
    field_index: usize,
    key: &proc_macro2::Ident,
    facade: &TokenStream,
) -> TokenStream {
    let argument = field.argument().expect("state branch requires argument semantics");
    let slot = syn::Index::from(field_index);
    let diagnostic = &argument.diagnostic;
    let given = if field.is_switch() {
        quote!(partial.#slot.0)
    } else if argument.shape == model::Shape::Many {
        quote!(!partial.#slot.is_empty())
    } else {
        quote!(partial.#slot.0.is_some())
    };
    let satisfied = if argument.has_default { quote!(true) } else { given.clone() };

    quote! {
        if key == #key {
            let given = #given;
            return ::std::option::Option::Some(#facade::__private::ArgumentState {
                diagnostic: #diagnostic,
                given,
                satisfied: #satisfied,
            });
        }
    }
}

/// Generates final conversion for one destination field.
fn finish_field(field: &model::Field, field_index: usize, facade: &TokenStream) -> TokenStream {
    let slot = syn::Index::from(field_index);

    if matches!(&field.semantics, model::FieldSemantics::Flatten) {
        let ty = &field.binding.ty;
        return quote!(<#ty as #facade::__private::CommandArgs>::finish(partial.#slot)?);
    }
    if matches!(&field.semantics, model::FieldSemantics::Subcommand) {
        let ty = &field.binding.ty;
        let name = &field.binding.name;
        return quote! {
            {
                let ::std::option::Option::Some(value) =
                    <#ty as #facade::__private::Subcommands>::finish(partial.#slot)?
                else {
                    return ::std::result::Result::Err(
                        #facade::Error::MissingSubcommand { name: #name },
                    );
                };
                value
            }
        };
    }

    if field.is_switch() {
        return quote!(partial.#slot.0);
    }

    let binding = field.value_binding();
    let ty = &binding.ty;
    let argument = field.argument().expect("value field must have argument semantics");
    let name = &argument.diagnostic;
    // Type-check the user's default expression directly against the value type. Relying on
    // surrounding branch unification instead would make rustc diagnose generated `match` arms
    // rather than the `default = ...` expression the user can actually fix.
    let default = field.binding.default.as_ref().map(|default| {
        quote!({
            let __argx_default: #ty = #default;
            __argx_default
        })
    });
    let one = |value: TokenStream| match binding.conversion {
        model::ValueConversion::Text => quote!(#facade::__private::text_value(#value, #name)?),
        model::ValueConversion::Os => {
            quote!(#facade::__private::os_value::<#ty>(#value, #name)?)
        }
        model::ValueConversion::FromStr => {
            quote!(#facade::__private::parsed_value::<#ty>(#value, #name)?)
        }
    };
    let many = |value: TokenStream| match binding.conversion {
        model::ValueConversion::Text => quote!(#facade::__private::text_values(#value, #name)?),
        model::ValueConversion::Os => {
            quote!(#facade::__private::os_values::<#ty>(#value, #name)?)
        }
        model::ValueConversion::FromStr => {
            quote!(#facade::__private::parsed_values::<#ty>(#value, #name)?)
        }
    };

    match argument.shape {
        model::Shape::Bool | model::Shape::Required => {
            let converted = one(quote!(value));
            default.as_ref().map_or_else(
                || {
                    quote! {
                        {
                            let ::std::option::Option::Some(value) = partial.#slot.0 else {
                                return ::std::result::Result::Err(
                                    #facade::Error::MissingRequired { name: #name },
                                );
                            };
                            #converted
                        }
                    }
                },
                |default| {
                    quote! {
                        match partial.#slot.0 {
                            ::std::option::Option::Some(value) => #converted,
                            ::std::option::Option::None => #default,
                        }
                    }
                },
            )
        }
        model::Shape::Optional => {
            let converted = one(quote!(value));
            default.as_ref().map_or_else(
                || {
                    quote! {
                        match partial.#slot.0 {
                            ::std::option::Option::Some(value) => {
                                ::std::option::Option::Some(#converted)
                            }
                            ::std::option::Option::None => ::std::option::Option::None,
                        }
                    }
                },
                |default| {
                    quote! {
                        match partial.#slot.0 {
                            ::std::option::Option::Some(value) => {
                                ::std::option::Option::Some(#converted)
                            }
                            ::std::option::Option::None => {
                                ::std::option::Option::Some(#default)
                            }
                        }
                    }
                },
            )
        }
        model::Shape::Many if binding.optional_collection => {
            let converted = many(quote!(partial.#slot));
            quote! {
                if partial.#slot.is_empty() {
                    ::std::option::Option::None
                } else {
                    ::std::option::Option::Some(#converted)
                }
            }
        }
        model::Shape::Many => many(quote!(partial.#slot)),
    }
}
