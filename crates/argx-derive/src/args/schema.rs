//! Thin Schemars routing for `#[argx(schema)]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr, parse_quote};

/// Adds Schemars' derive and routes generated paths through the Argx facade.
pub(crate) fn schema(input: TokenStream) -> syn::Result<TokenStream> {
    let mut input = syn::parse2::<DeriveInput>(input)?;
    let facade = crate::support::facade_path();
    let schemars_path = LitStr::new("argx::__private::schemars", proc_macro2::Span::call_site());

    input.attrs.push(parse_quote!(#[derive(#facade::__private::schemars::JsonSchema)]));
    input.attrs.push(parse_quote!(#[schemars(crate = #schemars_path)]));

    Ok(quote!(#input))
}
