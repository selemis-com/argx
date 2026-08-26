//! Code generation for `Parser` and `Args` structs.
//!
//! One normalized command produces three coordinated artifacts: static runtime parse/help tables,
//! static machine-contract tables, and typed partial-state binding code. Flattened `Args` children
//! are composed into the first two at compile time while retaining nested Rust values in the typed
//! state. This is intentionally verbose generation: semantics should be decided in `model`, not
//! rediscovered from emitted tables.

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

/// Generated static machine-contract table expressions for one command declaration.
#[derive(Debug)]
struct ContractTables {
    /// Declarations required to compose flattened child contract tables.
    decls: TokenStream,
    /// Final named-argument contract slice expression.
    flags: TokenStream,
    /// Final positional-argument contract slice expression.
    args: TokenStream,
    /// Final normalized constraint slice expression.
    constraints: TokenStream,
}

/// Generates static parse metadata and typed binding for one command struct.
pub(crate) fn command(command: &model::Command) -> TokenStream {
    let facade = crate_name::facade_path();
    let ident = &command.binding.ident;
    let binding_generics = binding_generics(command, &facade);
    let (impl_generics, ty_generics, where_clause) = binding_generics.split_for_impl();

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

    // Runtime and contract tables are generated side by side from the same normalized argument.
    // They intentionally differ in shape but must never reinterpret attributes independently.
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
        let required = argument.shape == model::Shape::Required
            && argument.env.is_none()
            && !argument.has_default;
        let required_if_env_unset = argument.shape == model::Shape::Required
            && argument.env.is_some()
            && !argument.has_default;
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
                required: #required,
                required_if_env_unset: #required_if_env_unset,
                allow_hyphen_values: #allow_hyphen_values,
                allow_negative_numbers: #allow_negative_numbers,
            };
        }
    });

    let contract_flag_tables = flags.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_CONTRACT_FLAG_{index}");
        let key = key::ident("FLAG", Some(index));
        let Some(argument) = field.argument() else {
            unreachable!("flag list only contains argument fields");
        };
        let model::ArgumentKind::Flag { longs, aliases, shorts } = &argument.kind else {
            unreachable!("flag list only contains named arguments");
        };
        let name = &field.binding.name;
        let help = option_str(argument.help.as_deref());
        let global = argument.global;
        let env = option_str(argument.env.as_deref());
        let cardinality = contract_cardinality(argument.shape, field.is_switch(), &facade);
        let required = argument.shape == model::Shape::Required && !argument.has_default;
        let has_default = argument.has_default;
        let allow_hyphen_values = argument.allow_hyphen_values;
        let allow_negative_numbers = argument.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::FlagSpec<'static> =
                #facade::__private::FlagSpec {
                    key: #key,
                    name: #name,
                    help: #help,
                    longs: &[#(#longs),*],
                    aliases: &[#(#aliases),*],
                    shorts: &[#(#shorts),*],
                    global: #global,
                    env: #env,
                    cardinality: #cardinality,
                    required: #required,
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
    let contract_arg_tables = args.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_CONTRACT_ARG_{index}");
        let key = key::ident("ARG", Some(index));
        let Some(argument) = field.argument() else {
            unreachable!("positional list only contains argument fields");
        };
        let name = &field.binding.name;
        let help = option_str(argument.help.as_deref());
        let cardinality = contract_cardinality(argument.shape, false, &facade);
        let required = matches!(argument.shape, model::Shape::Bool | model::Shape::Required)
            && !argument.has_default;
        let has_default = argument.has_default;
        let allow_negative_numbers = argument.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::ArgSpec<'static> =
                #facade::__private::ArgSpec {
                    key: #key,
                    name: #name,
                    help: #help,
                    cardinality: #cardinality,
                    required: #required,
                    has_default: #has_default,
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
    let contract_tables =
        contract_tables(command, &facade, flags.len(), args.len(), constraint_count);
    let contract_table_decls = &contract_tables.decls;
    let contract_flags = &contract_tables.flags;
    let contract_args = &contract_tables.args;
    let contract_constraints = &contract_tables.constraints;
    let constraint_tables = constraint_tables(command, &facade, command_flags, command_args);

    // Partial state mirrors Rust field order. Direct arguments accumulate raw bytes, flattened
    // declarations retain their own partial state, and subcommands retain branch-selection state.
    let partial_types = command.fields.iter().map(|field| match &field.semantics {
        model::FieldSemantics::Flatten => {
            let ty = &field.binding.ty;
            quote!(<#ty as #facade::__private::CommandArgs>::Partial)
        }
        model::FieldSemantics::Subcommand => {
            let ty = &field.binding.ty;
            quote!(<#ty as #facade::__private::Subcommands>::Partial)
        }
        _ if field.argument().is_some_and(|argument| argument.shape == model::Shape::Many) => {
            quote!(::std::vec::Vec<::std::vec::Vec<u8>>)
        }
        _ if field.is_switch() => quote!((bool, bool)),
        _ => quote!((::std::option::Option<#facade::__private::RawValue>, bool)),
    });
    let partial_start = command.fields.iter().map(|field| match &field.semantics {
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
    });
    let partial_type = quote!((#(#partial_types,)*));
    let partial_value =
        if command.fields.is_empty() { TokenStream::new() } else { quote!((#(#partial_start,)*)) };

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
    let contract_subcommands = subcommand.map_or_else(
        || quote!(&[]),
        |(_, field)| {
            let ty = &field.binding.ty;
            quote!(<#ty as #facade::__private::Subcommands>::CONTRACTS)
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
        #[doc(hidden)]
        const _: () = {
            #keys
            #(#flag_tables)*
            #(#arg_tables)*
            #(#contract_flag_tables)*
            #(#contract_arg_tables)*
            #table_decls
            #contract_table_decls
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

            static ARGX_CONTRACT_COMMAND: #facade::__private::CommandSpec<'static> =
                #facade::__private::CommandSpec {
                    name: #name,
                    about: #about,
                    aliases: &[#(#aliases),*],
                    flags: #contract_flags,
                    args: #contract_args,
                    constraints: #contract_constraints,
                    subcommands: #contract_subcommands,
                };

            const _: () = ::core::assert!(
                #facade::__private::action_flag_spellings_disjoint(
                    ARGX_COMMAND.actions,
                    ARGX_COMMAND.flags,
                ),
                "command contains a flag spelling reserved by a built-in action",
            );
            #flattened_checks

            impl #impl_generics #facade::__private::CommandContract
                for #ident #ty_generics #where_clause
            {
                const CONTRACT: &'static #facade::__private::CommandSpec<'static> =
                    &ARGX_CONTRACT_COMMAND;
            }

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

/// Maps one normalized Rust value shape into the private contract vocabulary.
fn contract_cardinality(shape: model::Shape, switch: bool, facade: &TokenStream) -> TokenStream {
    if switch {
        return quote!(#facade::__private::Cardinality::Switch);
    }
    match shape {
        model::Shape::Bool | model::Shape::Required => {
            quote!(#facade::__private::Cardinality::One)
        }
        model::Shape::Optional => quote!(#facade::__private::Cardinality::Optional),
        model::Shape::Many => quote!(#facade::__private::Cardinality::Many),
    }
}

/// Builds one flat machine-contract table from direct fields and flattened children.
fn contract_tables(
    command: &model::Command,
    facade: &TokenStream,
    flag_count: usize,
    arg_count: usize,
    constraint_count: usize,
) -> ContractTables {
    if !command.fields.iter().any(model::Field::is_flatten) {
        let flags = (0..flag_count).map(|index| {
            let table = format_ident!("ARGX_CONTRACT_FLAG_{index}");
            quote!(&#table)
        });
        let args = (0..arg_count).map(|index| {
            let table = format_ident!("ARGX_CONTRACT_ARG_{index}");
            quote!(&#table)
        });
        let constraints = (0..constraint_count).map(|index| {
            let table = format_ident!("ARGX_CONSTRAINT_{index}");
            quote!(#table)
        });
        return ContractTables {
            decls: TokenStream::new(),
            flags: quote!(&[#(#flags),*]),
            args: quote!(&[#(#args),*]),
            constraints: quote!(&[#(#constraints),*]),
        };
    }

    let mut flag_groups = Vec::new();
    let mut arg_groups = Vec::new();
    let mut constraint_groups = Vec::new();
    let mut own_flags = Vec::new();
    let mut own_args = Vec::new();
    let mut own_constraints = Vec::new();
    let mut flag_at = 0_usize;
    let mut arg_at = 0_usize;
    let mut constraint_at = 0_usize;

    fn flush_flags(own: &mut Vec<usize>, groups: &mut Vec<TokenStream>) {
        if own.is_empty() {
            return;
        }
        let refs = own.iter().map(|index| {
            let table = format_ident!("ARGX_CONTRACT_FLAG_{index}");
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
            let table = format_ident!("ARGX_CONTRACT_ARG_{index}");
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
                flag_groups
                    .push(quote!(<#ty as #facade::__private::CommandContract>::CONTRACT.flags));
                arg_groups
                    .push(quote!(<#ty as #facade::__private::CommandContract>::CONTRACT.args));
                constraint_groups.push(
                    quote!(<#ty as #facade::__private::CommandContract>::CONTRACT.constraints),
                );
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

    ContractTables {
        decls: quote! {
            const ARGX_CONTRACT_FLAG_GROUPS:
                &[&[&#facade::__private::FlagSpec<'static>]] = &[#(#flag_groups),*];
            const ARGX_CONTRACT_ARG_GROUPS:
                &[&[&#facade::__private::ArgSpec<'static>]] = &[#(#arg_groups),*];
            static ARGX_CONTRACT_FLAGS: [&#facade::__private::FlagSpec<'static>;
                #facade::__private::table_len(ARGX_CONTRACT_FLAG_GROUPS)] =
                #facade::__private::concat_contract_flags(ARGX_CONTRACT_FLAG_GROUPS);
            static ARGX_CONTRACT_ARGS: [&#facade::__private::ArgSpec<'static>;
                #facade::__private::table_len(ARGX_CONTRACT_ARG_GROUPS)] =
                #facade::__private::concat_contract_args(ARGX_CONTRACT_ARG_GROUPS);
            const ARGX_CONTRACT_CONSTRAINT_GROUPS: &[&[#facade::__private::Constraint]] =
                &[#(#constraint_groups),*];
            static ARGX_CONTRACT_CONSTRAINTS: [#facade::__private::Constraint;
                #facade::__private::table_len(ARGX_CONTRACT_CONSTRAINT_GROUPS)] =
                #facade::__private::concat_constraints(ARGX_CONTRACT_CONSTRAINT_GROUPS);
        },
        flags: quote!(&ARGX_CONTRACT_FLAGS),
        args: quote!(&ARGX_CONTRACT_ARGS),
        constraints: quote!(&ARGX_CONTRACT_CONSTRAINTS),
    }
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
