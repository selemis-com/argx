//! Derive-first command-line parsing and configuration for Rust.
//!
//! Define your CLI and configuration with Rust types, and Argx derives parsing, help, diagnostics,
//! completions, schema discovery, and layered configuration from those definitions.
//!
//! # Installation
//!
//! ```text
//! cargo add argx
//! ```
//!
//! # Feature flags
//!
//! - `derive` enables the derive macros and is enabled by default.
//! - `toml` enables TOML configuration layers and implies `derive`.
//! - `chrono` enables schema integration for Chrono values. `DateTime` and `NaiveDate` receive the
//!   standard `date-time` and `date` formats. `NaiveTime` and `NaiveDateTime` remain lexical
//!   strings because JSON Schema has no standard format that faithfully represents their
//!   timezone-free values.
//! - `url` preserves the `uri` format for `url` values in invocation and typed schemas.
//! - `uuid` preserves the `uuid` format for `uuid` values in invocation and typed schemas.
//!
//! For example:
//!
//! ```text
//! cargo add argx --features chrono,toml,url,uuid
//! ```
//!
//! # Quick start
//!
//! ```
//! use argx::{Args, Parser, Subcommand};
//!
//! #[derive(Parser)]
//! #[argx(name = "acme")]
//! struct Cli {
//!     #[argx(subcommand)]
//!     command: Command,
//! }
//!
//! #[derive(Subcommand)]
//! enum Command {
//!     /// Start the service.
//!     Serve(Serve),
//!
//!     /// Print service status.
//!     Status,
//! }
//!
//! #[derive(Args)]
//! struct Serve {
//!     /// Port to listen on.
//!     #[argx(long, default = 8080)]
//!     port: u16,
//! }
//!
//! let cli = Cli::try_parse_from(["acme", "serve", "--port", "3000"])?;
//! match cli.command {
//!     Command::Serve(args) => assert_eq!(args.port, 3000),
//!     Command::Status => unreachable!(),
//! }
//! # Ok::<(), argx::Error>(())
//! ```
//!
//! Rust documentation becomes CLI help, while field types define parsing. [`Parser::parse`] is the
//! ordinary process entry point. The `try_parse*` methods return [`Error`] instead of printing and
//! exiting.
//!
//! # Configuration
//!
//! `#[derive(Config)]` builds a typed configuration value from explicitly ordered layers. A
//! generated `loader()` starts empty. Applications add [`Defaults`], [`Dotenv`], [`Environment`],
//! and [`Argv`] in the precedence order they want. The optional `toml` feature adds `Toml`:
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
//! Layers are applied in call order. A later layer replaces only fields it supplies. An absent
//! value never masks an earlier one. Declared field defaults are therefore not implicit:
//! they take effect only when [`Defaults`] appears in the layer stack. Non-optional fields are
//! required only after all configured layers have been resolved.
//!
//! For example, an application can define increasing precedence entirely by layer order:
//!
//! ```text
//! earlier layers                                      later layers
//! Defaults -> Dotenv -> Toml -> Environment -> Argv
//! ```
//!
//! This order is illustrative, not built in. Omitting or reordering a layer changes the
//! application's configuration policy.
//!
//! ## Configuration attributes
//!
//! A `Config` declaration accepts `#[argx(prefix = "...")]`. The prefix maps ordinary fields to
//! environment variables by uppercasing field components and joining them with `_`. For example,
//! `#[argx(prefix = "ACME")]` maps `workers` to `ACME_WORKERS`. A flattened `server.workers`
//! field maps to `ACME_SERVER_WORKERS`. Variables without a generated or explicit mapping are
//! ignored.
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
//! | `alias`, `aliases`, `global`, `delimited`, `value_enum`, `allow_hyphen_values`, `allow_negative_numbers`, `help` | forward normal CLI metadata to the generated argv field |
//!
//! A field participates in [`Argv`] only when it has CLI metadata such as `long` or `short`.
//! Configuration-only fields need no CLI annotation. A flattened field always composes its nested
//! argv surface, but does not itself accept `default` or `env`.
//!
//! [`Dotenv`] and `Toml` read only the paths supplied to their layers; Argx performs no
//! configuration-file discovery. [`Environment`] reads the current process environment. TOML
//! interpolation can use environment values supplied by earlier environment layers.
//!
//! [`Argv::new`] expects a complete argument vector including the program name. [`Argv::current`]
//! captures the current process argv in that form.
//!
//! # Commands and composition
//!
//! Argx has three derive roles:
//!
//! - `#[derive(Parser)]` applies to a named-field or unit struct and defines the root command.
//! - `#[derive(Args)]` applies to a named-field or unit struct and defines reusable arguments.
//! - `#[derive(Subcommand)]` applies to a non-empty enum. Each variant is either a unit command or
//!   contains exactly one direct `Args` payload.
//!
//! Command and variant names default to kebab-case. `#[argx(name = "...")]` replaces the canonical
//! spelling, and subcommand variants may declare hidden `alias` or `aliases` spellings.
//!
//! A field with `#[argx(subcommand)]` selects one child from a derived subcommand enum. Command
//! names are matched exactly.
//!
//! A field with `#[argx(flatten)]` composes one direct `Args` declaration into the current command.
//! Flattening does not create a new command scope: its arguments participate in the containing
//! command's parsing, validation, and help. To place those arguments in an explicit help section,
//! start the flatten field's Rust documentation with a level-one heading:
//!
//! ```
//! # #[derive(argx::Args)]
//! # struct Logging {
//! #     #[argx(long)]
//! #     verbose: bool,
//! # }
//! # #[derive(argx::Parser)]
//! # struct Cli {
//! /// # Logging
//! #[argx(flatten)]
//! logging: Logging,
//! # }
//! ```
//!
//! Ordinary prose documentation on a flattened field does not create a help section.
//! Named options are local to their declaring command unless marked `#[argx(global)]`. Global
//! options remain visible in descendant scopes. If an ancestor and descendant use the same
//! spelling, the nearest active command scope wins.
//!
//! # Arguments and cardinality
//!
//! A field is positional unless `long` or `short` is present. A bare `#[argx(long)]` infers the
//! kebab-case field name. A bare `#[argx(short)]` infers its first character. Explicit spellings
//! are accepted with `long = "..."` and `short = 'x'`. Named fields may add hidden long spellings
//! with `alias` or `aliases`.
//!
//! The derive recognizes these field shapes:
//!
//! | Rust shape | Binding semantics |
//! | --- | --- |
//! | named `bool` | value-less switch |
//! | `T` | exactly one required value |
//! | `Option<T>` | zero or one value |
//! | `Vec<T>` | zero or more values |
//! | `Option<Vec<T>>` | optional zero-or-more collection |
//!
//! Named collections may be repeated. Positional collections consume the remaining positional
//! values.
//!
//! Value conversion depends on the direct value type:
//!
//! - a field marked `#[argx(value_enum)]` parses through its finite [`trait@ValueEnum`] vocabulary.
//! - `String` consumes UTF-8 text.
//! - `OsString` and `PathBuf` preserve operating-system strings.
//! - other value types are converted through [`std::str::FromStr`].
//!
//! ## Finite values
//!
//! When a value has a fixed command-line vocabulary, derive [`ValueEnum`] and mark the field with
//! `#[argx(value_enum)]`. The enum then supplies the accepted values for parsing, help, and
//! completion.
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
//!     format: Output,
//! }
//! ```
//!
//! Derived variants use Argx's normal kebab-case spelling, and parsing is exact and case-sensitive.
//! The derive also implements [`std::str::FromStr`] for ordinary Rust use.
//!
//! ## Typed defaults
//!
//! Scalar named options may declare `#[argx(default = expression)]`. The expression is evaluated as
//! the field's Rust type and is used when the option is absent.
//!
//! ## Argument relationships
//!
//! `requires` and `conflicts` express relationships between argument fields in one composed command
//! context. References use Rust field names and are validated during derivation/composition.
//!
//! `requires` makes another field mandatory when the source argument is supplied. `conflicts`
//! rejects combinations that cannot be used together. Typed defaults satisfy requirements without
//! activating conflicts.
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
//! # Argv grammar
//!
//! Argx accepts long options as `--name value` or `--name=value`, supports short-option bundles,
//! and treats `--` as the end of option parsing.
//!
//! Detached values that look like options are rejected by default. Use `allow_hyphen_values` for
//! arbitrary flag-like values or `allow_negative_numbers` when only negative numbers should be
//! accepted. `OsString` and `PathBuf` preserve native argument strings; text and `FromStr` values
//! require UTF-8.
//!
//! # Parser entry points
//!
//! [`Parser::parse`] and [`Parser::try_parse`] read the current process arguments. The `*_from`
//! variants accept a complete argv sequence including the program name. `parse` methods print
//! terminal actions and errors and may exit the process; `try_parse` methods return [`Error`] to
//! the caller.
//!
//! ```
//! use argx::{Error, Parser as _};
//!
//! #[derive(argx::Parser)]
//! struct Cli {
//!     input: String,
//! }
//!
//! match Cli::try_parse_from(["acme", "--help"]) {
//!     Err(Error::DisplayHelp { help }) => assert!(help.contains("Usage:")),
//!     _ => panic!("expected the built-in help action"),
//! }
//! ```
//!
//! # Help and version
//!
//! Every command scope has built-in `-h` and `--help`. Commands with `version` or `long_version`
//! also receive `-V` and `--version`. If only one version is supplied, it is used for both forms.
//!
//! Rust documentation supplies command and argument descriptions. The first paragraph is used as
//! the short summary, and level-one headings create additional help sections. On a flattened field,
//! a leading level-one heading explicitly groups that field's composed arguments under the heading.
//! Ordinary prose on a flattened field remains documentation and does not create a section.
//!
//! `about = "..."` explicitly replaces the command's derived descriptive text. `help = "..."`
//! replaces a field's derived one-line summary. Hidden flag and subcommand aliases are accepted by
//! parsing but omitted from generated help so help presents one canonical interface.
//!
//! During parsing, help and version are represented as [`Error::DisplayHelp`] and
//! [`Error::DisplayVersion`] terminal actions. Dynamic completion requests are represented as
//! [`Error::DisplayCompletion`]. The process-oriented parsing methods print those actions to stdout
//! and exit successfully. Other parse/binding errors go to stderr and exit with status 2.
//!
//! # Shell completions
//!
//! Argx generates dynamic completion adapters for Bash, Fish, Nushell, and Zsh through the
//! [`completion`] module.
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
//! [`Parser::parse`] and [`Parser::try_parse`] handle completion requests automatically.
//!
//! Fields marked `#[argx(value_enum)]` complete from the same finite vocabulary used for parsing
//! and help. Hidden aliases are accepted while reconstructing command scope but are not suggested.
//! Argx does not infer choices from arbitrary [`std::str::FromStr`] implementations or provide
//! filesystem or custom value completers.
//!
//! Applications typically expose generated adapters through a `completions <shell>` command. See
//! the `completions` example for a complete integration.
//!
//! # Schema discovery
//!
//! Mark commands that participate in schema discovery with `#[argx(schema)]`. Argx exposes
//! Draft 2020-12 JSON Schema through `-S` / `--schema` in the selected command scope and through
//! the root `schema [COMMAND]...` pseudo-command.
//!
//! Structural commands expose canonical child names as referenced object properties, allowing
//! tools to walk the command tree incrementally using ordinary JSON Schema relationships. Default
//! projections leave the immediate child boundary open; `--full` recursively bundles descendants
//! and validates the complete canonical invocation tree. Leaf commands expose their invocation
//! schema and, when associated with a handler, typed result and error schemas.
//!
//! Structural [`Args`] and `Subcommand` declarations use the same `#[argx(schema)]` marker.
//! Associate executable leaves with typed results and errors using `#[argx(handler = CommandType)]`
//! on a free function or `#[argx(handler = method)]` on an inherent impl.
//!
//! `#[argx(schema)]` on result and error data types delegates their JSON Schema generation to
//! Schemars and makes the type available through [`Schema`]. See the `schema` example for a
//! complete structural and leaf discovery flow.
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
//! kebab-case command name. `about` replaces documentation-derived descriptive text. A `Parser`
//! declaration may additionally use `version = expression`, `long_version = expression`, and the
//! marker `schema`. If only one version expression is supplied, Argx uses it for both `-V` and
//! `--version`. `schema` enables machine-readable discovery. Structural `Args` declarations that
//! contain a subcommand field may also use `schema` to participate in that command topology.
//! Version metadata remains root-only.
//!
//! Argx-owned schema keys use lower camel case consistently. Commands may also attach
//! application-defined machine-readable metadata with `metadata({ "key": value })`. Values may be
//! `null`, booleans, finite numbers, strings, arrays, or nested objects. Metadata keys are
//! preserved exactly as authored. Argx preserves metadata values without assigning semantics to
//! individual keys and exposes the metadata under `x-argx-metadata` in generated JSON Schema
//! documents. Standard JSON Schema keywords and application-owned schema fields retain their own
//! spellings.
//!
//! Aliases belong to selectable `Subcommand` variants. An `Args` declaration has no standalone
//! command name: flattening composes it into the current command, while a subcommand payload uses
//! the variant as the visible command.
//!
//! ## `Subcommand` variants
//!
//! The enum itself accepts the `schema` marker when it participates in machine-readable command
//! topology. Individual variants accept:
//!
//! - `name = "..."` to replace the inferred kebab-case command spelling.
//! - `about = "..."` to override documentation-derived descriptive text.
//! - `alias = "..."` for one hidden accepted command spelling.
//! - `aliases = ["...", "..."]` for multiple hidden accepted spellings.
//! - `version = expression` and `long_version = expression` for version actions local to that
//!   command scope.
//! - `metadata({ "key": value })` for application-defined machine-readable command metadata.
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
//! | `count` | bind the number of occurrences of a value-less flag to a `u8` field |
//! | `delimited` | split collection values on commas before conversion |
//! | `default = expression` | use a typed Rust default for a scalar value option or counted flag |
//! | `requires = "field"` | require another argument when this argument is supplied |
//! | `requires = ["a", "b"]` | require multiple arguments |
//! | `conflicts = "field"` | reject use with another argument |
//! | `conflicts = ["a", "b"]` | reject use with multiple arguments |
//! | `allow_hyphen_values` | allow arbitrary flag-like detached values for a named value option |
//! | `allow_negative_numbers` | accept negative-number values without accepting other flags |
//! | `value_enum` | use a finite [`trait@ValueEnum`] vocabulary for parsing, help, completion, and schema discovery |
//! | `help = "..."` | override the field's documentation-derived one-line help text |
//! | `flatten` | compose one direct [`Args`] field into the current command |
//! | `subcommand` | select one direct derived `Subcommand` enum |
//!
//! Long and alias spellings are written without leading dashes. `count` uses a `u8` field, and
//! `delimited` splits collection values on commas. `requires` and `conflicts` refer to Rust field
//! names, including fields contributed through `flatten`. Incompatible attribute combinations are
//! rejected during derivation.
//!
//! # Derive restrictions
//!
//! Argx rejects unsupported command shapes at compile time. `Parser` and `Args` use unit or
//! named-field structs, subcommand variants are unit variants or carry one direct `Args` payload,
//! and structural fields hold their derived types directly. Invalid layouts and incompatible
//! attributes produce compile-time diagnostics.
//!
//! # Platform support
//!
//! The supported native targets are Linux and macOS. Windows is supported through the Windows
//! Subsystem for Linux (WSL). Native Windows targets are not supported.
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
pub use error::Error;

