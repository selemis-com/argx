//! Derive-first command-line parsing and unified configuration from Rust data types.
//!
//! Argx derives static command and configuration models from Rust data types. Command metadata
//! drives raw argv parsing, typed value binding, generated help and version output, diagnostics,
//! dynamic shell completion, and schema discovery. Configuration metadata resolves the same typed
//! fields across explicitly ordered defaults, files, environment values, and argv.
//!
//! `#[derive(Config)]` treats defaults, TOML, dotenv, process environment, and argv as ordered
//! sources for one resolved Rust value rather than separate configuration systems.
//!
//! # Installation
//!
//! Add Argx with its default derive support:
//!
//! ```text
//! cargo add argx
//! ```
//!
//! The default `derive` feature re-exports the procedural macros used throughout this guide.
//! Enable the optional `toml` feature to add the `Toml` configuration layer. Disable default
//! features only when a crate needs the runtime API without derive macros.
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
//! # Unified configuration
//!
//! `#[derive(Config)]` generates one typed configuration contract. A generated `loader()` starts
//! empty; applications add [`Defaults`], [`Dotenv`], [`Environment`], and [`Argv`] layers
//! explicitly, in the precedence order they want. The optional `toml` feature adds `Toml`:
//!
//! ```
//! use argx::{Argv, Defaults};
//!
//! #[derive(Debug, argx::Config)]
//! struct Config {
//!     #[argx(long, default = 4)]
//!     workers: usize,
//!
//!     #[argx(long)]
//!     endpoint: String,
//! }
//!
//! let config = Config::loader()
//!     .layer(Defaults)
//!     .layer(Argv::new(["acme", "--endpoint", "http://localhost"]))
//!     .resolve()?;
//!
//! assert_eq!(config.workers, 4);
//! assert_eq!(config.endpoint, "http://localhost");
//! # Ok::<(), argx::ConfigError>(())
//! ```
//!
//! Layers are sparse and are applied in call order. A later layer replaces only fields it supplies;
//! an absent value never masks an earlier one. Declared field defaults are therefore not implicit:
//! they take effect only when [`Defaults`] appears in the layer stack. Non-optional fields are
//! required only after all configured layers have been resolved.
//!
//! ## Configuration attributes
//!
//! A `Config` declaration accepts `#[argx(prefix = "...")]`. The prefix maps ordinary fields to
//! environment variables by uppercasing field components and joining them with `_`. For example,
//! `#[argx(prefix = "ACME")]` maps `workers` to `ACME_WORKERS`. A flattened `server.workers`
//! field maps to `ACME_SERVER_WORKERS`.
//!
//! Configuration fields accept:
//!
//! | Attribute | Meaning |
//! | --- | --- |
//! | `default` | use the field type's [`std::default::Default`] implementation in a [`Defaults`] layer |
//! | `default = expression` | use a typed Rust expression in a [`Defaults`] layer |
//! | `env = "NAME"` | map the field to one exact environment variable |
//! | `flatten` | compose one direct nested `Config` across every layer |
//! | `long`, `short` | expose the field through argv using the normal named-option spelling rules |
//! | `alias`, `aliases`, `global`, `value_enum`, `allow_hyphen_values`, `allow_negative_numbers`, `help` | forward normal CLI metadata to the generated argv field |
//!
//! A field participates in [`Argv`] only when it has CLI metadata such as `long` or `short`.
//! Configuration-only fields need no CLI annotation. A flattened field always composes its nested
//! argv surface, but does not itself accept `default` or `env`.
//!
//! With the `toml` feature enabled, `Toml` reads exactly the path supplied to the layer and
//! rejects unknown fields. Argx performs no configuration-file discovery. [`Dotenv`] likewise reads
//! exactly the supplied dotenv path; [`Environment`] contributes the current process environment.
//! TOML interpolation can observe environment values accumulated by earlier `Dotenv` and
//! `Environment` layers, so moving an environment layer after a TOML layer also removes it from
//! that TOML layer's interpolation scope.
//!
//! [`Argv::new`] expects a complete argument vector including the program name. [`Argv::current`]
//! captures the current process argv in that form.
//!
//! # Entry points and embedding
//!
//! All parser entry points use the same generated command model; they differ only in where argv
//! comes from and who owns terminal/process policy:
//!
//! | Entry point | Input | Process policy |
//! | --- | --- | --- |
//! | [`Parser::parse`] | current process, excluding program name | Argx prints and exits |
//! | [`Parser::try_parse`] | current process, excluding program name | caller receives [`Error`] |
//! | [`Parser::parse_from`] | complete argv including program name | Argx prints and exits |
//! | [`Parser::try_parse_from`] | complete argv including program name | caller receives [`Error`] |
//! | [`Parser::parse_args`] | arguments only | Argx prints and exits |
//! | [`Parser::try_parse_args`] | arguments only | caller receives [`Error`] |
//!
//! The `try_parse*` methods represent built-in help and version requests as terminal actions in the
//! error type. Embedding code can therefore preserve Argx's parser semantics while choosing its
//! own transport, logging, or exit policy:
//!
//! ```
//! use argx::{Error, Parser as _};
//!
//! #[derive(argx::Parser)]
//! struct Cli {
//!     input: String,
//! }
//!
//! match Cli::try_parse_args(["--help"]) {
//!     Err(Error::DisplayHelp { help }) => assert!(help.contains("Usage:")),
//!     _ => panic!("expected the built-in help action"),
//! }
//! ```
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
//! command's parsing, validation, and help. A flatten field's Rust documentation
//! becomes a help-group heading when present.
//!
//! Named options are local to their declaring command unless marked `#[argx(global)]`. Global
//! options remain visible in descendant scopes. If an ancestor and descendant use the same
//! spelling, the nearest active command scope wins.
//!
//! # `#[argx(...)]` attribute reference
//!
//! Rust documentation is the preferred source for user-facing descriptions. `#[argx(...)]`
//! metadata controls command-line semantics or provides an explicit override where Rust docs are
//! not the desired CLI text.
//!
//! ## `Parser` and `Args` declarations
//!
//! Struct declarations accept `name = "..."` and `about = "..."`. `name` replaces the inferred
//! kebab-case command name; `about` replaces documentation-derived descriptive text. A `Parser`
//! declaration may additionally use `version = expression`, `long_version = expression`, and the
//! marker `schema`. If only one version expression is supplied, Argx uses it for both `-V` and
//! `--version`. `schema` enables machine-readable discovery. Structural `Args` declarations that
//! contain a subcommand field may also use `schema` to participate in that command topology.
//! Version metadata remains root-only.
//!
//! Command aliases are intentionally not accepted on structs: aliases belong to selectable
//! `Subcommand` variants. An `Args` declaration has no standalone process entry point. Flattening
//! it does not create a command scope, and using it as a subcommand payload keeps the enum variant
//! as the visible command.
//!
//! ## `Subcommand` variants
//!
//! The enum itself accepts the `schema` marker when it participates in machine-readable command
//! topology. Individual variants accept:
//!
//! - `name = "..."` to replace the inferred kebab-case command spelling;
//! - `about = "..."` to override documentation-derived descriptive text;
//! - `alias = "..."` for one hidden accepted command spelling;
//! - `aliases = ["...", "..."]` for multiple hidden accepted spellings;
//! - `version = expression` and `long_version = expression` for version actions local to that
//!   command scope.
//!
//! Canonical names and aliases share one sibling namespace. Aliases are accepted by parsing and
//! dynamic lookup but omitted from human help.
//!
//! ## Argument fields
//!
//! Ordinary fields are positional unless `long` or `short` is present. The supported field
//! metadata is:
//!
//! | Attribute | Meaning |
//! | --- | --- |
//! | `long` / `long = "name"` | infer or explicitly set a long option spelling |
//! | `short` / `short = 'x'` | infer or explicitly set a short option spelling |
//! | `alias = "name"` | add one hidden long spelling to a named option |
//! | `aliases = ["a", "b"]` | add multiple hidden long spellings to a named option |
//! | `global` | keep a named option visible in descendant command scopes |
//! | `default = expression` | use a typed Rust default for a scalar value-taking named option |
//! | `requires = "field"` | require another argument when this argument is supplied |
//! | `requires = ["a", "b"]` | require multiple arguments |
//! | `conflicts = "field"` | reject use with another argument |
//! | `conflicts = ["a", "b"]` | reject use with multiple arguments |
//! | `allow_hyphen_values` | allow arbitrary flag-like detached values for a named value option |
//! | `allow_negative_numbers` | accept negative-number values without accepting other flags |
//! | `value_enum` | use a finite [`trait@ValueEnum`] vocabulary for parsing, help, and completion |
//! | `help = "..."` | override the field's documentation-derived one-line help text |
//! | `flatten` | compose one direct [`Args`] field into the current command |
//! | `subcommand` | select one direct derived `Subcommand` enum |
//!
//! Long and alias spellings are written without leading dashes. A short spelling is one visible
//! ASCII character other than `-` or `=`.
//!
//! `alias` / `aliases` and `global` require a named option. `default` is restricted to scalar
//! value-taking named options, so it cannot be used on switches or collections.
//! `allow_hyphen_values` is named-option only; `allow_negative_numbers` may also be used on a
//! positional value. Value policies do not apply to `bool` switches.
//!
//! `requires` and `conflicts` refer to Rust field names in the composed command context, including
//! fields contributed by `flatten`. Structural `flatten` and `subcommand` fields cannot also carry
//! ordinary flag, relationship, or `help` metadata. Their types must be held directly
//! rather than through `Option` or collection wrappers.
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
//! - a field marked `#[argx(value_enum)]` parses through its finite [`trait@ValueEnum`] vocabulary;
//! - `String` consumes UTF-8 text;
//! - `OsString` and `PathBuf` preserve operating-system strings;
//! - other value types are converted through [`std::str::FromStr`].
//!
//! ## Finite values
//!
//! Arbitrary [`std::str::FromStr`] implementations are intentionally opaque to Argx. When a value
//! has a finite command-line vocabulary, derive [`ValueEnum`] and mark the field with
//! `#[argx(value_enum)]`. The enum declaration then becomes the source of truth for parsing, help,
//! and completion, so accepted values do not need to be repeated elsewhere.
//!
//! ```
//! #[derive(Debug, argx::ValueEnum)]
//! enum Output {
//!     HumanReadable,
//!     Json,
//! }
//!
//! #[derive(argx::Parser)]
//! struct Cli {
//!     /// Output format.
//!     #[argx(long, value_enum)]
//!     output: Output,
//! }
//! ```
//!
//! Derived variants use Argx's normal kebab-case spelling, and parsing is exact and case-sensitive.
//! The derive also implements [`std::str::FromStr`] for ordinary Rust use. The marker works through
//! Argx's scalar, `Option`, `Vec`, and `Option<Vec<_>>` field shapes and applies the vocabulary to
//! the logical element value.
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
//! # Typed defaults
//!
//! Scalar value-taking named options may declare `#[argx(default = expression)]`. A typed default
//! satisfies absence without round-tripping the expression through command-line text. Defaults are
//! not accepted on switches, collections, positionals, flatten fields, or subcommand selectors.
//!
//! # Argument relationships
//!
//! `requires` and `conflicts` express relationships between argument fields in one composed command
//! context. References use Rust field names and are validated during derivation/composition.
//!
//! `requires` is conditional: when the source is given through argv,
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
//! # Help, version, and schema discovery
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
//! A parser marked `#[argx(schema)]` additionally exposes `-S` and `--schema` in every selected
//! command scope and the root `schema [COMMAND]...` pseudo-command. Structural `Args` and
//! `Subcommand` declarations opt into topology with the same `#[argx(schema)]` marker; executable
//! leaves are associated with result and error schemas through `#[argx(handler = ...)]`.
//!
//! [`Parser::render_help`] renders the root scope directly. During parsing, help, version, and
//! schema discovery are represented as [`Error::DisplayHelp`], [`Error::DisplayVersion`], and
//! [`Error::DisplaySchema`] terminal actions. The process-oriented parsing methods print those
//! actions to stdout and exit successfully. Other parse/binding errors go to stderr and exit with
//! status 2.
//!
//! # Shell completions
//!
//! Argx generates dynamic completion adapters for Bash, Fish, Nushell, and Zsh through the
//! [`completion`] module. Bash, Fish, and Zsh send the command line through the cursor back to the
//! executable, while Nushell forwards the tokenized spans its external-completer API already
//! provides. Argx normalizes either form and walks completed argv words through the same raw argv
//! parser used for ordinary invocation. Command selection, aliases, global scope, lexical
//! shadowing, option repeatability, conflicts, negative-number routing, and `--` therefore do not
//! have a second shell-specific implementation.
//!
//! ```
//! use argx::{Parser as _, completion::Shell};
//!
//! # #[derive(argx::Parser)]
//! # #[argx(name = "acme")]
//! # struct Cli;
//! let script = Cli::render_completion(Shell::Zsh)?;
//! assert!(script.contains("#compdef acme"));
//! # Ok::<(), argx::completion::ScriptError>(())
//! ```
//!
//! [`Parser::parse`] handles requests from generated adapters automatically. A binary that uses
//! any other parser entry point for its current process should call [`Parser::handle_completion`]
//! first and return when it yields `true`.
//!
//! Completion includes canonical values for fields marked `#[argx(value_enum)]`, using the same
//! finite vocabulary already shared by parsing, help, and completion. Argx does not infer finite
//! choices from arbitrary [`std::str::FromStr`] implementations, enumerate filesystem paths, or
//! expose custom value completers. Hidden aliases remain accepted while reconstructing scope but
//! are never suggested.
//!
//! Applications normally expose the generated text through a small `completions <shell>` command,
//! as demonstrated by the `completions` example. The adapter can then be sourced from shell startup
//! or saved in the shell's normal completion location. For example, assuming an application named
//! `acme` exposes that command:
//!
//! ```text
//! # Bash
//! source <(acme completions bash)
//!
//! # Fish
//! acme completions fish | source
//!
//! # Zsh
//! source <(acme completions zsh)
//!
//! # Nushell
//! acme completions nushell | save --force ~/.cache/acme-completions.nu
//! source ~/.cache/acme-completions.nu
//! ```
//!
//! Because these adapters call the current executable dynamically, changes to the command tree do
//! not require regenerating a shell-specific copy of the CLI. Regenerate only when updating the
//! adapter itself is appropriate for the application installation.
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
//! - `value_enum` fields cannot depend on the containing command's generic parameters because their
//!   vocabulary is part of static command metadata;
//! - reserved built-in help/version/schema spellings and invalid composed layouts are rejected
//!   during derivation or const-time composition.
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
//! The default `derive` feature exports the `Parser`, `Args`, `Subcommand`, `ValueEnum`, and
//! `Config` derives plus the unified `argx` attribute macro. The optional `toml` feature enables
//! the `Toml` configuration layer and its `toml_edit` dependency.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/logo.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod cli;
pub mod completion;
pub mod config;
mod error;
mod schema;

