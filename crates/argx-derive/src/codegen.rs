//! Code generation from the validated Argx semantic model.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Generics, parse_quote};

use crate::{attrs, crate_name, key, model};

/// Generates static parse metadata and typed binding for one command struct.
pub(crate) fn command(command: &model::Command) -> TokenStream {
    let facade = crate_name::facade_path();
    let ident = &command.ident;
    let binding_generics = binding_generics(command);
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
    let keys = key::constants(&facade, &command.fingerprint, flags.len(), args.len());
    let command_key = key::ident("COMMAND", None);

    let flag_tables = flags.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_FLAG_{index}");
        let key = key::ident("FLAG", Some(index));
        let model::FieldKind::Flag { longs, shorts } = &field.kind else {
            unreachable!("flag list only contains flag fields");
        };
        let name = &field.name;
        let takes_value = !field.is_switch();
        let allow_hyphen_values = field.allow_hyphen_values;
        let allow_negative_numbers = field.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Flag<'static> = #facade::__private::Flag {
                key: #key,
                name: #name,
                longs: &[#(#longs),*],
                shorts: &[#(#shorts),*],
                takes_value: #takes_value,
                allow_hyphen_values: #allow_hyphen_values,
                allow_negative_numbers: #allow_negative_numbers,
            };
        }
    });
    let flag_refs = (0..flags.len()).map(|index| {
        let table = format_ident!("ARGX_FLAG_{index}");
        quote!(&#table)
    });

    let arg_tables = args.iter().enumerate().map(|(index, (_, field))| {
        let table = format_ident!("ARGX_ARG_{index}");
        let key = key::ident("ARG", Some(index));
        let name = &field.name;
        let required = matches!(field.shape, model::Shape::Bool | model::Shape::Required);
        let variadic = field.shape == model::Shape::Many;
        let allow_negative_numbers = field.allow_negative_numbers;
        quote! {
            static #table: #facade::__private::Arg<'static> =
                #facade::__private::Arg {
                    key: #key,
                    name: #name,
                    required: #required,
                    variadic: #variadic,
                    allow_negative_numbers: #allow_negative_numbers,
                };
        }
    });
    let arg_refs = (0..args.len()).map(|index| {
        let table = format_ident!("ARGX_ARG_{index}");
        quote!(&#table)
    });

    let partial_types = command.fields.iter().map(|field| {
        if field.shape == model::Shape::Many {
            quote!(::std::vec::Vec<::std::vec::Vec<u8>>)
        } else if field.is_switch() {
            quote!((bool, bool))
        } else {
            quote!((::std::option::Option<::std::vec::Vec<u8>>, bool))
        }
    });
    let partial_start = command.fields.iter().map(|field| {
        if field.shape == model::Shape::Many {
            quote!(::std::vec::Vec::new())
        } else if field.is_switch() {
            quote!((false, false))
        } else {
            quote!((::std::option::Option::None, false))
        }
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
    let duplicate_checks = flags
        .iter()
        .filter_map(|(field_index, field)| {
            if field.shape == model::Shape::Many {
                return None;
            }
            let slot = syn::Index::from(*field_index);
            let name = &field.name;
            Some(quote! {
                if partial.#slot.1 {
                    return ::std::result::Result::Err(
                        #facade::Error::DuplicateArgument { name: #name },
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    let required_checks = command
        .fields
        .iter()
        .enumerate()
        .filter_map(|(field_index, field)| {
            if field.is_switch()
                || !matches!(field.shape, model::Shape::Bool | model::Shape::Required)
            {
                return None;
            }
            let slot = syn::Index::from(field_index);
            let name = &field.name;
            Some(quote! {
                if partial.#slot.0.is_none() {
                    return ::std::result::Result::Err(
                        #facade::Error::MissingRequired { name: #name },
                    );
                }
            })
        })
        .collect::<Vec<_>>();

    let apply_partial = if command.fields.is_empty() { quote!(_partial) } else { quote!(partial) };
    let check_partial = if duplicate_checks.is_empty() && required_checks.is_empty() {
        quote!(_partial)
    } else {
        quote!(partial)
    };
    let finish_partial = if command.fields.is_empty() { quote!(_partial) } else { quote!(partial) };

    let built_fields = command.fields.iter().enumerate().map(|(field_index, field)| {
        let ident = &field.ident;
        let value = finish_field(field, field_index, &facade);
        quote!(#ident: #value)
    });
    let built = if command.unit { quote!(Self) } else { quote!(Self { #(#built_fields),* }) };

    let name = &command.name;
    let parser_impl = command.root.then(|| {
        quote! {
            impl #impl_generics #facade::Parser for #ident #ty_generics #where_clause {}
        }
    });

    quote! {
        #[doc(hidden)]
        const _: () = {
            #keys
            #(#flag_tables)*
            #(#arg_tables)*

            static ARGX_COMMAND: #facade::__private::Command<'static> =
                #facade::__private::Command {
                    name: #name,
                    flags: &[#(#flag_refs),*],
                    args: &[#(#arg_refs),*],
                    subcommands: &[],
                    key: #command_key,
                };

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
                    match *event {
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
                    }
                }

                fn check(
                    #check_partial: &mut Self::Partial,
                ) -> ::std::result::Result<(), #facade::Error> {
                    #(#duplicate_checks)*
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
        };
    }
}

/// Adds the conversion bounds required by generated typed binding.
fn binding_generics(command: &model::Command) -> Generics {
    let mut generics = command.generics.clone();
    let mut bounded = Vec::new();
    for field in &command.fields {
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

/// Generates the current subcommand composition marker after validating the derive shape.
pub(crate) fn subcommands(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Subcommand can only be derived for enums",
        ));
    };

    attrs::reject(&input.attrs, "subcommand")?;
    for variant in &data.variants {
        attrs::reject(&variant.attrs, "subcommand variant")?;
        for field in &variant.fields {
            attrs::reject(&field.attrs, "subcommand field")?;
        }
    }

    let facade = crate_name::facade_path();
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #facade::__private::Subcommands
            for #ident #ty_generics #where_clause
        {}
    })
}
