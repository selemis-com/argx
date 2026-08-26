//! Derive-first command-line argument parsing from Rust data types.
//!
//! Argx derives a static command model from Rust structs and enums. The same model drives raw argv
//! parsing, typed value binding, generated help and version output, diagnostics, and
//! machine-readable invocation contracts. Normal applications therefore define the command once
//! rather than maintaining separate parser, help, and discovery schemas.
//!
//! # Quick start
//!
//! A [`Parser`] struct defines the root command. A `#[derive(Subcommand)]` enum selects exact child
//! commands, and an [`Args`] struct provides fields for a subcommand or reusable flattened group.
//!
//! ```
//! use argx::{Args, Parser, Subcommand};
//!
//! /// Inspect stored objects.
//! #[derive(Debug, Parser)]
//! #[argx(name = "acme")]
//! struct Cli {
//!     /// Enable verbose diagnostics.
//!     #[argx(long, global)]
//!     verbose: bool,
//!
//!     #[argx(subcommand)]
//!     command: Command,
//! }
//!
//! #[derive(Debug, Subcommand)]
//! enum Command {
//!     /// Read one object.
//!     Get(GetArgs),
//!     /// Print service status.
//!     Status,
//! }
//!
//! #[derive(Debug, Args)]
//! struct GetArgs {
//!     /// Object identifier.
//!     id: String,
//! }
//!
//! let cli = Cli::try_parse_from(["acme", "get", "object-7", "--verbose"])?;
//! assert!(cli.verbose);
//! match cli.command {
//!     Command::Get(args) => assert_eq!(args.id, "object-7"),
//!     Command::Status => unreachable!(),
//! }
//! # Ok::<(), argx::Error>(())
//! ```
//!
//! [`Parser::parse`] is the ordinary process entry point. The `try_parse*` methods expose the same
//! parser without process exit behavior and are useful for tests, embedding, and custom error
//! handling.
//!
//! # Command model
//!
//! Argx has three derive roles:
//!
//! - `#[derive(Parser)]` applies to a named-field or unit struct and defines the root command.
//! - `#[derive(Args)]` applies to a named-field or unit struct and defines reusable arguments.
//! - `#[derive(Subcommand)]` applies to a non-empty enum. Each variant is either a unit command or
//!   contains exactly one direct `Args` payload.
//!
//! Command and variant names default to kebab-case. `#[argx(name = "...")]` replaces the canonical
//! spelling. Subcommand variants may additionally declare hidden `alias` or `aliases` spellings.
//! Root-command aliases are deliberately unsupported so the root has one canonical identity.
//!
//! A field with `#[argx(subcommand)]` selects one child from a derived subcommand enum. Selection
//! is exact; Argx does not perform prefix matching. Once a child is selected, parsing enters that
//! child's lexical scope.
//!
//! A field with `#[argx(flatten)]` composes one direct `Args` declaration into the current command.
//! Flattening does not create a new command scope: its arguments participate in the containing
//! command's parsing, validation, help, and machine contract. A flatten field's Rust documentation
//! becomes a help-group heading when present.
//!
//! Named options are local to their declaring command unless marked `#[argx(global)]`. Global
//! options remain visible in descendant scopes. If an ancestor and descendant use the same
//! spelling, the nearest active command scope wins.
//!
//! # Arguments and cardinality
//!
//! A field is positional unless `long` or `short` is present. A bare `#[argx(long)]` infers the
//! kebab-case field name; a bare `#[argx(short)]` infers its first character. Explicit spellings
//! are accepted with `long = "..."` and `short = 'x'`. Named fields may add hidden long spellings
//! with `alias` or `aliases`.
//!
//! The derive recognizes these field shapes syntactically:
//!
//! | Rust shape | Binding semantics |
//! | --- | --- |
//! | named `bool` | value-less switch |
//! | `T` | exactly one required value |
//! | `Option<T>` | zero or one value |
//! | `Vec<T>` | zero or more values |
//! | `Option<Vec<T>>` | optional zero-or-more collection |
//!
//! For a named collection, each occurrence consumes one value and the option may repeat. For a
//! positional collection, values are consumed variadically according to the statically validated
//! positional layout.
//!
//! Value conversion depends on the direct value type:
//!
//! - `String` consumes UTF-8 text;
//! - `OsString` and `PathBuf` preserve operating-system strings;
//! - other value types are converted through [`std::str::FromStr`].
//!
//! Type-shape inference is intentionally syntactic. Special treatment for `bool`, `Option`, `Vec`,
//! `String`, `OsString`, and `PathBuf` requires a recognized standard spelling to appear directly
//! in the field type. A type alias around one of those types is an ordinary `FromStr` value type
//! because a procedural macro cannot resolve type aliases.
//!
//! # Argv grammar
//!
//! Argx accepts long options as either `--name value` or `--name=value`. Short options may be
//! bundled. A value-taking short option consumes the remainder of its bundle as its value, or the
//! next token when no remainder exists. Bundles are validated before any event from the bundle is
//! committed, so a failing member does not partially apply earlier members.
//!
//! `--` stops flag interpretation in the active command scope. Ordinary flag-like detached values
//! are refused by default. `allow_hyphen_values` permits them for a named value-taking option;
//! `allow_negative_numbers` permits recognized negative-number spellings while continuing to
//! reject other flag-like values. The latter is also available for positional values.
//!
//! Raw argv is handled as operating-system strings. On Unix, `OsString` and `PathBuf` fields can
//! therefore receive non-UTF-8 argv without forcing a lossy conversion. `String` and `FromStr`
//! destinations require UTF-8 and report an [`Error`] when conversion is not possible.
//!
//! # Value sources
//!
//! Scalar value-taking named options may declare `#[argx(env = "NAME")]` and
//! `#[argx(default = expression)]`. Argv is authoritative when supplied; the environment is
//! consulted only when argv did not provide the option. A typed default satisfies absence without
//! round-tripping the expression through command-line text.
//!
//! Environment and typed-default metadata are intentionally restricted to scalar value-taking
//! named options. They are not accepted on switches, collections, positionals, flatten fields, or
//! subcommand selectors.
//!
//! # Argument relationships
//!
//! `requires` and `conflicts` express relationships between argument fields in one composed command
//! context. References use Rust field names and are validated during derivation/composition.
//!
//! `requires` is conditional: when the source is given through argv or its environment fallback,
//! the target must be satisfied. A typed default counts as a satisfied target. `conflicts` rejects
//! the case where both source and target were given; a default alone does not make an argument
//! conflict with another argument.
//!
//! ```
//! #[derive(argx::Parser)]
//! struct Cli {
//!     #[argx(long, requires = "token")]
//!     remote: bool,
//!
//!     #[argx(long)]
//!     token: Option<String>,
//!
//!     #[argx(long, conflicts = "remote")]
//!     offline: bool,
//! }
//! ```
//!
//! # Help and version metadata
//!
//! Help is generated from the same static command model used for parsing. Every command scope has
//! built-in `-h` and `--help`. A root command or subcommand variant that declares `version` or
//! `long_version` also receives `-V` and `--version`; if only one version expression is supplied,
//! it is used for both forms.
//!
//! Rust documentation participates directly in help generation:
//!
//! - the first prose paragraph is the one-line summary used in command listings and argument rows;
//! - the command's prose before the first level-one heading is its full description;
//! - level-one Markdown headings become user-authored help sections after generated sections;
//! - documentation on a flatten field becomes the flattened group's heading.
//!
//! `about = "..."` explicitly replaces the command's derived descriptive text; `help = "..."`
//! replaces a field's derived one-line summary. Hidden flag and subcommand aliases are accepted by
//! parsing but omitted from generated help so help presents one canonical interface.
//!
//! [`Parser::render_help`] renders the root scope directly. During parsing, help and version are
//! represented as [`Error::DisplayHelp`] and [`Error::DisplayVersion`] terminal actions. The
//! process-oriented parsing methods print those actions to stdout and exit successfully. Other
//! parse/binding errors go to stderr and exit with status 2.
//!
//! # Machine-readable invocation contracts
//!
//! [`Parser::contract`] exposes a versioned description of how the CLI can be invoked. This is
//! derived from Argx's internal command metadata rather than from a separate reflection registry.
//!
//! ```
//! use argx::{ContractRequest, Parser as _};
//!
//! # #[derive(argx::Args)]
//! # struct GetArgs { id: String }
//! # #[derive(argx::Subcommand)]
//! # enum Command { Get(GetArgs) }
//! # #[derive(argx::Parser)]
//! # struct Cli { #[argx(subcommand)] command: Command }
//! let contract = Cli::contract(ContractRequest::new(["get"]).recursive())?;
//! assert_eq!(contract.version, argx::CONTRACT_VERSION);
//! assert_eq!(contract.command.path, ["get"]);
//! # Ok::<(), argx::ContractError>(())
//! ```
//!
//! A [`Contract`] contains the canonical root and selected command, command aliases, direct
//! invocability, the root-to-selected invocation contexts, positional and named arguments, value
//! cardinality, global scope, environment/default sources, and normalized `requires` / `conflicts`
//! relationships. Command paths supplied in a [`ContractRequest`] may use aliases; returned paths
//! always use canonical command names.
//!
//! [`ContractDepth::Shallow`] includes the selected command in full and direct children as
//! summaries. [`ContractDepth::Recursive`] expands the selected command's complete descendant
//! subtree. [`Contract::to_json`] and [`Contract::to_json_pretty`] serialize the public protocol,
//! whose current version is [`CONTRACT_VERSION`].
//!
//! The contract is deliberately an **invocation contract**. It describes command structure,
//! accepted inputs, cardinality, sources, and relationships; it is not a serialization schema for
//! the concrete Rust value types stored in fields.
//!
//! # Failure model
//!
//! Parsing is terminal on the first public action or failure. [`Error`] is owned, non-exhaustive,
//! and preserves the relevant command-line bytes for diagnostics. Invalid values are classified
//! separately from structural argv errors such as unknown options, missing values, duplicate
//! scalar arguments, unknown commands, missing subcommands, and unsatisfied relationships.
//!
//! [`Parser::parse`], [`Parser::parse_from`], and [`Parser::parse_args`] provide conventional CLI
//! process behavior. [`Parser::try_parse`], [`Parser::try_parse_from`], and
//! [`Parser::try_parse_args`] return the error/action to the caller instead.
//!
//! # Derive restrictions
//!
//! The derive surface rejects ambiguous or unsupported shapes rather than approximating them:
//!
//! - `Parser` and `Args` do not support tuple structs;
//! - a command may have at most one direct `subcommand` field;
//! - `flatten` and `subcommand` fields must hold their derived type directly, not through `Option`
//!   or collection wrappers;
//! - subcommand variants support only unit variants or one unnamed direct `Args` payload;
//! - nested `Option` / `Vec` wrappers outside the recognized shapes are unsupported;
//! - reserved built-in help/version spellings and invalid composed layouts are rejected during
//!   derivation or const-time composition.
//!
//! These restrictions keep generated metadata statically coherent and make unsupported behavior a
//! compile-time error.
//!
//! # Platform support
//!
//! The supported native targets are Linux and macOS. Windows is supported through the Windows
//! Subsystem for Linux (WSL); native Windows targets are not supported. The parser still uses
//! [`OsString`] internally so supported Unix targets do not need to force argv through UTF-8.
//!
//! # Cargo features
//!
//! The default `derive` feature exports the `Parser`, `Args`, and `Subcommand` procedural macros.
//! Disabling it removes those macros while retaining Argx's runtime traits, error types, and
//! machine-contract data types.

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
    ArgumentContract, CONTRACT_VERSION, CommandContextContract, CommandContract,
    ConstraintContract, ConstraintContractKind, Contract, ContractDepth, ContractError,
    ContractRequest, InvocationContract, OptionContract, ValueContract,
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
///
/// # Examples
///
/// ```
/// use argx::Parser as _;
///
/// #[derive(argx::Args)]
/// struct Output {
///     #[argx(long)]
///     json: bool,
/// }
///
/// #[derive(argx::Parser)]
/// struct Cli {
///     #[argx(flatten)]
///     output: Output,
/// }
///
/// let cli = Cli::try_parse_from(["tool", "--json"])?;
/// assert!(cli.output.json);
/// # Ok::<(), argx::Error>(())
/// ```
pub trait Args: Sized + __private::CommandArgs + __private::CommandContract {}

