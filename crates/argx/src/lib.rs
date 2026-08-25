//! Command line argument parser for Rust.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/logo.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod binding;
mod error;
mod parser;

use std::ffi::{OsStr, OsString};

pub use error::{Error, InvalidValue};

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
pub trait Parser: Sized + __private::CommandArgs {
    /// Parses the current process arguments, excluding the program name.
    ///
    /// Parse failures are printed to standard error and terminate the process with status 2.
    fn parse() -> Self {
        Self::try_parse().unwrap_or_else(|error| error.exit())
    }

    /// Parses the current process arguments, excluding the program name.
    ///
    /// # Errors
    ///
    /// Returns an error when argv cannot be bound to this command or a bound value cannot be
    /// converted to its Rust field type.
    fn try_parse() -> Result<Self, Error> {
        Self::try_parse_args(std::env::args_os().skip(1))
    }

    /// Parses a complete argv sequence whose first item is the program name.
    ///
    /// Parse failures are printed to standard error and terminate the process with status 2.
    fn parse_from<I, T>(argv: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        Self::try_parse_from(argv).unwrap_or_else(|error| error.exit())
    }

    /// Parses a complete argv sequence whose first item is the program name.
    ///
    /// The program name is ignored. An empty sequence is therefore equivalent to a program name
    /// followed by no command-line arguments.
    ///
    /// # Errors
    ///
    /// Returns an error when argv cannot be bound to this command or a bound value cannot be
    /// converted to its Rust field type.
    fn try_parse_from<I, T>(argv: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut argv = argv.into_iter();
        let _ = argv.next();
        Self::try_parse_args(argv)
    }

    /// Parses arguments that do not include a program name.
    ///
    /// Parse failures are printed to standard error and terminate the process with status 2.
    fn parse_args<I, T>(argv: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        Self::try_parse_args(argv).unwrap_or_else(|error| error.exit())
    }

