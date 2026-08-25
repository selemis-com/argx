//! Resolves the Argx facade name chosen by the downstream crate.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Returns an absolute path to the Argx facade from generated code.
pub(crate) fn facade_path() -> TokenStream {
    match crate_name("argx") {
        Ok(FoundCrate::Name(name)) => {
            let facade = Ident::new(&name, Span::call_site());
            quote!(::#facade)
        }
        Ok(FoundCrate::Itself) | Err(_) => quote!(::argx),
    }
}
