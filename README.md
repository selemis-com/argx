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
command tree while generated static metadata drives parsing, typed binding, help, diagnostics, and
machine-readable contracts.

```sh
cargo add argx
```

Argx intentionally has a small, focused feature set. It does not aim for feature parity or API
compatibility with Clap or Usage, and diverges from their APIs where a different design better fits
Argx. Features are added selectively as they are needed.

Full API documentation and behavioral details are available on
[docs.rs/argx](https://docs.rs/argx/latest/argx/).

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

[`Parser`](https://docs.rs/argx/latest/argx/trait.Parser.html) defines a root command,
[`Args`](https://docs.rs/argx/latest/argx/trait.Args.html) defines reusable argument groups, and
[`Subcommand`](https://docs.rs/argx/latest/argx/derive.Subcommand.html) defines exact typed
child-command selection. Rust field shapes determine cardinality and conversion; Rust documentation
and `#[argx(...)]` metadata define the human-facing CLI.

Argx supports positional and named arguments, short bundles, nested subcommands, flattened argument
groups, lexical globals, aliases, typed defaults, environment fallbacks, argument relationships,
structured help, and version actions. See the [crate documentation](https://docs.rs/argx/latest/argx/)
for the complete grammar, precedence rules, derive restrictions, and error behavior.

## Machine-readable contracts

Argx can expose the same command model to tools and agents without maintaining a second schema
registry. [`#[derive(argx::Contract)]`](https://docs.rs/argx/latest/argx/derive.Contract.html)
describes semantic Rust value types, while
[`#[argx::contract(CommandType)]`](https://docs.rs/argx/latest/argx/attr.contract.html) binds an
invocable command to the success and error types returned by its handler.

```rust
use argx::{ContractRequest, Parser as _};

#[derive(argx::Args)]
struct GetArgs {
    id: String,
}

#[derive(argx::Subcommand)]
enum Command {
    Get(GetArgs),
}

#[derive(argx::Parser)]
struct Cli {
    #[argx(subcommand)]
    command: Command,
}

#[derive(argx::Contract)]
struct GetOutput {
    id: String,
}

#[derive(argx::Contract)]
enum GetError {
    NotFound,
}

#[argx::contract(GetArgs)]
fn get(args: GetArgs) -> Result<GetOutput, GetError> {
    Ok(GetOutput { id: args.id })
}

let contract = Cli::contract(ContractRequest::new(["get"]))
    .expect("get command must exist");
assert!(contract.command.execution.is_some());
```

Contracts describe invocation semantics and semantic Rust input/output types. They are not JSON
Schema, do not interpret `serde` attributes, and do not describe the lexical grammar of custom
`FromStr` implementations. Standalone type contracts and combined CLI contracts share one wire
version, and named types use document-local references so repeated and recursive shapes have one
consistent representation.

See [`Parser::contract`](https://docs.rs/argx/latest/argx/trait.Parser.html#method.contract), the
[`contract` module](https://docs.rs/argx/latest/argx/contract/), and the
[`type_contract` module](https://docs.rs/argx/latest/argx/type_contract/) for discovery behavior,
execution requirements, wire fields, and type-contract semantics.

## Examples

The examples are executable documentation. Detailed behavior is documented on
[docs.rs/argx](https://docs.rs/argx/latest/argx/).

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

[Usage](https://github.com/jdx/usage) was a particularly important influence on Argx’s compile-time architecture: static command metadata, separation of argv parsing from typed construction, compile-time composition of commands and argument groups, and the use of one authoritative CLI description to drive parsing and other derived behavior.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

This software includes third-party components subject to separate license
terms. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Argx by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