    /// Parses arguments that do not include a program name.
    ///
    /// This entry point is useful when argv is already separated from the executable name, such as
    /// in tests, embedded command dispatch, or an agent invoking a command directly.
    ///
    /// # Errors
    ///
    /// Returns an error when argv cannot be bound to this command or a bound value cannot be
    /// converted to its Rust field type.
    fn try_parse_args<I, T>(argv: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let owned: Vec<OsString> = argv.into_iter().map(Into::into).collect();
        let refs: Vec<&OsStr> = owned.iter().map(OsString::as_os_str).collect();
        binding::parse_refs::<Self>(&refs)
    }
}

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

    /// Returns the total number of entries across several static table slices.
    pub const fn table_len<T>(groups: &[&[T]]) -> usize {
        let mut total = 0;
        let mut group = 0;
        while group < groups.len() {
            total += groups[group].len();
            group += 1;
        }
        total
    }

    /// Concatenates flag-table groups into one static array while preserving group order.
    ///
    /// # Panics
    ///
    /// Panics during const evaluation if `N` is not [`table_len`] of `groups`.
    pub const fn concat_flags<const N: usize>(
        groups: &[&[&'static Flag<'static>]],
    ) -> [&'static Flag<'static>; N] {
        static PLACEHOLDER: Flag<'static> = Flag::BOOL;
        let mut joined = [&PLACEHOLDER; N];
        let mut at = 0;
        let mut group = 0;
        while group < groups.len() {
            let entries = groups[group];
            let mut index = 0;
            while index < entries.len() {
                joined[at] = entries[index];
                at += 1;
                index += 1;
            }
            group += 1;
        }
        assert!(at == N, "concatenated flag-table length must match table_len");
        joined
    }

    /// Concatenates positional-table groups into one static array while preserving group order.
    ///
    /// # Panics
    ///
    /// Panics during const evaluation if `N` is not [`table_len`] of `groups`.
    pub const fn concat_args<const N: usize>(
        groups: &[&[&'static Arg<'static>]],
    ) -> [&'static Arg<'static>; N] {
        static PLACEHOLDER: Arg<'static> = Arg::REQUIRED;
        let mut joined = [&PLACEHOLDER; N];
        let mut at = 0;
        let mut group = 0;
        while group < groups.len() {
            let entries = groups[group];
            let mut index = 0;
            while index < entries.len() {
                joined[at] = entries[index];
                at += 1;
                index += 1;
            }
            group += 1;
        }
        assert!(at == N, "concatenated positional-table length must match table_len");
        joined
    }

    /// Reports whether every flag and positional key on one composed command is unique.
    pub const fn command_keys_unique(flags: &[&Flag<'_>], args: &[&Arg<'_>]) -> bool {
        let mut flag = 0;
        while flag < flags.len() {
            let mut other = flag + 1;
            while other < flags.len() {
                if flags[flag].key == flags[other].key {
                    return false;
                }
                other += 1;
            }
            let mut arg = 0;
            while arg < args.len() {
                if flags[flag].key == args[arg].key {
                    return false;
                }
                arg += 1;
            }
            flag += 1;
        }

        let mut arg = 0;
        while arg < args.len() {
            let mut other = arg + 1;
            while other < args.len() {
                if args[arg].key == args[other].key {
                    return false;
                }
                other += 1;
            }
            arg += 1;
        }
        true
    }

    /// Reports whether no two flags on one composed command answer to the same spelling.
    pub const fn flag_spellings_unique(flags: &[&Flag<'_>]) -> bool {
        let mut left = 0;
        while left < flags.len() {
            let mut right = left + 1;
            while right < flags.len() {
                let mut long = 0;
                while long < flags[left].longs.len() {
                    let mut other = 0;
                    while other < flags[right].longs.len() {
                        if str_eq(flags[left].longs[long], flags[right].longs[other]) {
                            return false;
                        }
                        other += 1;
                    }
                    long += 1;
                }

                let mut short = 0;
                while short < flags[left].shorts.len() {
                    let mut other = 0;
                    while other < flags[right].shorts.len() {
                        if flags[left].shorts[short] == flags[right].shorts[other] {
                            return false;
                        }
                        other += 1;
                    }
                    short += 1;
                }
                right += 1;
            }
            left += 1;
        }
        true
    }

    /// Reports whether a composed positional table has deterministic left-to-right binding.
    pub const fn positional_layout_valid(args: &[&Arg<'_>]) -> bool {
        let mut optional_seen = false;
        let mut variadic_seen = false;
        let mut index = 0;
        while index < args.len() {
            let arg = args[index];
            if variadic_seen || (arg.required && optional_seen) {
                return false;
            }
            if !arg.required {
                optional_seen = true;
            }
            if arg.variadic {
                variadic_seen = true;
            }
            index += 1;
        }
        true
    }

    /// Const-compatible string equality used by composed table validation.
    const fn str_eq(left: &str, right: &str) -> bool {
        let left = left.as_bytes();
        let right = right.as_bytes();
        if left.len() != right.len() {
            return false;
        }
        let mut index = 0;
        while index < left.len() {
            if left[index] != right[index] {
                return false;
            }
            index += 1;
        }
        true
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

    /// Static command metadata and generated typed-binding behavior.
    pub trait CommandArgs: Sized {
        /// Values collected so far during one parse.
        type Partial;

        /// Parse tables for this declaration.
        const COMMAND: &'static Command<'static>;

        /// Creates empty binding state for a new parse.
        fn start() -> Self::Partial;

        /// Applies one raw parser event when it belongs to this declaration.
        ///
        /// Returns whether this declaration owned the event. Occurrence policy is checked after
        /// raw argv parsing completes so syntax errors take precedence over binding errors.
        fn apply(partial: &mut Self::Partial, event: &Event<'_, '_>) -> bool;

        /// Validates completed occurrence cardinality before requiredness or conversion.
        ///
        /// # Errors
        ///
        /// Returns an error when a scalar argument occurred more than once.
        fn check_occurrences(partial: &mut Self::Partial) -> Result<(), crate::Error>;

        /// Validates required fields after every occurrence check has succeeded.
        ///
        /// # Errors
        ///
        /// Returns an error when a required argument was not supplied.
        fn check_required(partial: &mut Self::Partial) -> Result<(), crate::Error>;

        /// Validates completed occurrence and requiredness state before conversion.
        ///
        /// # Errors
        ///
        /// Returns an error when typed cardinality or requiredness is not satisfied.
        fn check(partial: &mut Self::Partial) -> Result<(), crate::Error> {
            Self::check_occurrences(partial)?;
            Self::check_required(partial)
        }

        /// Converts completed raw binding state into the destination Rust value.
        ///
        /// # Errors
        ///
        /// Returns an error when a required value is absent or a supplied value cannot be
        /// converted to the destination field type.
        fn finish(partial: Self::Partial) -> Result<Self, crate::Error>;
    }

    /// Marker implemented only by reusable declarations derived with `Args`.
    pub trait FlattenArgs: CommandArgs {}

    /// Static command metadata exposed by a derived subcommand enum.
    ///
    /// Variant table generation arrives with subcommand support. The empty default keeps the
    /// composition contract in place while that feature is not implemented yet.
    pub trait Subcommands: Sized {
        /// Parse tables for the enum's named subcommands.
        const COMMANDS: &'static [&'static Command<'static>] = &[];
    }

    /// Converts one raw value directly into a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is not valid UTF-8.
    pub fn text_value(value: Vec<u8>, name: &'static str) -> Result<String, crate::Error> {
        crate::binding::text_value(value, name)
    }

    /// Converts repeated raw values directly into UTF-8 strings.
    ///
    /// # Errors
    ///
    /// Returns the first invalid UTF-8 value.
    pub fn text_values(
        values: Vec<Vec<u8>>,
        name: &'static str,
    ) -> Result<Vec<String>, crate::Error> {
        crate::binding::text_values(values, name)
    }

    /// Converts one raw value through UTF-8 and the destination type's `FromStr` implementation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid UTF-8 or a destination conversion failure.
    pub fn parsed_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, crate::Error>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        crate::binding::parsed_value(value, name)
    }

    /// Converts repeated raw values through UTF-8 and `FromStr`.
    ///
    /// # Errors
    ///
    /// Returns the first conversion failure.
    pub fn parsed_values<T>(
        values: Vec<Vec<u8>>,
        name: &'static str,
    ) -> Result<Vec<T>, crate::Error>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        crate::binding::parsed_values(values, name)
    }

    /// Converts one raw value into an operating-system-backed destination type.
    ///
    /// # Errors
    ///
    /// Returns an error when the encoded bytes cannot be reconstructed as an operating-system
    /// string.
    pub fn os_value<T>(value: Vec<u8>, name: &'static str) -> Result<T, crate::Error>
    where
        T: From<std::ffi::OsString>,
    {
        crate::binding::os_value(value, name)
    }

    /// Converts repeated raw values into an operating-system-backed destination type.
    ///
    /// # Errors
    ///
    /// Returns the first operating-system string reconstruction failure.
    pub fn os_values<T>(values: Vec<Vec<u8>>, name: &'static str) -> Result<Vec<T>, crate::Error>
    where
        T: From<std::ffi::OsString>,
    {
        crate::binding::os_values(values, name)
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
