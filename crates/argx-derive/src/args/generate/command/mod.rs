//! Code generation for `Parser` and `Args` structs.
//!
//! One normalized command produces static command metadata and typed partial-state binding code.
//! Flattened `Args` children are composed into static metadata while retaining nested Rust values
//! in typed state. Generated nominal witnesses keep private composed types out of the public derive
//! ABI. Semantics are decided in `model`; this module only coordinates the individual
//! code-generation projections.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::option_str;
use crate::{
    args::{key, model},
    support,
};

mod binding;
mod metadata;
mod projection;

use binding::{apply_arm, argument_state_branch, binding_generics, finish_field};
use metadata::{command_tables, constraint_tables};
use projection::partial_projection;

/// Projects one normalized value type into runtime schema metadata.
fn value_schema(field: &model::Field, facade: &TokenStream) -> TokenStream {
    match field.binding.value.as_ref().map(|binding| binding.schema) {
        Some(model::ValueSchema::Date) => quote!(#facade::__private::ValueSchema::Date),
        Some(model::ValueSchema::DateTime) => quote!(#facade::__private::ValueSchema::DateTime),
        Some(model::ValueSchema::Uuid) => quote!(#facade::__private::ValueSchema::Uuid),
        Some(model::ValueSchema::Url) => quote!(#facade::__private::ValueSchema::Url),
        Some(model::ValueSchema::Lexical) | None => {
            quote!(#facade::__private::ValueSchema::Lexical)
        }
    }
}

/// Generates static parse metadata and typed binding for one command struct.
pub(crate) fn command(command: &model::Command) -> TokenStream {
    let facade = support::facade_path();
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
    let metadata = command
        .semantics
        .metadata
        .iter()
        .map(|entry| metadata_entry_tokens(entry, &facade))
        .collect::<Vec<_>>();

    // Static command metadata is generated once from the normalized argument model and is shared
    // by parsing and help generation.
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
        let long_help = option_str(argument.long_help.as_deref());
        let global = argument.global;
        let takes_value = field.takes_value();
        let accepted_values = if argument.value_enum {
            let ty = &field.value_binding().ty;
            quote!(<#ty as #facade::ValueEnum>::VALUES)
        } else {
            quote!(&[])
        };
        let value_schema = value_schema(field, &facade);
        let repeatable = field.is_repeatable();
        let required = argument.shape == model::Shape::Required
            && !argument.has_default
            && field.takes_value();
        let has_default = argument.has_default;
        let default_value = option_str(argument.default_value.as_deref());
        let allow_hyphen_values = argument.allow_hyphen_values;
        let allow_negative_numbers = argument.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Flag<'static> = #facade::__private::Flag {
                key: #key,
                name: #name,
                diagnostic: #diagnostic,
                help: #help,
                long_help: #long_help,
                longs: &[#(#longs),*],
                aliases: &[#(#aliases),*],
                shorts: &[#(#shorts),*],
                global: #global,
                takes_value: #takes_value,
                accepted_values: #accepted_values,
                value_schema: #value_schema,
                repeatable: #repeatable,
                required: #required,
                has_default: #has_default,
                default_value: #default_value,
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
        let long_help = option_str(argument.long_help.as_deref());
        let required = matches!(argument.shape, model::Shape::Bool | model::Shape::Required);
        let variadic = argument.shape == model::Shape::Many;
        let accepted_values = if argument.value_enum {
            let ty = &field.value_binding().ty;
            quote!(<#ty as #facade::ValueEnum>::VALUES)
        } else {
            quote!(&[])
        };
        let value_schema = value_schema(field, &facade);
        let allow_negative_numbers = argument.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Arg<'static> =
                #facade::__private::Arg {
                    key: #key,
                    name: #name,
                    help: #help,
                    long_help: #long_help,
                    required: #required,
                    variadic: #variadic,
                    accepted_values: #accepted_values,
                    value_schema: #value_schema,
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
    let partial_projection = partial_projection(command, &facade);
    let partial_declarations = &partial_projection.declarations;
    let partial_type = &partial_projection.partial;
    let invocable_handler_impl = subcommand.is_none().then(|| {
        quote! {
            impl #impl_generics #facade::__private::InvocableCommandHandler
                for #ident #ty_generics #where_clause
            {}
        }
    });
    let schema_command_impl = (!command.binding.root && command.semantics.schema).then(|| {
        let (_, field) = subcommand.expect("schema-enabled Args must contain a subcommand field");
        let ty = &field.binding.ty;
        quote! {
            impl #impl_generics #facade::__private::SchemaCommand
                for #ident #ty_generics #where_clause
            {
                fn register_schema_commands(
                    command: &'static #facade::__private::Command<'static>,
                    registry: &mut #facade::__private::SchemaRegistry,
                ) {
                    <#ty as #facade::__private::SchemaSubcommands>::register_schema_subcommands(
                        command.subcommands,
                        registry,
                    );
                }
            }
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
            _ if field.is_count() => {
                let ty = &field.binding.ty;
                quote!(0 as #ty)
            }
            _ if field.is_switch() => quote!((false, false)),
            _ => quote!((::std::option::Option::None, false)),
        })
        .collect::<Vec<_>>();
    let partial_value = partial_projection.partial_constructor.as_ref().map_or_else(
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
        apply_arm(field, *field_index, &table, &key, true)
    });
    let arg_apply = args.iter().enumerate().map(|(index, (field_index, field))| {
        let table = format_ident!("ARGX_ARG_{index}");
        let key = key::ident("ARG", Some(index));
        apply_arm(field, *field_index, &table, &key, false)
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
                    && argument.shape != model::Shape::Many
                    && !argument.count =>
            {
                let slot = syn::Index::from(field_index);
                let name = &argument.diagnostic;
                Some(quote! {
                    if partial.#slot.1 {
                        return ::std::result::Result::Err(
                            #facade::Error::DuplicateArgument { name: #name, usage: None },
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
                    && !field.is_count()
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
    let schema_property_check = (command.semantics.schema && subcommand.is_some()).then(|| {
        quote! {
            const _: () = ::core::assert!(
                #facade::__private::schema_property_names_disjoint(
                    ARGX_COMMAND.args,
                    ARGX_COMMAND.subcommands,
                ),
                "schema-enabled command contains a positional argument matching a subcommand name",
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
    let schema_enabled = command.binding.root && command.semantics.schema;
    let schema_registry = if schema_enabled {
        subcommand.map_or_else(
            || {
                quote! {
                    fn schema_registry() -> ::std::option::Option<#facade::__private::SchemaRegistry> {
                        let mut registry = #facade::__private::SchemaRegistry::new();
                        <Self as #facade::__private::SchemaCommand>::register_schema_commands(
                            Self::COMMAND,
                            &mut registry,
                        );
                        ::std::option::Option::Some(registry)
                    }
                }
            },
            |(_, field)| {
                let ty = &field.binding.ty;
                quote! {
                    fn schema_registry() -> ::std::option::Option<#facade::__private::SchemaRegistry> {
                        let mut registry = #facade::__private::SchemaRegistry::new();
                        <#ty as #facade::__private::SchemaSubcommands>::register_schema_subcommands(
                            Self::COMMAND.subcommands,
                            &mut registry,
                        );
                        ::std::option::Option::Some(registry)
                    }
                }
            },
        )
    } else {
        TokenStream::new()
    };
    let parser_impl = command.binding.root.then(|| {
        quote! {
            impl #impl_generics #facade::Parser for #ident #ty_generics #where_clause {}
        }
    });
    let args_impl = (!command.binding.root).then(|| {
        quote! {
            impl #impl_generics #facade::__private::Args
                for #ident #ty_generics #where_clause
            {}
        }
    });

    // A private const namespace keeps all generated statics and assertions local to the derived
    // declaration while still allowing trait associated constants to point at `'static` tables.
    quote! {
        #partial_declarations

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
                    metadata: &[#(#metadata),*],
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
            #schema_property_check

            #invocable_handler_impl
            #schema_command_impl

            impl #impl_generics #facade::__private::CommandArgs
                for #ident #ty_generics #where_clause
            {
                type Partial = #partial_type;

                const COMMAND: &'static #facade::__private::Command<'static> = &ARGX_COMMAND;
                const SCHEMA_ENABLED: bool = #schema_enabled;

                #schema_registry

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
                        #facade::__private::Event::Command { .. }
                        | #facade::__private::Event::Action { .. } => false,
                    };
                    if matched {
                        return true;
                    }
                    #(#flattened_apply)*
                    #subcommand_apply
                    false
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

/// Projects one normalized metadata entry into the runtime static command vocabulary.
pub(super) fn metadata_entry_tokens(
    entry: &crate::args::metadata::MetadataEntry,
    facade: &TokenStream,
) -> TokenStream {
    metadata_pair_tokens(&entry.key, &entry.value, facade)
}

/// Projects one metadata key/value pair into the runtime static command vocabulary.
fn metadata_pair_tokens(key: &str, value: &serde_json::Value, facade: &TokenStream) -> TokenStream {
    let value = metadata_value_tokens(value, facade);
    quote! {
        #facade::__private::MetadataEntry { key: #key, value: #value }
    }
}

/// Projects one normalized JSON metadata value into a static runtime expression.
fn metadata_value_tokens(value: &serde_json::Value, facade: &TokenStream) -> TokenStream {
    use serde_json::Value;

    match value {
        Value::Null => quote!(#facade::__private::MetadataValue::Null),
        Value::Bool(value) => quote!(#facade::__private::MetadataValue::Bool(#value)),
        Value::Number(value) => value.as_i64().map_or_else(
            || {
                value.as_f64().map_or_else(
                    || {
                        unreachable!(
                            "metadata parser only accepts i64 integers and finite f64 values"
                        )
                    },
                    |value| quote!(#facade::__private::MetadataValue::Float(#value)),
                )
            },
            |value| quote!(#facade::__private::MetadataValue::Integer(#value)),
        ),
        Value::String(value) => quote!(#facade::__private::MetadataValue::String(#value)),
        Value::Array(values) => {
            let values = values.iter().map(|value| metadata_value_tokens(value, facade));
            quote!(#facade::__private::MetadataValue::Array(&[#(#values),*]))
        }
        Value::Object(entries) => {
            let entries =
                entries.iter().map(|(key, value)| metadata_pair_tokens(key, value, facade));
            quote!(#facade::__private::MetadataValue::Object(&[#(#entries),*]))
        }
    }
}
