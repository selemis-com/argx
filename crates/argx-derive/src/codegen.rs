//! Code generation from the validated Argx semantic model.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput};

use crate::{attrs, crate_name, key, model};

/// Generates static parse metadata and the facade traits for one command struct.
pub(crate) fn command(command: &model::Command) -> TokenStream {
    let facade = crate_name::facade_path();
    let ident = &command.ident;
    let (impl_generics, ty_generics, where_clause) = command.generics.split_for_impl();

    let flags = command
        .fields
        .iter()
        .filter(|field| matches!(&field.kind, model::FieldKind::Flag { .. }))
        .collect::<Vec<_>>();
    let args = command
        .fields
        .iter()
        .filter(|field| matches!(&field.kind, model::FieldKind::Positional))
        .collect::<Vec<_>>();
    let keys = key::constants(&facade, &command.fingerprint, flags.len(), args.len());
    let command_key = key::ident("COMMAND", None);

    let flag_tables = flags.iter().enumerate().map(|(index, field)| {
        let table = format_ident!("ARGX_FLAG_{index}");
        let key = key::ident("FLAG", Some(index));
        let model::FieldKind::Flag { longs, shorts } = &field.kind else {
            unreachable!("flag list only contains flag fields");
        };
        let name = &field.name;
        let takes_value = field.shape != model::Shape::Bool;
        quote! {
            static #table: #facade::__private::Flag<'static> = #facade::__private::Flag {
                key: #key,
                name: #name,
                longs: &[#(#longs),*],
                shorts: &[#(#shorts),*],
                takes_value: #takes_value,
                allow_hyphen_values: false,
                allow_negative_numbers: false,
            };
        }
    });
    let flag_refs = (0..flags.len()).map(|index| {
        let table = format_ident!("ARGX_FLAG_{index}");
        quote!(&#table)
    });

    let arg_tables = args.iter().enumerate().map(|(index, field)| {
        let table = format_ident!("ARGX_ARG_{index}");
        let key = key::ident("ARG", Some(index));
        let name = &field.name;
        let required = matches!(field.shape, model::Shape::Bool | model::Shape::Required);
        let variadic = field.shape == model::Shape::Many;
        quote! {
            static #table: #facade::__private::Arg<'static> =
                #facade::__private::Arg {
                    key: #key,
                    name: #name,
                    required: #required,
                    variadic: #variadic,
                    allow_negative_numbers: false,
                };
        }
    });
    let arg_refs = (0..args.len()).map(|index| {
        let table = format_ident!("ARGX_ARG_{index}");
        quote!(&#table)
    });

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
                const COMMAND: &'static #facade::__private::Command<'static> = &ARGX_COMMAND;
            }

            #parser_impl
        };
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
