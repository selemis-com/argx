//! Parsing and code generation for unified Argx configuration declarations.

mod generate;
mod input;

use generate::generate_config;
use input::Config;
use proc_macro2::{Span, TokenStream};
use syn::{DeriveInput, LitStr, Result};

use crate::crate_name::{facade_name, facade_path};

/// Expands one typed Argx configuration declaration.
pub(crate) fn config(input: &DeriveInput) -> Result<TokenStream> {
    let config = Config::parse(input)?;
    let argx = facade_path();
    let serde = LitStr::new(&format!("{}::__private::serde", facade_name()), Span::call_site());
    Ok(generate_config(&config, &argx, &serde))
}
