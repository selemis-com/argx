//! Derive support for finite command-line value vocabularies.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::{case, crate_name};

/// Generates one canonical finite CLI vocabulary and exact parser.
pub(crate) fn value_enum(input: &DeriveInput) -> syn::Result<TokenStream> {
    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "ValueEnum can only be derived for enums",
            ));
        }
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "ValueEnum does not support generic enums",
        ));
    }
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "ValueEnum requires at least one variant",
        ));
    }

    let mut seen = HashSet::new();
    let mut variants = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                &variant.fields,
                "ValueEnum variants cannot contain fields",
            ));
        }
        let rust_name = ident_name(&variant.ident);
        let value = case::to_kebab(&rust_name);
        if !seen.insert(value.clone()) {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                format!("duplicate ValueEnum spelling `{value}`"),
            ));
        }
        variants.push((&variant.ident, value));
    }

    let facade = crate_name::facade_path();
    let ident = &input.ident;
    let values = variants.iter().map(|(_, value)| value);
    let matches = variants.iter().map(|(variant, value)| quote!(#value => Self::#variant,));

    Ok(quote! {
        impl #facade::ValueEnum for #ident {
            const VALUES: &'static [&'static str] = &[#(#values),*];

            fn from_value(value: &str) -> ::std::option::Option<Self> {
                ::std::option::Option::Some(match value {
                    #(#matches)*
                    _ => return ::std::option::Option::None,
                })
            }
        }

        impl ::std::str::FromStr for #ident {
            type Err = #facade::ValueEnumError;

            fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
                <Self as #facade::ValueEnum>::from_value(value).ok_or_else(|| {
                    #facade::ValueEnumError::new(<Self as #facade::ValueEnum>::VALUES)
                })
            }
        }
    })
}

/// Returns an identifier without Rust's raw-identifier prefix.
fn ident_name(ident: &syn::Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}
