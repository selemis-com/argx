//! Stable semantic identity generation for commands and arguments.

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

/// Low-half tag for a named flag.
const KIND_FLAG: u64 = 0;
/// Low-half tag for a positional argument.
const KIND_ARG: u64 = 1 << 30;
/// Low-half tag for the command itself.
const KIND_COMMAND: u64 = 2 << 30;

/// Computes the declaration component of a generated key.
///
/// The module component is added by generated code, where `module_path!()` is available.
pub(crate) fn declaration_hash(fingerprint: &str) -> u32 {
    fingerprint.as_bytes().iter().fold(0x811c_9dc5_u32, |state, byte| hash_step(state, *byte))
}

/// Advances the fixed FNV-1a key hash by one byte.
fn hash_step(state: u32, byte: u8) -> u32 {
    (state ^ u32::from(byte)).wrapping_mul(0x0100_0193)
}

/// Emits semantic identity constants for one generated command declaration.
///
/// Each complete identity is a constant because generated binding code also uses these names as
/// `match` patterns; a bitwise expression cannot be used there as a single pattern value.
pub(crate) fn constants(
    facade: &TokenStream,
    fingerprint: &str,
    flags: usize,
    args: usize,
) -> TokenStream {
    let declaration = declaration_hash(fingerprint);
    let command = ident("COMMAND", None);
    let flag_keys = (0..flags).map(|index| {
        let name = ident("FLAG", Some(index));
        let index = index as u64;
        quote!(const #name: u64 = ARGX_KEY_BASE | #KIND_FLAG | #index;)
    });
    let arg_keys = (0..args).map(|index| {
        let name = ident("ARG", Some(index));
        let index = index as u64;
        quote!(const #name: u64 = ARGX_KEY_BASE | #KIND_ARG | #index;)
    });

    quote! {
        const ARGX_KEY_BASE: u64 =
            #facade::__private::key_base(::core::module_path!(), #declaration);
        const #command: u64 = ARGX_KEY_BASE | #KIND_COMMAND;
        #(#flag_keys)*
        #(#arg_keys)*
    }
}

/// Emits semantic command identities for the variants of one derived subcommand enum.
pub(crate) fn subcommand_constants(
    facade: &TokenStream,
    fingerprint: &str,
    commands: usize,
) -> TokenStream {
    let declaration = declaration_hash(fingerprint);
    let command_keys = (0..commands).map(|index| {
        let name = ident("SUBCOMMAND", Some(index));
        let index = index as u64;
        quote!(const #name: u64 = ARGX_KEY_BASE | #KIND_COMMAND | #index;)
    });

    quote! {
        const ARGX_KEY_BASE: u64 =
            #facade::__private::key_base(::core::module_path!(), #declaration);
        #(#command_keys)*
    }
}

/// Returns the generated constant name for one key.
pub(crate) fn ident(kind: &str, index: Option<usize>) -> Ident {
    index.map_or_else(
        || format_ident!("ARGX_KEY_{kind}"),
        |index| format_ident!("ARGX_KEY_{kind}_{index}"),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn declaration_hash_is_stable() {
        assert_eq!(super::declaration_hash("struct Cli ;"), 0xdef1_ee77);
    }

    #[test]
    fn declaration_hash_changes_with_the_declaration() {
        assert_ne!(super::declaration_hash("struct A ;"), super::declaration_hash("struct B ;"));
    }
}
