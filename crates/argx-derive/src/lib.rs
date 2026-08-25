//! Procedural macros for Argx.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/logo.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input};

/// Derives the root Argx parser facade for a struct.
#[proc_macro_derive(Parser, attributes(argx))]
pub fn derive_parser(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_parser(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives reusable command arguments for a struct.
#[proc_macro_derive(Args, attributes(argx))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_args(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives a subcommand set for an enum.
#[proc_macro_derive(Subcommand, attributes(argx))]
pub fn derive_subcommand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_subcommand(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Expands the root parser marker implementation.
fn expand_parser(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if !matches!(&input.data, Data::Struct(_)) {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Parser can only be derived for structs",
        ));
    }

    let facade = facade_path();
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #facade::Parser for #ident #ty_generics #where_clause {}
    })
}

/// Expands the reusable-arguments marker implementation.
fn expand_args(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if !matches!(&input.data, Data::Struct(_)) {
        return Err(syn::Error::new_spanned(&input.ident, "Args can only be derived for structs"));
    }

    let facade = facade_path();
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #facade::__private::CommandArgs for #ident #ty_generics #where_clause {}
    })
}

/// Expands the subcommand marker implementation.
fn expand_subcommand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if !matches!(&input.data, Data::Enum(_)) {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Subcommand can only be derived for enums",
        ));
    }

    let facade = facade_path();
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #facade::__private::Subcommands for #ident #ty_generics #where_clause {}
    })
}

/// Resolves the Argx facade under the dependency name chosen by the downstream crate.
fn facade_path() -> TokenStream2 {
    match crate_name("argx") {
        Ok(FoundCrate::Name(name)) => {
            let facade = Ident::new(&name, Span::call_site());
            quote!(::#facade)
        }
        Ok(FoundCrate::Itself) | Err(_) => quote!(::argx),
    }
}
