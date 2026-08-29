//! Typed partial-state binding helpers for generated commands.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, parse_quote};

use crate::args::model;

/// Adds the conversion bounds required by generated typed binding.
pub(super) fn binding_generics(command: &model::Command, facade: &TokenStream) -> Generics {
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
        let value_enum = field.argument().is_some_and(|argument| argument.value_enum);
        let rendered = quote!(#ty).to_string();
        let bound = if value_enum { format!("value-enum:{rendered}") } else { rendered };
        if bounded.contains(&bound) {
            continue;
        }
        bounded.push(bound);

        if value_enum {
            generics.make_where_clause().predicates.push(parse_quote!(#ty: #facade::ValueEnum));
            continue;
        }

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
pub(super) fn apply_arm(
    field: &model::Field,
    field_index: usize,
    table: &proc_macro2::Ident,
    key: &proc_macro2::Ident,
    flag: bool,
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
                    partial.#slot.0 = ::std::option::Option::Some(value.to_vec());
                }
                true
            },
        }
    }
}

/// Generates one semantic argument-presence lookup branch.
pub(super) fn argument_state_branch(
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
pub(super) fn finish_field(
    field: &model::Field,
    field_index: usize,
    facade: &TokenStream,
) -> TokenStream {
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
    let one = |value: TokenStream| {
        if argument.value_enum {
            return quote!(#facade::__private::value_enum_value::<#ty>(#value, #name)?);
        }
        match binding.conversion {
            model::ValueConversion::Text => quote!(#facade::__private::text_value(#value, #name)?),
            model::ValueConversion::Os => {
                quote!(#facade::__private::os_value::<#ty>(#value, #name)?)
            }
            model::ValueConversion::FromStr => {
                quote!(#facade::__private::parsed_value::<#ty>(#value, #name)?)
            }
        }
    };
    let many = |value: TokenStream| {
        if argument.value_enum {
            return quote!(#facade::__private::value_enum_values::<#ty>(#value, #name)?);
        }
        match binding.conversion {
            model::ValueConversion::Text => quote!(#facade::__private::text_values(#value, #name)?),
            model::ValueConversion::Os => {
                quote!(#facade::__private::os_values::<#ty>(#value, #name)?)
            }
            model::ValueConversion::FromStr => {
                quote!(#facade::__private::parsed_values::<#ty>(#value, #name)?)
            }
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
