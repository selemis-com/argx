//! Thin Schemars routing for `#[argx(schema)]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr, parse_quote};

/// Adds Schemars' derive and routes generated paths through the Argx facade.
pub(crate) fn schema(input: TokenStream) -> syn::Result<TokenStream> {
    let mut input = syn::parse2::<DeriveInput>(input)?;
    let facade = crate::crate_name::facade_path();
    let facade_name = crate::crate_name::facade_name();
    let schemars_path =
        LitStr::new(&format!("{facade_name}::__private::schemars"), proc_macro2::Span::call_site());

    input.attrs.push(parse_quote!(#[derive(#facade::__private::schemars::JsonSchema)]));
    input.attrs.push(parse_quote!(#[schemars(crate = #schemars_path)]));

    Ok(quote!(#input))
}
