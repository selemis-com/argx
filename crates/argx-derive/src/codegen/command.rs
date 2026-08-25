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
    let ident = &command.ident;
    let binding_generics = binding_generics(command, &facade);
    let (impl_generics, ty_generics, where_clause) = binding_generics.split_for_impl();

    let flags = command
        .fields
        .iter()
        .enumerate()
        .filter(|(_, field)| matches!(&field.kind, model::FieldKind::Flag { .. }))
        .collect::<Vec<_>>();
    let args = command
        .fields
        .iter()
        .enumerate()
        .filter(|(_, field)| matches!(&field.kind, model::FieldKind::Positional))
        .collect::<Vec<_>>();
    let subcommand = command
        .fields
        .iter()
        .enumerate()
        .find(|(_, field)| matches!(&field.kind, model::FieldKind::Subcommand { .. }));
    let keys = key::constants(&facade, &command.fingerprint, flags.len(), args.len());
    let command_key = key::ident("COMMAND", None);

    let flag_tables = flags.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_FLAG_{index}");
        let key = key::ident("FLAG", Some(index));
        let model::FieldKind::Flag { longs, shorts } = &field.kind else {
            unreachable!("flag list only contains flag fields");
        };
        let name = &field.name;
        let help = option_str(field.help.as_deref());
        let takes_value = !field.is_switch();
        let required = field.shape == model::Shape::Required;
        let allow_hyphen_values = field.allow_hyphen_values;
        let allow_negative_numbers = field.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Flag<'static> = #facade::__private::Flag {
                key: #key,
                name: #name,
                help: #help,
                longs: &[#(#longs),*],
                shorts: &[#(#shorts),*],
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
        let name = &field.name;
        let help = option_str(field.help.as_deref());
        let required = matches!(field.shape, model::Shape::Bool | model::Shape::Required);
        let variadic = field.shape == model::Shape::Many;
        let allow_negative_numbers = field.allow_negative_numbers;
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

    let partial_types = command.fields.iter().map(|field| match &field.kind {
        model::FieldKind::Flatten { ty } => {
            quote!(<#ty as #facade::__private::CommandArgs>::Partial)
        }
        model::FieldKind::Subcommand { ty } => {
            quote!(<#ty as #facade::__private::Subcommands>::Partial)
        }
        _ if field.shape == model::Shape::Many => {
            quote!(::std::vec::Vec<::std::vec::Vec<u8>>)
        }
        _ if field.is_switch() => quote!((bool, bool)),
        _ => quote!((::std::option::Option<::std::vec::Vec<u8>>, bool)),
    });
    let partial_start = command.fields.iter().map(|field| match &field.kind {
        model::FieldKind::Flatten { ty } => {
            quote!(<#ty as #facade::__private::CommandArgs>::start())
        }
        model::FieldKind::Subcommand { ty } => {
            quote!(<#ty as #facade::__private::Subcommands>::start())
        }
        _ if field.shape == model::Shape::Many => quote!(::std::vec::Vec::new()),
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
        let model::FieldKind::Flatten { ty } = &field.kind else {
            return None;
        };
        let slot = syn::Index::from(field_index);
        Some(quote! {
            if <#ty as #facade::__private::CommandArgs>::apply(&mut partial.#slot, event) {
                return true;
            }
        })
    });
    let selected_subcommand_apply = subcommand.map(|(field_index, field)| {
        let model::FieldKind::Subcommand { ty } = &field.kind else {
            unreachable!("subcommand lookup only returns subcommand fields");
        };
        let slot = syn::Index::from(field_index);
        quote! {
            if <#ty as #facade::__private::Subcommands>::selected(&partial.#slot) {
                return <#ty as #facade::__private::Subcommands>::apply(
                    &mut partial.#slot,
                    event,
                );
            }
        }
    });
    let subcommand_apply = subcommand.map(|(field_index, field)| {
        let model::FieldKind::Subcommand { ty } = &field.kind else {
            unreachable!("subcommand lookup only returns subcommand fields");
        };
        let slot = syn::Index::from(field_index);
        quote! {
            if <#ty as #facade::__private::Subcommands>::apply(&mut partial.#slot, event) {
                return true;
            }
        }
    });

    let occurrence_checks = command
        .fields
        .iter()
        .enumerate()
        .filter_map(|(field_index, field)| match &field.kind {
            model::FieldKind::Flag { .. } if field.shape != model::Shape::Many => {
                let slot = syn::Index::from(field_index);
                let name = &field.name;
                Some(quote! {
                    if partial.#slot.1 {
                        return ::std::result::Result::Err(
                            #facade::Error::DuplicateArgument { name: #name },
                        );
                    }
                })
            }
            model::FieldKind::Flatten { ty } => {
                let slot = syn::Index::from(field_index);
                Some(quote! {
                    <#ty as #facade::__private::CommandArgs>::check_occurrences(
                        &mut partial.#slot,
                    )?;
                })
            }
            model::FieldKind::Subcommand { ty } => {
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
        .filter_map(|(field_index, field)| match &field.kind {
            model::FieldKind::Flatten { ty } => {
                let slot = syn::Index::from(field_index);
                Some(quote! {
                    <#ty as #facade::__private::CommandArgs>::check_required(
                        &mut partial.#slot,
                    )?;
                })
            }
            model::FieldKind::Subcommand { ty } => {
                let slot = syn::Index::from(field_index);
                let name = &field.name;
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
                || !matches!(field.shape, model::Shape::Bool | model::Shape::Required) =>
            {
                None
            }
            _ => {
                let slot = syn::Index::from(field_index);
                let name = &field.name;
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
        let ident = &field.ident;
        let value = finish_field(field, field_index, &facade);
        quote!(#ident: #value)
    });
    let built = if command.unit { quote!(Self) } else { quote!(Self { #(#built_fields),* }) };

    let composed_checks = command.fields.iter().any(model::Field::is_flatten).then(|| {
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

    let name = &command.name;
    let about = option_str(command.about.as_deref());
    let command_subcommands = subcommand.map_or_else(
        || quote!(&[]),
        |(_, field)| {
            let model::FieldKind::Subcommand { ty } = &field.kind else {
                unreachable!("subcommand lookup only returns subcommand fields");
            };
            quote!(<#ty as #facade::__private::Subcommands>::COMMANDS)
        },
    );
    let parser_impl = command.root.then(|| {
        quote! {
            impl #impl_generics #facade::Parser for #ident #ty_generics #where_clause {}
        }
    });
    let args_impl = (!command.root).then(|| {
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

            static ARGX_COMMAND: #facade::__private::Command<'static> =
                #facade::__private::Command {
                    name: #name,
                    about: #about,
                    flags: #command_flags,
                    args: #command_args,
                    subcommands: #command_subcommands,
                    key: #command_key,
                };

            #composed_checks

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
        match &field.kind {
            model::FieldKind::Flag { .. } => {
                own_flags.push(flag_at);
                flag_at += 1;
            }
            model::FieldKind::Positional => {
                own_args.push(arg_at);
                arg_at += 1;
            }
            model::FieldKind::Flatten { ty } => {
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
            model::FieldKind::Subcommand { .. } => {}
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
    let mut generics = command.generics.clone();
    let mut bounded = Vec::new();
    for field in &command.fields {
        match &field.kind {
            model::FieldKind::Flatten { ty } => {
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: #facade::Args));
                continue;
            }
            model::FieldKind::Subcommand { ty } => {
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
        let ty = field.value_type();
        let rendered = quote!(#ty).to_string();
        if bounded.contains(&rendered) {
            continue;
        }
        bounded.push(rendered);

        if field.string_value() {
            continue;
        }

        let where_clause = generics.make_where_clause();
        if field.os_value() {
            where_clause
                .predicates
                .push(parse_quote!(#ty: ::std::convert::From<::std::ffi::OsString>));
        } else {
            where_clause.predicates.push(parse_quote!(#ty: ::std::str::FromStr));
            where_clause.predicates.push(parse_quote!(
                <#ty as ::std::str::FromStr>::Err: ::std::fmt::Display
            ));
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

    if field.shape == model::Shape::Many {
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

    if let model::FieldKind::Flatten { ty } = &field.kind {
        return quote!(<#ty as #facade::__private::CommandArgs>::finish(partial.#slot)?);
    }
    if let model::FieldKind::Subcommand { ty } = &field.kind {
        let name = &field.name;
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

    let ty = field.value_type();
    let name = &field.name;
    let one = |value: TokenStream| {
        if field.string_value() {
            quote!(#facade::__private::text_value(#value, #name)?)
        } else if field.os_value() {
            quote!(#facade::__private::os_value::<#ty>(#value, #name)?)
        } else {
            quote!(#facade::__private::parsed_value::<#ty>(#value, #name)?)
        }
    };
    let many = |value: TokenStream| {
        if field.string_value() {
            quote!(#facade::__private::text_values(#value, #name)?)
        } else if field.os_value() {
            quote!(#facade::__private::os_values::<#ty>(#value, #name)?)
        } else {
            quote!(#facade::__private::parsed_values::<#ty>(#value, #name)?)
        }
    };

    match field.shape {
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
        model::Shape::Many if field.optional_collection() => {
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