use std::ffi::{OsStr, OsString};

pub use cli::value_enum::{ValueEnum, ValueEnumError};
pub use error::{Error, InvalidValue};

/// Compiler-facing marker for command declarations that are directly invocable.
#[doc(hidden)]
pub trait InvocableHandlerCommand: __private::InvocableCommandHandler {}

/// Schema source generated by `#[argx(handler = CommandType)]`.
#[doc(hidden)]
pub use cli::protocol::HandlerSchemaSource;

// Generated absolute paths must also work when a derive is used inside this crate. Integration
// targets already receive this name through Cargo; the library target needs the self alias.
#[cfg_attr(
    not(test),
    expect(
        unused_extern_crates,
        reason = "proc-macro expansions refer to this crate through `::argx`"
    )
)]
extern crate self as argx;

#[cfg(feature = "derive")]
pub use argx_derive::Config;
#[cfg(feature = "derive")]
pub use argx_derive::{Args, Parser, Subcommand, ValueEnum, argx};
#[cfg(feature = "toml")]
#[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
pub use config::Toml;
pub use config::{
    Argv, Defaults, Dotenv, Environment, Error as ConfigError, Layer, Loader as ConfigLoader,
};

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
pub trait Args: Sized + __private::CommandArgs {}

/// Parses command-line arguments into a typed value.
///
/// The derive generates one static command model and the hidden binding implementation required by
/// these entry points. Prefer the `try_parse*` methods when the caller owns process policy; the
/// corresponding `parse*` methods are convenience entry points for ordinary CLI binaries.
pub trait Parser: Sized + __private::CommandArgs {
    /// Parses the current process arguments, excluding the program name.
    ///
    /// Help, version, and schema requests are printed to standard output and terminate
    /// successfully. Parse failures are printed to standard error and terminate the process with
    /// status 2.
    fn parse() -> Self {
        if completion::handle_process::<Self>() {
            std::process::exit(0);
        }
        Self::try_parse().unwrap_or_else(|error| error.exit())
    }

