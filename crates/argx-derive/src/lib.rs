//! Procedural macros for Argx.
//!
//! The derive crate is intentionally a compile-time frontend. It parses attributes and Rust
//! documentation into one normalized semantic model, validates every invariant visible to the
//! current expansion, and emits shared static command metadata plus typed binding and semantic
//! projections. Runtime parsing policy lives in the `argx` facade crate rather than in generated
//! token-matching logic.

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
mod handler;
mod key;
mod model;
mod schema;
mod value_enum;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives the root Argx parser facade for a struct.
///
/// `Parser` accepts unit structs and structs with named fields. The resulting type owns the root
/// command metadata and receives the public `argx::Parser` implementation. Tuple structs and enum
/// declarations are rejected.
///
/// Type-shape inference is syntactic. Special handling for `bool`, `Option`, `Vec`, `String`,
/// `OsString`, and `PathBuf` requires a recognized standard spelling to appear directly in the
/// field type. Type aliases around those types are treated as ordinary value types because a
/// procedural macro cannot resolve aliases.
#[proc_macro_derive(Parser, attributes(argx))]
pub fn derive_parser(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command(&input, true).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives reusable command arguments for a struct.
///
/// `Args` accepts the same struct shapes as `Parser`, but the generated declaration has no process
/// entry-point semantics of its own. It can be mounted through `#[argx(flatten)]` or used as the
/// direct payload of a `Subcommand` variant.
///
/// Type-shape inference is syntactic. Special handling for `bool`, `Option`, `Vec`, `String`,
/// `OsString`, and `PathBuf` requires a recognized standard spelling to appear directly in the
/// field type. Type aliases around those types are treated as ordinary value types because a
/// procedural macro cannot resolve aliases.
#[proc_macro_derive(Args, attributes(argx))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command(&input, false).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives a finite canonical command-line value vocabulary for an enum.
///
/// Every unit variant is accepted using Argx's normal kebab-case spelling. The derive implements
/// both `argx::ValueEnum` and `FromStr`; parsing is exact and case-sensitive. Generic enums and
/// variants carrying fields are rejected. Use `#[argx(value_enum)]` on a parser field to project
/// the same vocabulary into parsing metadata, generated help, and completion.
#[proc_macro_derive(ValueEnum)]
pub fn derive_value_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    value_enum::value_enum(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Marks a Rust type for JSON Schema generation through Schemars.
///
/// The macro only injects Schemars' `JsonSchema` derive and routes its generated crate paths
/// through Argx, so downstream users do not need to depend on Schemars directly.
#[proc_macro_attribute]
pub fn schema(attribute: TokenStream, input: TokenStream) -> TokenStream {
    if !attribute.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[argx::schema] takes no arguments",
        )
        .into_compile_error()
        .into();
    }
    schema::schema(proc_macro2::TokenStream::from(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Associates a handler with one directly invocable command type.
///
/// The handler remains an ordinary Rust function. Its concrete `Result<Success, Error>` return
/// type defines the command's success and error JSON Schemas; both types must implement
/// Schemars' `JsonSchema` trait. The attribute records metadata only and does not dispatch the
/// function.
#[proc_macro_attribute]
pub fn handler(attribute: TokenStream, input: TokenStream) -> TokenStream {
    handler::handler(
        proc_macro2::TokenStream::from(attribute),
        proc_macro2::TokenStream::from(input),
    )
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Derives a subcommand set for an enum.
///
/// Every variant becomes one exact child-command spelling. Variants may be unit variants or contain
/// exactly one unnamed direct `Args` payload; named fields, multiple tuple fields, and collection
/// or optional wrappers around a payload are rejected. Canonical names and aliases share one
/// sibling namespace so command lookup is never order-dependent.
#[proc_macro_derive(Subcommand, attributes(argx))]
pub fn derive_subcommand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_subcommand(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Builds the semantic model and emits one command declaration.
fn expand_command(input: &DeriveInput, root: bool) -> syn::Result<proc_macro2::TokenStream> {
    let command = model::Command::from_input(input, root)?;
    Ok(codegen::command(&command))
}

/// Builds the semantic model and emits one subcommand enum.
fn expand_subcommand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let subcommand = model::Subcommand::from_input(input)?;
    Ok(codegen::subcommands(&subcommand))
}
