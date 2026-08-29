//! Privacy-safe partial-state projection for generated commands.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{key, model};

/// Generated partial-state projection for one command declaration.
#[derive(Debug)]
pub(super) struct PartialProjection {
    /// Helper declarations emitted alongside the derived type.
    pub(super) declarations: TokenStream,
    /// Partial-state type exposed through `CommandArgs`.
    pub(super) partial: TokenStream,
    /// Nominal constructor used when private composed types require an opaque partial witness.
    pub(super) partial_constructor: Option<proc_macro2::Ident>,
}

/// Builds the privacy-safe partial state for this command declaration.
pub(super) fn partial_projection(
    command: &model::Command,
    facade: &TokenStream,
) -> PartialProjection {
    let ident = &command.binding.ident;
    let visibility = &command.binding.visibility;
    let suffix = ident.to_string().trim_start_matches("r#").to_owned();
    let declaration = key::declaration_hash(&command.binding.fingerprint);
    let partial_ident = partial_type_constructor(command);
    let shape_ident = format_ident!("__ArgxPartialShapeFor{}H{}", suffix, declaration);
    let (impl_generics, ty_generics, where_clause) = command.binding.generics.split_for_impl();

    let nested = command
        .fields
        .iter()
        .enumerate()
        .filter_map(|(index, field)| match &field.semantics {
            model::FieldSemantics::Flatten | model::FieldSemantics::Subcommand => {
                Some((index, field))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let has_nested = !nested.is_empty();

    let associated =
        nested.iter().map(|(index, _)| format_ident!("Field{index}")).collect::<Vec<_>>();
    let definitions = nested
        .iter()
        .zip(&associated)
        .map(|((_, field), associated)| {
            let ty = &field.binding.ty;
            quote!(type #associated = #ty;)
        })
        .collect::<Vec<_>>();

    let shape = has_nested.then(|| {
        quote! {
            trait #shape_ident {
                #(type #associated;)*
            }

            impl #impl_generics #shape_ident for #ident #ty_generics #where_clause {
                #(#definitions)*
            }
        }
    });

    let mut nested_index = 0_usize;
    let mut bounds = Vec::new();
    let partial_types = command.fields.iter().map(|field| match &field.semantics {
        model::FieldSemantics::Flatten => {
            let associated = &associated[nested_index];
            nested_index += 1;
            bounds.push(quote!(<T as #shape_ident>::#associated: #facade::__private::CommandArgs));
            quote!(<<T as #shape_ident>::#associated as #facade::__private::CommandArgs>::Partial)
        }
        model::FieldSemantics::Subcommand => {
            let associated = &associated[nested_index];
            nested_index += 1;
            bounds.push(quote!(<T as #shape_ident>::#associated: #facade::__private::Subcommands));
            quote!(<<T as #shape_ident>::#associated as #facade::__private::Subcommands>::Partial)
        }
        _ if field.argument().is_some_and(|argument| argument.shape == model::Shape::Many) => {
            quote!(::std::vec::Vec<::std::vec::Vec<u8>>)
        }
        _ if field.is_switch() => quote!((bool, bool)),
        _ => quote!((::std::option::Option<::std::vec::Vec<u8>>, bool)),
    }).collect::<Vec<_>>();

    let (partial, partial_constructor, declaration) = if has_nested {
        (
            quote!(#partial_ident<Self>),
            Some(partial_ident.clone()),
            quote! {
                #shape

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
                    #(#bounds,)*;
            },
        )
    } else {
        (quote!((#(#partial_types,)*)), None, TokenStream::new())
    };

    PartialProjection { declarations: declaration, partial, partial_constructor }
}

/// Returns the generated partial projection type name for `command`.
fn partial_type_constructor(command: &model::Command) -> proc_macro2::Ident {
    let suffix = command.binding.ident.to_string().trim_start_matches("r#").to_owned();
    let declaration = key::declaration_hash(&command.binding.fingerprint);
    format_ident!("__ArgxPartialFor{}H{}", suffix, declaration)
}
