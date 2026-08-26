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
mod execution_contract;
mod key;
mod model;
mod type_contract;

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

/// Derives a standalone machine-readable semantic contract for a Rust struct or enum.
///
/// The generated contract preserves Rust declaration, field, and variant names. It is independent
/// of serialization frameworks and does not interpret attributes such as `serde(rename = ...)`.
/// When `Contract` is co-derived with `Parser` or `Args`, their CLI helper attributes remain
/// available but do not rename semantic type-contract fields. `Contract` by itself does not
/// register `#[argx(...)]` as meaningful type-contract metadata.
#[proc_macro_derive(Contract)]
pub fn derive_contract(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    type_contract::contract(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Attaches one canonical execution result contract to an invocable command type.
///
/// The annotated free function remains ordinary Rust. Its parameters are runtime-only and do not
/// participate in the machine contract. Its concrete `Result<Success, Error>` return type is the
/// command's semantic execution contract: both `Success` and `Error` must implement Argx's type
/// contract. The attribute records contract metadata only; it does not register, wrap, or invoke
/// the function for dispatch.
#[proc_macro_attribute]
pub fn contract(attribute: TokenStream, input: TokenStream) -> TokenStream {
    execution_contract::contract(
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
