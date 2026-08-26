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

Argx is a derive-first command-line argument parser for Rust. Rust structs and enums define one
static command model that drives argv parsing, typed binding, generated help and version output,
diagnostics, and machine-readable invocation contracts.

The public model is intentionally small: [`Parser`](https://docs.rs/argx/latest/argx/trait.Parser.html)
defines a root command, [`Args`](https://docs.rs/argx/latest/argx/trait.Args.html) defines reusable
arguments, and `#[derive(Subcommand)]` defines typed child-command selection.

## Installation

```sh
cargo add argx
```

The default `derive` feature exports the `Parser`, `Args`, and `Subcommand` derive macros.

## Quick start

```rust
use argx::{Args, Parser, Subcommand};

#[derive(Parser)]
#[argx(name = "acme", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Enable verbose diagnostics.
    #[argx(long, global)]
    verbose: bool,

    #[argx(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read one object.
    Get(GetArgs),

    /// Print service status.
    Status,
}

#[derive(Args)]
struct GetArgs {
    /// Object identifier.
    id: String,
}

fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!("verbose mode enabled");
    }
    match cli.command {
        Command::Get(args) => println!("{}", args.id),
        Command::Status => println!("ok"),
    }
}
```

```text
$ acme get object-7
object-7

$ acme --help
Usage: acme [OPTIONS] <COMMAND>
...
```

Argx supports positional and named arguments, short bundles, exact nested subcommands, reusable
flattened argument groups, lexical globals, hidden aliases, typed defaults, environment fallbacks,
argument relationships, documentation-derived structured help, version actions, and versioned
machine-readable invocation contracts.

The complete behavioral model and `#[argx(...)]` attribute reference live in the
[crate documentation](https://docs.rs/argx). Unsupported derive shapes are rejected explicitly
rather than approximated at runtime.

## Examples

The examples are executable documentation. Each focuses on one public behavior and includes
runnable invocations in its module documentation.

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
