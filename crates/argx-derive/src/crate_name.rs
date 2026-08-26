//! Resolves the Argx facade name chosen by the downstream crate.
//!
//! Macro expansion must work when the dependency is renamed in `Cargo.toml` and when Argx derives
//! are exercised inside the facade crate itself. All generated runtime paths go through this helper
//! so crate-name resolution policy stays centralized.

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
        // `Itself` is reached while testing/using the derive from the facade crate. Falling back
        // to `::argx` also keeps expansion deterministic if dependency discovery is unavailable.
        Ok(FoundCrate::Itself) | Err(_) => quote!(::argx),
    }
}