/// Generates the JSON Schema for a type supported by Argx schema discovery.
///
/// Types marked with `#[argx(schema)]` implement this trait automatically. Types that implement
/// Schemars' schema contract directly are supported as well.
pub trait Schema {
    /// Generates a standalone JSON Schema document for this type.
    fn schema() -> serde_json::Value;
}

impl<T> Schema for T
where
    T: schemars::JsonSchema,
{
    fn schema() -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(T))
            .expect("Schemars-generated schemas must serialize")
    }
}

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
pub use config::{Argv, Defaults, Dotenv, Environment, Error as ConfigError};

/// Parses command-line arguments into a typed value.
///
/// Use [`Parser::parse`] and [`Parser::try_parse`] for the current process. These entry points
/// honor process-level Argx protocols such as dynamic completion. Use [`Parser::parse_from`] and
/// [`Parser::try_parse_from`] when parsing an explicit argv sequence; the `*_from` methods are
/// determined only by the supplied argv and do not inspect process-level completion state.
///
/// Within each pair, the `parse*` method applies Argx's normal rendering and exit policy while the
/// corresponding `try_parse*` method returns [`Error`] to the caller.
pub trait Parser: Sized + __private::CommandArgs {
    /// Parses the current process arguments, excluding the program name.
    ///
    /// Help, version, and schema requests are printed to standard output and terminate
    /// successfully. Parse failures are printed to standard error and terminate the process with
    /// status 2.
    fn parse() -> Self {
        Self::try_parse().unwrap_or_else(|error| error.exit())
    }