    /// Parses the current process arguments, excluding the program name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DisplayHelp`], [`Error::DisplayVersion`], or [`Error::DisplaySchema`] when
    /// the corresponding built-in action is requested, or an error when argv cannot be bound to
    /// this command or a bound value cannot be converted to its Rust field type.
    fn try_parse() -> Result<Self, Error> {
        Self::try_parse_args(std::env::args_os().skip(1))
    }

    /// Parses a complete argv sequence whose first item is the program name.
    ///
    /// Help, version, and schema requests are printed to standard output and terminate
    /// successfully. Parse failures are printed to standard error and terminate the process with
    /// status 2.
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
    /// Returns [`Error::DisplayHelp`], [`Error::DisplayVersion`], or [`Error::DisplaySchema`] when
    /// the corresponding built-in action is requested, or an error when argv cannot be bound to
    /// this command or a bound value cannot be converted to its Rust field type.
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
    /// Help, version, and schema requests are printed to standard output and terminate
    /// successfully. Parse failures are printed to standard error and terminate the process with
    /// status 2.
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
    /// Returns [`Error::DisplayHelp`], [`Error::DisplayVersion`], or [`Error::DisplaySchema`] when
    /// the corresponding built-in action is requested, or an error when argv cannot be bound to
    /// this command or a bound value cannot be converted to its Rust field type.
    fn try_parse_args<I, T>(argv: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let owned: Vec<OsString> = argv.into_iter().map(Into::into).collect();
        let refs: Vec<&OsStr> = owned.iter().map(OsString::as_os_str).collect();
        if !<Self as __private::CommandArgs>::SCHEMA_ENABLED {
            return cli::binding::parse_refs::<Self>(&refs);
        }
        let registry = <Self as __private::CommandArgs>::schema_registry()
            .expect("schema-enabled parser must generate a schema registry");
        if let Some(error) = schema::pseudo_command(Self::COMMAND, &refs, &registry) {
            return Err(error);
        }
        cli::binding::parse_refs_with_schema::<Self>(&refs, &registry)
    }

