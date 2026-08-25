//! Code generation for `Parser` and `Args` structs.

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
}

/// Generates static parse metadata and typed binding for one command struct.
pub(crate) fn command(command: &model::Command) -> TokenStream {
    let facade = crate_name::facade_path();
    let ident = &command.binding.ident;
    let binding_generics = binding_generics(command, &facade);
    let (impl_generics, ty_generics, where_clause) = binding_generics.split_for_impl();

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

    let flag_tables = flags.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_FLAG_{index}");
        let key = key::ident("FLAG", Some(index));
        let Some(argument) = field.argument() else {
            unreachable!("flag list only contains argument fields");
        };
        let model::ArgumentKind::Flag { longs, shorts } = &argument.kind else {
            unreachable!("flag list only contains named arguments");
        };
        let name = &field.binding.name;
        let help = option_str(argument.help.as_deref());
        let global = argument.global;
        let takes_value = !field.is_switch();
        let required = argument.shape == model::Shape::Required;
        let allow_hyphen_values = argument.allow_hyphen_values;
        let allow_negative_numbers = argument.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Flag<'static> = #facade::__private::Flag {
                key: #key,
                name: #name,
                help: #help,
                longs: &[#(#longs),*],
                shorts: &[#(#shorts),*],
                global: #global,
                takes_value: #takes_value,
                required: #required,
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
    let tables = command_tables(command, &facade, flags.len(), args.len());
    let table_decls = &tables.decls;
    let command_flags = &tables.flags;
    let command_args = &tables.args;

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
        _ => quote!((::std::option::Option<::std::vec::Vec<u8>>, bool)),
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

    let flag_apply = flags.iter().enumerate().map(|(index, (field_index, field))| {
        let key = key::ident("FLAG", Some(index));
        apply_arm(field, *field_index, &key, true)
    });
    let arg_apply = args.iter().enumerate().map(|(index, (field_index, field))| {
        let key = key::ident("ARG", Some(index));
        apply_arm(field, *field_index, &key, false)
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
                let name = &field.binding.name;
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
            _ if field.is_switch()
                || !field.argument().is_some_and(|argument| {
                    matches!(argument.shape, model::Shape::Bool | model::Shape::Required)
                }) =>
            {
                None
            }
            _ => {
                let slot = syn::Index::from(field_index);
                let name = &field.binding.name;
                Some(quote! {
                    if partial.#slot.0.is_none() {
                        return ::std::result::Result::Err(
                            #facade::Error::MissingRequired { name: #name },
                        );
                    }
                })
            }
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
    let short_version =
        command.semantics.version.as_ref().or(command.semantics.long_version.as_ref());
    let long_version =
        command.semantics.long_version.as_ref().or(command.semantics.version.as_ref());
    let version_action = short_version.zip(long_version).map(|(short, long)| {
        quote! {
            static ARGX_VERSION_ACTION: #facade::__private::Action<'static> =
                #facade::__private::Action {
                    name: "version",
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

    quote! {
        #[doc(hidden)]
        const _: () = {
            #keys
            #(#flag_tables)*
            #(#arg_tables)*
            #table_decls
            #version_action

            static ARGX_COMMAND: #facade::__private::Command<'static> =
                #facade::__private::Command {
                    name: #name,
                    about: #about,
                    actions: #command_actions,
                    flags: #command_flags,
                    args: #command_args,
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

/// Builds one flat parse table from this declaration's own fields and flattened children.
fn command_tables(
    command: &model::Command,
    facade: &TokenStream,
    flag_count: usize,
    arg_count: usize,
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
        return Tables {
            decls: TokenStream::new(),
            flags: quote!(&[#(#flags),*]),
            args: quote!(&[#(#args),*]),
        };
    }

    let mut flag_groups = Vec::new();
    let mut arg_groups = Vec::new();
    let mut own_flags = Vec::new();
    let mut own_args = Vec::new();
    let mut flag_at = 0_usize;
    let mut arg_at = 0_usize;
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

    for field in &command.fields {
        match &field.semantics {
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Flag { .. },
                ..
            }) => {
                own_flags.push(flag_at);
                flag_at += 1;
            }
            model::FieldSemantics::Argument(model::Argument {
                kind: model::ArgumentKind::Positional,
                ..
            }) => {
                own_args.push(arg_at);
                arg_at += 1;
            }
            model::FieldSemantics::Flatten => {
                let ty = &field.binding.ty;
                flush_flags(&mut own_flags, &mut flag_groups);
                flush_args(&mut own_args, &mut arg_groups);
                flag_groups.push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.flags));
                arg_groups.push(quote!(<#ty as #facade::__private::CommandArgs>::COMMAND.args));
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

    debug_assert_eq!(flag_at, flag_count);
    debug_assert_eq!(arg_at, arg_count);

    Tables {
        decls: quote! {
            #(#flatten_checks)*
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
        },
        flags: quote!(&ARGX_FLAGS),
        args: quote!(&ARGX_ARGS),
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

/// Generates one event-dispatch arm for a field.
fn apply_arm(
    field: &model::Field,
    field_index: usize,
    key: &proc_macro2::Ident,
    flag: bool,
) -> TokenStream {
    let slot = syn::Index::from(field_index);

    if field.is_switch() {
        return quote! {
            #key => {
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
            #key => {
                #raw_value
                partial.#slot.push(value.to_vec());
                true
            },
        }
    } else {
        quote! {
            #key => {
                #raw_value
                if partial.#slot.0.is_some() {
                    partial.#slot.1 = true;
                } else {
                    partial.#slot.0 = ::std::option::Option::Some(value.to_vec());
                }
                true
            },
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
    let name = &field.binding.name;
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

    match field.argument().expect("value field must have argument semantics").shape {
        model::Shape::Bool | model::Shape::Required => {
            let converted = one(quote!(value));
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
        }
        model::Shape::Optional => {
            let converted = one(quote!(value));
            quote! {
                match partial.#slot.0 {
                    ::std::option::Option::Some(value) => {
                        ::std::option::Option::Some(#converted)
                    }
                    ::std::option::Option::None => ::std::option::Option::None,
                }
            }
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
