<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-light.svg">
  <img alt="Argx" src="https://raw.githubusercontent.com/selemis-com/argx/master/.github/assets/wordmark-light.svg" width="100%" height="140px">
</picture>

<p align="center">
  Expressive command-line parsing and configuration for Rust
</p>

<br/>

<p align="center">
  <a href="https://crates.io/crates/argx"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/crates/v/argx?colorA=21262d&colorB=21262d&style=flat"><img src="https://img.shields.io/crates/v/argx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="Version"></picture></a>
  <a href="#license"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/crates/l/argx?colorA=21262d&colorB=21262d&style=flat"><img src="https://img.shields.io/crates/l/argx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="MIT OR Apache-2.0"></picture></a>
</p>

Argx is a derive-first command-line parser and configuration library for Rust. Rust types define
its interface. Generated static metadata drives parsing, typed binding, help, diagnostics,
completions, schemas, and ordered configuration resolution.

The model is deliberately small:

- [`Parser`](https://docs.rs/argx/latest/argx/trait.Parser.html) defines a root command.
- [`Args`](https://docs.rs/argx/latest/argx/trait.Args.html) defines reusable argument groups.
- [`Subcommand`](https://docs.rs/argx/latest/argx/derive.Subcommand.html) defines typed child commands.
- [`Config`](https://docs.rs/argx/latest/argx/trait.Config.html) resolves typed values across explicitly ordered layers.
- one static command model drives parsing, help, completions, and schema discovery.

Full API documentation and behavioral details are available on
[docs.rs/argx](https://docs.rs/argx/latest/argx/).

## Installation

```sh
cargo add argx
```

### Features

- `derive` is enabled by default. It exports the `Parser`, `Args`, `Subcommand`, `ValueEnum`, and
  `Config` derives plus the `#[argx(...)]` attribute macro.
- `toml` enables TOML configuration layers and implies `derive`.

Enable TOML support with:

```sh
cargo add argx --features toml
```

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

Rust field shapes determine cardinality and conversion, while Rust documentation and
`#[argx(...)]` metadata define the user-facing CLI. The same derived model is reused for help,
completions, and schema discovery rather than maintaining parallel descriptions of the command
surface.

## Configuration

`#[derive(argx::Config)]` resolves one typed configuration from an explicitly ordered stack of
layers. Defaults, dotenv files, process environment, and argv all supply sparse values for the same
Rust fields.

```rust
use argx::{Argv, Defaults, Environment};

#[derive(argx::Config)]
#[argx(prefix = "ACME")]
struct Config {
    #[argx(long, default = 4)]
    workers: usize,

    #[argx(long)]
    endpoint: String,
}

let config = Config::loader()
    .layer(Defaults)
    .layer(Environment)
    .layer(Argv::current())
    .resolve()?;
```

Layers are applied in declaration order. Later layers replace only fields they actually supply, so
precedence comes from composition rather than a policy built into Argx. Declared defaults are not
implicit: they participate only when `Defaults` is added. A non-optional field becomes required
only after every configured layer has been considered.

A configuration-level prefix maps fields to environment variables. For example,
`#[argx(prefix = "ACME")]` maps `workers` to `ACME_WORKERS`. A flattened `server.workers` field maps
to `ACME_SERVER_WORKERS`. `#[argx(env = "EXACT_NAME")]` selects an exact variable instead.
Environment layers inspect only mapped variables. Unrelated process variables are ignored.

Files are explicit layers too:

```rust
let config = Config::loader()
    .layer(Dotenv::new(".env"))
    .resolve()?;
```

With the `toml` feature enabled, `Toml::new("acme.toml")` adds a TOML layer. TOML interpolation can
observe environment values established by earlier `Dotenv` or `Environment` layers, so layer order
also controls interpolation visibility. Argx performs no file discovery.

Configuration fields participate in argv only when they carry CLI metadata such as `long` or
`short`. `#[argx(flatten)]` composes a nested `Config` across every layer. See the
[configuration example](crates/argx/examples/configuration.rs) for a runnable version.

## Schema discovery

A parser marked `#[argx(schema)]` exposes its command interface as Draft 2020-12 JSON Schema for
tooling and agents:

```text
acme schema get
acme get object-7 --schema
```

Both forms describe the selected command. `#[argx(handler = CommandType)]` can associate an
executable leaf with typed result and error schemas, while `#[argx(schema)]` derives the underlying
Rust data-model schemas through Schemars without requiring downstream users to depend on Schemars
directly.

```rust
use argx::{Args, argx};

#[derive(Args)]
struct GetCommand {
    id: String,
}

#[argx(schema)]
struct GetOutput {
    id: String,
}

#[argx(schema)]
enum GetError {
    NotFound,
}

#[argx(handler = GetCommand)]
fn get(command: GetCommand) -> Result<GetOutput, GetError> {
    Ok(GetOutput { id: command.id })
}
```

See the [schema example](crates/argx/examples/schema.rs) and
[crate documentation](https://docs.rs/argx/latest/argx/) for structural schema composition and the
exact discovery contract.

## Examples

The examples are executable documentation. Start with `basic` for the smallest integration point or
`complete` for the complete Argx API. The remaining examples isolate one major subsystem.
Detailed behavior is documented on [docs.rs/argx](https://docs.rs/argx/latest/argx/).

| Example | Focus | Try it |
| --- | --- | --- |
| [`basic`](crates/argx/examples/basic.rs) | Smallest complete parser and built-in help | `cargo run --example basic -- --help` |
| [`complete`](crates/argx/examples/complete.rs) | Integrated reference application showing the complete Argx API | `cargo run --example complete -- get object-7 --format json` |
| [`arguments`](crates/argx/examples/arguments.rs) | Arguments, defaults, aliases, constraints, and value enums | `cargo run --example arguments -- input.txt --format json` |
| [`commands`](crates/argx/examples/commands.rs) | Subcommands, flattening, structured help, aliases, and versions | `cargo run --example commands -- --verbose add hello --force` |
| [`configuration`](crates/argx/examples/configuration.rs) | Ordered defaults, environment, and argv configuration | `cargo run --example configuration -- --workers 8` |
| [`schema`](crates/argx/examples/schema.rs) | Schema discovery and typed handler result/error contracts | `cargo run --example schema -- schema get` |
| [`completions`](crates/argx/examples/completions.rs) | Dynamic shell-completion adapters | `cargo run --example completions -- zsh` |

## Support

Argx supports Linux and macOS natively. Windows is supported through the
Windows Subsystem for Linux (WSL). Native Windows targets are not supported.

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
