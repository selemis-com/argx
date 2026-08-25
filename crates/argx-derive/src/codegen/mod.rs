//! Code generation from the validated Argx semantic model.

mod command;
mod subcommand;

pub(crate) use command::command;
use proc_macro2::TokenStream;
use quote::quote;
pub(crate) use subcommand::subcommands;

/// Renders optional static text into generated metadata.
pub(super) fn option_str(value: Option<&str>) -> TokenStream {
    value.map_or_else(
        || quote!(::std::option::Option::None),
        |value| quote!(::std::option::Option::Some(#value)),
    )
}
