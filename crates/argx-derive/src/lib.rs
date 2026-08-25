//! Procedural macros for Argx.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/logo.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod attrs;
mod case;
mod codegen;
mod crate_name;
mod key;
mod model;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives the root Argx parser facade for a struct.
#[proc_macro_derive(Parser, attributes(argx))]
pub fn derive_parser(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command(&input, true).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives reusable command arguments for a struct.
#[proc_macro_derive(Args, attributes(argx))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command(&input, false).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives a subcommand set for an enum.
#[proc_macro_derive(Subcommand, attributes(argx))]
pub fn derive_subcommand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    codegen::subcommands(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Builds the semantic model and emits one command declaration.
fn expand_command(input: &DeriveInput, root: bool) -> syn::Result<proc_macro2::TokenStream> {
    let command = model::Command::from_input(input, root)?;
    Ok(codegen::command(&command))
}
