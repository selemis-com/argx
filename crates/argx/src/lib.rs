//! Command line argument parser for Rust.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/logo.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod argv;
mod binding;
pub mod contract;
mod error;
mod help;

use std::ffi::{OsStr, OsString};

pub use contract::{
    ArgumentCardinality, ArgumentContract, ArgumentSyntax, CONTRACT_VERSION,
    CommandContextContract, CommandContract, ConstraintContract, ConstraintContractKind, Contract,
    ContractDepth, ContractError, ContractRequest, InvocationContract,
};
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

/// Marks a reusable argument group derived with `#[derive(Args)]`.
///
/// This trait distinguishes reusable argument groups from root [`Parser`] declarations. It is
/// implemented by the `Args` derive and is not intended for manual implementation.
pub trait Args: Sized + __private::CommandArgs + __private::CommandContract {}

/// Parses command-line arguments into a typed value.
pub trait Parser: Sized + __private::CommandArgs + __private::CommandContract {
    /// Parses the current process arguments, excluding the program name.
    ///
    /// Help and version requests are printed to standard output and terminate successfully. Parse
    /// failures are printed to standard error and terminate the process with status 2.
    fn parse() -> Self {
        Self::try_parse().unwrap_or_else(|error| error.exit())
    }

    /// Parses the current process arguments, excluding the program name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DisplayHelp`] or [`Error::DisplayVersion`] when the corresponding built-in
    /// action is requested, or an error when argv cannot be bound to this command or a bound value
    /// cannot be converted to its Rust field type.
    fn try_parse() -> Result<Self, Error> {
        Self::try_parse_args(std::env::args_os().skip(1))
    }

    /// Parses a complete argv sequence whose first item is the program name.
    ///
    /// Help and version requests are printed to standard output and terminate successfully. Parse
    /// failures are printed to standard error and terminate the process with status 2.
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
    /// Returns [`Error::DisplayHelp`] or [`Error::DisplayVersion`] when the corresponding built-in
    /// action is requested, or an error when argv cannot be bound to this command or a bound value
    /// cannot be converted to its Rust field type.
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
    /// Help and version requests are printed to standard output and terminate successfully. Parse
    /// failures are printed to standard error and terminate the process with status 2.
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
    /// Returns [`Error::DisplayHelp`] or [`Error::DisplayVersion`] when the corresponding built-in
    /// action is requested, or an error when argv cannot be bound to this command or a bound value
    /// cannot be converted to its Rust field type.
    fn try_parse_args<I, T>(argv: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let owned: Vec<OsString> = argv.into_iter().map(Into::into).collect();
        let refs: Vec<&OsStr> = owned.iter().map(OsString::as_os_str).collect();
        binding::parse_refs::<Self>(&refs)
    }

    /// Discovers the machine-readable invocation contract for this CLI.
    ///
    /// Command paths are relative to the root command and may use canonical names or aliases.
    /// Returned paths always use canonical command names.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownCommand`] when one requested path segment does not resolve.
    fn contract(request: ContractRequest) -> Result<Contract, ContractError> {
        contract::discover(Self::CONTRACT, request)
    }

    /// Renders generated help for this root command.
    #[must_use]
    fn render_help() -> String {
        help::render(&[Self::COMMAND])
    }
}

/// Implementation details shared with generated code.
///
/// This module is public so proc-macro expansions can name these items from downstream crates. It
/// is not part of Argx's stable user-facing API.
#[doc(hidden)]
#[path = "private/mod.rs"]
pub mod __private;
