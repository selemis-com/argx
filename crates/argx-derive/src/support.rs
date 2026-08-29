//! Small compile-time helpers shared across derive frontends and code generation.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Returns an identifier without Rust's raw-identifier prefix.
pub(crate) fn ident_name(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}

/// Converts the default Rust spelling to Argx's kebab-case command-line spelling.
///
/// Underscores become dashes and every uppercase character begins a new segment.
pub(crate) fn to_kebab(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 4);
    for (index, character) in value.chars().enumerate() {
        if character == '_' {
            output.push('-');
        } else if character.is_uppercase() {
            if index > 0 {
                output.push('-');
            }
            output.extend(character.to_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

/// Returns the resolved Rust crate name for the Argx facade.
pub(crate) fn facade_name() -> String {
    match crate_name("argx") {
        Ok(FoundCrate::Name(name)) => name,
        Ok(FoundCrate::Itself) | Err(_) => String::from("argx"),
    }
}

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

#[cfg(test)]
mod tests {
    #[test]
    fn converts_default_names() {
        assert_eq!(super::to_kebab("HTTPServer"), "h-t-t-p-server");
        assert_eq!(super::to_kebab("output_file"), "output-file");
    }
}