    /// Handles a dynamic shell-completion request for the current process.
    ///
    /// Generated completion adapters use a private, versioned invocation protocol that is
    /// intercepted before ordinary argv parsing. [`Self::parse`] calls this automatically. Binaries
    /// that use another parser entry point for their current process should call this method first
    /// and return from `main` when it yields `true`.
    ///
    /// Ordinary invocations return `false` without writing output or changing parser state. Call
    /// this before expensive application startup or writing to standard output, because generated
    /// adapters invoke the process for every completion request.
    #[must_use]
    fn handle_completion() -> bool {
        completion::handle_process::<Self>()
    }

    /// Generates a dynamic completion adapter for this parser's configured root command name.
    ///
    /// Use [`completion::script`] instead when the installed executable name intentionally differs
    /// from the root command name.
    ///
    /// # Errors
    ///
    /// Returns [`completion::ScriptError::InvalidCommandName`] when the configured command name
    /// cannot be safely registered by every supported shell adapter.
    fn render_completion(shell: completion::Shell) -> Result<String, completion::ScriptError> {
        completion::script(Self::COMMAND.name, shell)
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
        cli::help::render_with_schema(
            &[Self::COMMAND],
            <Self as __private::CommandArgs>::SCHEMA_ENABLED,
        )
    }
}

/// Implementation details shared with generated code.
///
/// This module is public so proc-macro expansions can name these items from downstream crates. It
/// is not part of Argx's stable user-facing API.
#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
