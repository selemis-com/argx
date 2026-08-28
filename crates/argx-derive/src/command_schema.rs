//! Static schema-topology derivation for structural command groups.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput};

use crate::{crate_name, model};

/// Derives schema traversal for one structural command declaration.
pub(crate) fn command_schema(input: &DeriveInput) -> syn::Result<TokenStream> {
    match &input.data {
        Data::Struct(_) => command_group(input),
        Data::Enum(_) => subcommand_group(input),
        _ => Err(syn::Error::new_spanned(
            &input.ident,
            "CommandSchema can only be derived for structs or enums",
        )),
    }
}

/// Implements schema traversal for one `Args` command group with a nested subcommand field.
fn command_group(input: &DeriveInput) -> syn::Result<TokenStream> {
    let command = model::Command::from_input(input, false)?;
    let Some(field) = command
        .fields
        .iter()
        .find(|field| matches!(field.semantics, model::FieldSemantics::Subcommand))
    else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "CommandSchema on a struct requires a `#[argx(subcommand)]` field; executable leaves use #[argx(handler = ...)]",
        ));
    };

    let facade = crate_name::facade_path();
    let ident = &command.binding.ident;
    let generics = &command.binding.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let subcommands = &field.binding.ty;

    Ok(quote! {
        impl #impl_generics #facade::__private::SchemaCommand for #ident #ty_generics #where_clause {
            fn register_schema_commands(
                command: &'static #facade::__private::Command<'static>,
                registry: &mut #facade::__private::SchemaRegistry,
            ) {
                <#subcommands as #facade::__private::SchemaSubcommands>::register_schema_subcommands(
                    command.subcommands,
                    registry,
                );
            }
        }
    })
}

/// Implements schema traversal for one `Subcommand` enum.
fn subcommand_group(input: &DeriveInput) -> syn::Result<TokenStream> {
    let subcommand = model::Subcommand::from_input(input)?;
    for variant in &subcommand.variants {
        if variant.binding.payload.is_none() {
            return Err(syn::Error::new_spanned(
                &variant.binding.ident,
                "CommandSchema requires executable subcommands to use a concrete Args payload; use an empty Args struct instead of a unit variant",
            ));
        }
    }

    let facade = crate_name::facade_path();
    let ident = &subcommand.binding.ident;
    let generics = &subcommand.binding.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let registrations = subcommand.variants.iter().enumerate().map(|(index, variant)| {
        let ty = variant.binding.payload.as_ref().expect("unit variants were rejected above");
        quote! {
            <#ty as #facade::__private::SchemaCommand>::register_schema_commands(
                commands[#index],
                registry,
            );
        }
    });
    let count = subcommand.variants.len();

    Ok(quote! {
        impl #impl_generics #facade::__private::SchemaSubcommands for #ident #ty_generics #where_clause {
            fn register_schema_subcommands(
                commands: &'static [&'static #facade::__private::Command<'static>],
                registry: &mut #facade::__private::SchemaRegistry,
            ) {
                assert_eq!(
                    commands.len(),
                    #count,
                    "generated schema topology diverged from subcommand metadata",
                );
                #(#registrations)*
            }
        }
    })
}