/// Parses command-line arguments into a typed value.
///
/// The derive generates one static command model and the hidden binding implementation required by
/// these entry points. Prefer the `try_parse*` methods when the caller owns process policy; the
/// corresponding `parse*` methods are convenience entry points for ordinary CLI binaries.
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
    /// # Examples
    ///
    /// ```
    /// use argx::Parser as _;
    ///
    /// #[derive(argx::Parser)]
    /// struct Cli {
    ///     #[argx(long)]
    ///     port: u16,
    /// }
    ///
    /// let cli = Cli::try_parse_from(["server", "--port", "8080"])?;
    /// assert_eq!(cli.port, 8080);
    /// # Ok::<(), argx::Error>(())
    /// ```
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
    /// This entry point is useful when `argv` is already separated from the executable name, such
    /// as in tests, embedded command dispatch, or an agent invoking a command directly. Unlike
    /// [`Self::try_parse_from`], the first item is a real argument and is not discarded.
    ///
    /// # Examples
    ///
    /// ```
    /// use argx::Parser as _;
    ///
    /// #[derive(argx::Parser)]
    /// struct Cli {
    ///     value: String,
    /// }
    ///
    /// let cli = Cli::try_parse_args(["payload"])?;
    /// assert_eq!(cli.value, "payload");
    /// # Ok::<(), argx::Error>(())
    /// ```
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
    /// Returned paths always use canonical command names. Contract discovery does not parse
    /// process arguments or evaluate environment fallbacks.
    ///
    /// # Examples
    ///
    /// ```
    /// use argx::{ContractRequest, Parser as _};
    ///
    /// #[derive(argx::Args)]
    /// struct GetArgs {
    ///     id: String,
    /// }
    ///
    /// #[derive(argx::Subcommand)]
    /// enum Command {
    ///     Get(GetArgs),
    /// }
    ///
    /// #[derive(argx::Parser)]
    /// struct Cli {
    ///     #[argx(subcommand)]
    ///     command: Command,
    /// }
    ///
    /// let contract = Cli::contract(ContractRequest::new(["get"]))?;
    /// assert_eq!(contract.command.path.len(), 1);
    /// assert_eq!(contract.command.path[0], "get");
    /// assert!(contract.command.invocation.is_some());
    /// # Ok::<(), argx::ContractError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownCommand`] when one requested path segment does not resolve.
    fn contract(request: ContractRequest) -> Result<Contract, ContractError> {
        contract::discover(Self::CONTRACT, request)
    }

    /// Renders generated help for this root command.
    ///
    /// This renders only the root scope. Help selected while parsing a child command is returned as
    /// [`Error::DisplayHelp`] by the `try_parse*` methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use argx::Parser as _;
    ///
    /// /// Inspect one input file.
    /// #[derive(argx::Parser)]
    /// struct Cli {
    ///     /// Input path.
    ///     input: String,
    /// }
    ///
    /// let help = Cli::render_help();
    /// assert!(help.contains("Usage:"));
    /// assert!(help.contains("<INPUT>"));
    /// ```
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
