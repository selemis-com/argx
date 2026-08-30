//! Procedural macros used by the `argx` crate.
//!
//! Applications normally use these macros through the `argx` facade crate.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/logo.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod args;
mod config;
mod support;

use proc_macro::TokenStream;
use syn::{
    DeriveInput, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Derives the root command-line parser for a unit or named-field struct.
#[proc_macro_derive(Parser, attributes(argx))]
pub fn derive_parser(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command(&input, true).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives a typed Argx configuration contract.
///
/// Configuration fields resolve through explicitly ordered layers. Defaults, TOML, environment
/// sources, and argv therefore contribute to one typed value rather than independent models.
#[proc_macro_derive(Config, attributes(argx))]
pub fn derive_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    config::config(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives reusable command arguments for a unit or named-field struct.
///
/// Use the resulting type as a `Subcommand` payload or compose it with `#[argx(flatten)]`.
#[proc_macro_derive(Args, attributes(argx))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command(&input, false).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Derives a finite command-line value vocabulary for a unit enum.
///
/// Variants use kebab-case CLI spellings. Mark a field with `#[argx(value_enum)]` to use the same
/// vocabulary for parsing, help, completion, and schema discovery.
#[proc_macro_derive(ValueEnum)]
pub fn derive_value_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    args::value_enum::value_enum(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// One standalone Argx item attribute.
enum StandaloneAttribute {
    /// Derive JSON Schema through Schemars.
    Schema,
    /// Associate one handler target with the annotated function or inherent impl.
    Handler(proc_macro2::TokenStream),
}

impl Parse for StandaloneAttribute {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let kind: syn::Ident = input.parse()?;
        if kind == "schema" {
            if !input.is_empty() {
                return Err(input.error("`schema` takes no value"));
            }
            return Ok(Self::Schema);
        }
        if kind == "handler" {
            if !input.peek(Token![=]) {
                return Err(syn::Error::new(
                    kind.span(),
                    "`handler` requires `= <command type or method>`",
                ));
            }
            input.parse::<Token![=]>()?;
            if input.is_empty() {
                return Err(input.error("`handler` requires a command type or method name"));
            }
            let value: proc_macro2::TokenStream = input.parse()?;
            return Ok(Self::Handler(value));
        }

        Err(syn::Error::new(kind.span(), "unsupported standalone Argx attribute"))
    }
}

/// Applies one standalone Argx item attribute.
///
/// `#[argx(schema)]` marks a Rust type for JSON Schema generation through Schemars.
/// `#[argx(handler = CommandType)]` associates a free function with one invocable command type,
/// while `#[argx(handler = method)]` associates an inherent impl with its execution method.
#[proc_macro_attribute]
pub fn argx(attribute: TokenStream, input: TokenStream) -> TokenStream {
    let attribute = proc_macro2::TokenStream::from(attribute);
    let input = proc_macro2::TokenStream::from(input);

    standalone_attribute(attribute, input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Dispatches the standalone `#[argx(...)]` attribute forms.
fn standalone_attribute(
    attribute: proc_macro2::TokenStream,
    input: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    match syn::parse2::<StandaloneAttribute>(attribute)? {
        StandaloneAttribute::Schema => args::schema::schema(input),
        StandaloneAttribute::Handler(handler) => args::handler::handler(handler, input),
    }
}

/// Derives child commands from an enum.
///
/// Variants may be unit commands or carry one direct `Args` payload. Add `#[argx(schema)]` when the
/// subcommand set participates in schema discovery.
#[proc_macro_derive(Subcommand, attributes(argx))]
pub fn derive_subcommand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_subcommand(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Builds the semantic model and emits one command declaration.
fn expand_command(input: &DeriveInput, root: bool) -> syn::Result<proc_macro2::TokenStream> {
    let command = args::model::Command::from_input(input, root)?;
    Ok(args::generate::command(&command))
}

/// Builds the semantic model and emits one subcommand enum.
fn expand_subcommand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let subcommand = args::model::Subcommand::from_input(input)?;
    Ok(args::generate::subcommands(&subcommand))
}
