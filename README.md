<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-light.svg">
  <img alt="Argx" src="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-light.svg" width="100%" height="140px">
</picture>

<p align="center">
  Expressive command line argument parser for Rust
</p>

<br/>

<p align="center">
  <a href="https://crates.io/crates/argx"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/crates/v/argx?colorA=21262d&colorB=21262d&style=flat"><img src="https://img.shields.io/crates/v/argx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="Version"></picture></a>
  <a href="#license"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/crates/l/argx?colorA=21262d&colorB=21262d&style=flat"><img src="https://img.shields.io/crates/l/argx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="MIT OR Apache-2.0"></picture></a>
</p>

Argx is a derive-first command-line argument parser for Rust. Rust structs and enums define the
command tree while generated static metadata drives argv parsing, typed binding, help and version
output, diagnostics, and machine-readable invocation contracts.

The model is deliberately small:

- [`Parser`](https://docs.rs/argx/latest/argx/trait.Parser.html) defines the root command;
- [`Args`](https://docs.rs/argx/latest/argx/trait.Args.html) defines reusable argument groups;
- `#[derive(Subcommand)]` defines exact, typed child-command selection;
- Rust field shapes define argument cardinality and conversion;
- Rust documentation and `#[argx(...)]` metadata define the human-facing CLI;
- [`Parser::contract`](https://docs.rs/argx/latest/argx/trait.Parser.html#method.contract) exposes the same invocation model to tools and agents.

## Installation

```sh
cargo add argx
```

The default `derive` feature exports the `Parser`, `Args`, and `Subcommand` derive macros and is the
normal way to define a CLI.

## Quick start

```rust
use std::path::PathBuf;

use argx::{Args, Parser, Subcommand};

/// Manage stored objects.
#[derive(Debug, Parser)]
#[argx(name = "acme", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Output
    #[argx(flatten)]
    output: OutputArgs,

    #[argx(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct OutputArgs {
    /// Emit machine-readable JSON.
    #[argx(long, global)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read one object.
    Get(GetArgs),

    /// Print service status.
    Status,
}

#[derive(Debug, Args)]
struct GetArgs {
    /// Object identifier.
    id: String,

    /// Write the object to a file.
    #[argx(long, short)]
    output: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    println!("{cli:#?}");
}
```

Argx derives `get` from `Get`, `--output` and `-o` from the field name, generated help from the Rust
documentation, and a built-in `-V` / `--version` action from the version metadata. Because `--json`
is global, it remains valid after a subcommand has been selected.

## Arguments and values

A field is positional unless it has `#[argx(long)]` or `#[argx(short)]`. Named arguments can expose
both spellings, plus hidden long aliases with `alias` or `aliases`.

| Rust field shape | CLI behavior |
| --- | --- |
| named `bool` | value-less switch |
| `T` | exactly one required value |
| `Option<T>` | zero or one value |
| `Vec<T>` | zero or more values |
| `Option<Vec<T>>` | optional zero-or-more collection |

`String` values require UTF-8. `OsString` and `PathBuf` preserve operating-system strings, including
non-UTF-8 argv on Unix. Other direct value types are parsed through `FromStr`.

Argx supports long values in both `--output file` and `--output=file` form, short-option bundles,
`--` to stop flag interpretation, repeated collection flags, and exact subcommand matching. Use
`allow_hyphen_values` when a named option must accept arbitrary flag-like values, or
`allow_negative_numbers` when only negative-number spellings should be accepted as values.

Type-shape inference is syntactic. Special handling for `bool`, `Option`, `Vec`, `String`,
`OsString`, and `PathBuf` requires those standard types to appear directly in the field type; a type
alias around one of them is treated as an ordinary `FromStr` value type.

## Reusable arguments and subcommands

`#[derive(Args)]` declarations can be composed into another command with `#[argx(flatten)]`:

```rust
#[derive(argx::Args)]
struct Common {
    /// Enable verbose diagnostics.
    #[argx(long, global)]
    verbose: bool,
}

#[derive(argx::Parser)]
struct Cli {
    /// Common options
    #[argx(flatten)]
    common: Common,
}
```

The flatten field's documentation becomes a help-group heading. Flattening is structural: the
nested arguments participate in the containing command's validation, help, and machine contract
rather than creating another command scope.

A `Subcommand` enum creates command scopes. Variants may be unit variants or carry one direct
`Args` payload:

```rust
#[derive(argx::Subcommand)]
enum Command {
    Status,
    Add(AddArgs),
}
```

Variant names default to kebab-case and can be changed with `name`. `alias` and `aliases` add hidden
accepted spellings without changing the canonical name shown in help or contracts. Command matching
is exact; Argx does not perform prefix matching.

Named options normally belong only to the command that declares them. `global` keeps an option in
scope in descendants. A child declaration may reuse a spelling from an ancestor; the nearest active
command scope wins.

## Value sources and constraints

Scalar value-taking options can fall back to an environment variable or a typed Rust default:

```rust
#[derive(argx::Parser)]
struct Cli {
    #[argx(long, env = "ACME_TOKEN")]
    token: String,

    #[argx(long, default = 3)]
    retries: u8,
}
```

Argv has precedence over the configured environment fallback. A typed default satisfies absence
without turning the default expression into command-line text.

Arguments can also declare relationships by Rust field name:

```rust
#[derive(argx::Parser)]
struct Cli {
    #[argx(long, requires = "token")]
    remote: bool,

    #[argx(long)]
    token: Option<String>,

    #[argx(long, conflicts = "remote")]
    offline: bool,
}
```

`requires` means that supplying the source requires the target to resolve a value. `conflicts`
rejects supplying both arguments. References are validated against the composed command model.

## Help and version output

Every command scope has built-in `-h` and `--help`. A root command or subcommand with `version` or
`long_version` metadata also receives `-V` and `--version`.

Rust documentation is part of the CLI definition:

- the first prose paragraph becomes the one-line command or argument summary;
- command prose is rendered before generated sections;
- level-one Markdown headings on a command become additional help sections;
- documentation on a flattened field becomes a named argument group;
- `about` and `help` provide explicit overrides when Rust documentation should differ from CLI text.

Aliases are intentionally hidden from generated help so one canonical interface is presented even
when compatibility spellings are accepted.

`Parser::parse` handles built-in help/version actions and parse failures as a normal CLI entrypoint:
help and version go to stdout with status 0, while failures go to stderr with status 2. The
`try_parse*` methods return the corresponding [`Error`](https://docs.rs/argx/latest/argx/enum.Error.html)
instead, which is useful for tests, embedding, and custom process policy.

## Machine-readable invocation contracts

Argx can discover a versioned description of the CLI without maintaining a second reflection or
schema registry:

```rust
use argx::{ContractRequest, Parser as _};

let contract = Cli::contract(ContractRequest::new(["get"]).recursive())
    .expect("get command must exist");
println!(
    "{}",
    contract.to_json_pretty().expect("contract must serialize")
);
```

A contract describes canonical command paths, accepted aliases, whether a command is directly
invocable, positional and named arguments, value cardinality, global scope, environment/default
sources, and `requires` / `conflicts` relationships. Shallow discovery returns the selected command
in full plus direct child summaries; recursive discovery expands the complete descendant subtree.
Lookup accepts command aliases, while returned paths remain canonical.

The wire format is explicitly versioned by
[`CONTRACT_VERSION`](https://docs.rs/argx/latest/argx/constant.CONTRACT_VERSION.html). The contract is
an **invocation contract**: it describes how to call the CLI, not a serialization schema for the
concrete Rust value types stored in its fields.

## Examples

The repository examples are deliberately small and each isolates one public behavior. Every file
starts with runnable commands and notes about the invariant it demonstrates, so the examples also
serve as focused reference documentation.

| Example | Focus | Try it |
| --- | --- | --- |
| [`basic`](crates/argx/examples/basic.rs) | Smallest complete parser and built-in help | `cargo run --example basic -- --help` |
| [`subcommands`](crates/argx/examples/subcommands.rs) | Typed command selection and reusable payloads | `cargo run --example subcommands -- add hello --force` |
| [`flatten`](crates/argx/examples/flatten.rs) | Reusable argument groups and help grouping | `cargo run --example flatten -- --help` |
| [`environment`](crates/argx/examples/environment.rs) | `argv` → environment → default precedence | `ARGX_PORT=8080 cargo run --example environment --` |
| [`defaults`](crates/argx/examples/defaults.rs) | Typed Rust defaults without text round-tripping | `cargo run --example defaults --` |
| [`constraints`](crates/argx/examples/constraints.rs) | `requires` and `conflicts` relationships | `cargo run --example constraints -- --endpoint https://example.invalid --token secret --workspace demo` |
| [`aliases`](crates/argx/examples/aliases.rs) | Hidden compatibility spellings and canonical help | `cargo run --example aliases -- --colour always rm` |
| [`structured_help`](crates/argx/examples/structured_help.rs) | Documentation-derived descriptions, groups, and sections | `cargo run --example structured_help -- --help` |
| [`version`](crates/argx/examples/version.rs) | Lexically scoped short and long version actions | `cargo run --example version -- run --version` |

`structured_help` is the best example to inspect when designing the user-facing CLI, while
`subcommands`, `flatten`, and `constraints` show the main composition rules. For the precise parsing
model, derive restrictions, failure precedence, and machine-contract semantics, see the
[crate documentation](https://docs.rs/argx).

## Support

Argx supports Linux and macOS natively. Windows is supported through the
Windows Subsystem for Linux (WSL); native Windows targets are not supported.

## MSRV

<!--
When updating this, also update:
- Cargo.toml
- .github/workflows/ci.yml
-->

The current MSRV (minimum supported Rust version) is 1.95.

Argx will keep a rolling MSRV policy of **at least** two versions behind the
latest stable release (so if the latest stable release is 1.97, we would
support 1.95).

Note that the MSRV is not increased automatically.

## Contributing

Contributions to Argx are welcome. See the [Contributing Guide](CONTRIBUTING.md) for information on reporting bugs, proposing features, submitting pull requests, and the licensing terms that apply to contributions.

## Security Policy

If you believe you have found a security vulnerability, please do not report it through GitHub Issues. See our [Security Policy](SECURITY.md) for reporting instructions.

## Credit

Argx is inspired in part by [Usage](https://github.com/jdx/usage), [Clap](https://github.com/clap-rs/clap) and [Incur](https://github.com/wevm/incur).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

This software includes third-party components subject to separate license
terms. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Argx by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