    /// Parses the current process arguments, excluding the program name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DisplayHelp`], [`Error::DisplayVersion`], [`Error::DisplaySchema`], or
    /// [`Error::DisplayCompletion`] when the corresponding built-in or process action is requested,
    /// or an error when argv cannot be bound to this command or a bound value cannot be converted
    /// to its Rust field type.
    fn try_parse() -> Result<Self, Error> {
        if let Some(completion) = completion::process_request::<Self>() {
            return Err(Error::DisplayCompletion { completion });
        }
        parse_args::<Self, _, _>(std::env::args_os().skip(1))
    }

    /// Parses a complete argv sequence whose first item is the program name.
    ///
    /// Parsing is determined only by `argv`; process-level completion state is not inspected.
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
    /// Parsing is determined only by `argv`; process-level completion state is not inspected. The
    /// program name is ignored. An empty sequence is therefore equivalent to a program name
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
        parse_args::<Self, _, _>(argv)
    }

    /// Generates a dynamic completion adapter for this parser's configured root command name.
    ///
    /// # Errors
    ///
    /// Returns [`completion::ScriptError::InvalidCommandName`] when the configured command name
    /// cannot be safely registered by every supported shell adapter.
    fn render_completion(shell: completion::Shell) -> Result<String, completion::ScriptError> {
        completion::script(Self::COMMAND.name, shell)
    }
}

/// Parses already-separated arguments for the public parser entry points.
fn parse_args<P, I, T>(argv: I) -> Result<P, Error>
where
    P: Parser,
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let owned: Vec<OsString> = argv.into_iter().map(Into::into).collect();
    let refs: Vec<&OsStr> = owned.iter().map(OsString::as_os_str).collect();
    if !<P as __private::CommandArgs>::SCHEMA_ENABLED {
        return cli::binding::parse_refs::<P>(&refs);
    }
    let registry = <P as __private::CommandArgs>::schema_registry()
        .expect("schema-enabled parser must generate a schema registry");
    if let Some(error) = schema::pseudo_command(P::COMMAND, &refs, &registry) {
        return Err(error);
    }
    cli::binding::parse_refs_with_schema::<P>(&refs, &registry)
}

/// Implementation details shared with generated code.
///
/// This module is public so proc-macro expansions can name these items from downstream crates. It
/// is not part of Argx's stable user-facing API.
#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
