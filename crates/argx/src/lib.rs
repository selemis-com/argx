//! Command line argument parser for Rust.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/logo.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod parser;

// Generated absolute paths must also work when a derive is used inside this crate. Integration
// targets already receive this name through Cargo; the library target needs the self alias.
#[expect(
    unused_extern_crates,
    reason = "proc-macro expansions refer to this crate through `::argx`"
)]
extern crate self as argx;

#[cfg(feature = "derive")]
pub use argx_derive::{Args, Parser, Subcommand};

/// Parses command-line arguments into a typed value.
///
/// Typed parsing entry points are added once generated value binding is in place. For now this
/// trait marks a root command and guarantees that derive-generated static command metadata is
/// available.
pub trait Parser: Sized + __private::CommandArgs {}

/// Implementation details shared with generated code.
///
/// This module is public so proc-macro expansions can name these items from downstream crates. It
/// is not part of Argx's stable user-facing API.
#[doc(hidden)]
pub mod __private {
    pub use crate::parser::{ArgvParser, Error, Event};

    /// Identifier echoed by the parser so generated binding code can dispatch without comparing
    /// argument names.
    pub type Key = u64;

    /// Static parse metadata for one command.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Command<'a> {
        /// Command name as exposed on the command line.
        pub name: &'a str,
        /// Flags accepted by this command.
        pub flags: &'a [&'a Flag<'a>],
        /// Positional arguments accepted by this command.
        pub args: &'a [&'a Arg<'a>],
        /// Child commands accepted by this command.
        pub subcommands: &'a [&'a Self],
        /// Derive-assigned command identity.
        pub key: Key,
    }

    impl Command<'static> {
        /// Empty command metadata for use with struct update syntax.
        pub const EMPTY: Self = Self { name: "", flags: &[], args: &[], subcommands: &[], key: 0 };
    }

    /// Static parse metadata for a named flag.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Flag<'a> {
        /// Derive-assigned field identity.
        pub key: Key,
        /// Canonical field name used by diagnostics and generated binding code.
        pub name: &'a str,
        /// Long spellings without the leading `--`.
        pub longs: &'a [&'a str],
        /// ASCII short spellings without the leading `-`.
        pub shorts: &'a [u8],
        /// Whether one occurrence consumes a value.
        pub takes_value: bool,
        /// Whether a detached value may itself be flag-like.
        pub allow_hyphen_values: bool,
        /// Whether a detached negative number may be consumed while other flag-like values are
        /// refused.
        pub allow_negative_numbers: bool,
    }

    impl Flag<'static> {
        /// A value-less flag for use with struct update syntax.
        pub const BOOL: Self = Self {
            key: 0,
            name: "",
            longs: &[],
            shorts: &[],
            takes_value: false,
            allow_hyphen_values: false,
            allow_negative_numbers: false,
        };

        /// A flag that consumes one value for use with struct update syntax.
        pub const VALUE: Self = Self { takes_value: true, ..Self::BOOL };
    }

    /// Static parse metadata for a positional argument.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Arg<'a> {
        /// Derive-assigned field identity.
        pub key: Key,
        /// Canonical field name used by diagnostics and generated binding code.
        pub name: &'a str,
        /// Whether this positional must receive at least one value.
        pub required: bool,
        /// Whether this positional may receive multiple values.
        pub variadic: bool,
        /// Whether a negative number may bind here while flag parsing remains enabled.
        pub allow_negative_numbers: bool,
    }

    impl Arg<'static> {
        /// A required single-value positional for use with struct update syntax.
        pub const REQUIRED: Self = Self {
            key: 0,
            name: "",
            required: true,
            variadic: false,
            allow_negative_numbers: false,
        };
    }

    /// Computes the high 32 bits shared by keys from one derived declaration.
    ///
    /// Generated code supplies the containing module so declarations expanded independently can
    /// still be distinguished when their source tokens are otherwise identical.
    pub const fn key_base(module: &str, declaration: u32) -> Key {
        let bytes = module.as_bytes();
        let mut state = declaration;
        let mut index = 0;
        while index < bytes.len() {
            state = (state ^ bytes[index] as u32).rotate_left(5).wrapping_mul(0x9e37_79b1);
            index += 1;
        }
        (state as Key) << 32
    }

    /// Static command metadata exposed by a root parser or reusable argument struct.
    ///
    /// The associated constant is what eventually lets a parent splice flattened child tables into
    /// its own static tables without constructing a command tree at runtime.
    pub trait CommandArgs: Sized {
        /// Parse tables for this declaration.
        const COMMAND: &'static Command<'static>;
    }

    /// Static command metadata exposed by a derived subcommand enum.
    ///
    /// Variant table generation arrives with subcommand support. The empty default keeps the
    /// composition contract in place while that feature is not implemented yet.
    pub trait Subcommands: Sized {
        /// Parse tables for the enum's named subcommands.
        const COMMANDS: &'static [&'static Command<'static>] = &[];
    }

    #[cfg(test)]
    mod tests {
        use super::key_base;

        #[test]
        fn key_base_is_stable() {
            assert_eq!(key_base("argx::tests", 0x1234_5678), 0xfb07_66cd_0000_0000);
        }

        #[test]
        fn module_path_contributes_to_key_base() {
            assert_ne!(key_base("argx::add", 42), key_base("argx::remove", 42));
        }
    }
}
