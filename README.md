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

Argx is a derive-first command-line parser and unified configuration library for Rust. Rust data
types define command and configuration surfaces while generated static metadata drives parsing,
typed binding, help, diagnostics, and ordered configuration resolution.

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
groups, lexical globals, aliases, typed defaults, finite value enums,
argument relationships, structured help, and version actions. See the
[crate documentation](https://docs.rs/argx/latest/argx/)
for the complete grammar, precedence rules, derive restrictions, and error behavior.

## Unified configuration

`#[derive(argx::Config)]` resolves one typed configuration from an explicitly ordered stack of
layers. Defaults, dotenv files, process environment, and argv all supply sparse values for the
same Rust fields. Enable the optional `toml` feature to add TOML file layers.

Enable TOML support with `cargo add argx --features toml`, then compose it like any other layer:

```rust
use argx::{Argv, Defaults, Environment, Toml};

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
    .layer(Toml::new("acme.toml"))
    .layer(Environment)
    .layer(Argv::current())
    .resolve()?;
# Ok::<(), argx::ConfigError>(())
```

Layers are applied in declaration order. Later layers replace only fields they actually supply, so
precedence is defined by composition rather than a policy built into Argx. Declared defaults are
not implicit: they participate only when `Defaults` is added to the loader. A non-optional Rust
field becomes required only after every configured layer has been considered.

A configuration-level prefix maps fields to environment variables. For example,
`#[argx(prefix = "ACME")]` maps `workers` to `ACME_WORKERS`; a flattened `server.workers` field maps
to `ACME_SERVER_WORKERS`. `#[argx(env = "EXACT_NAME")]` selects an exact variable instead. TOML
interpolation observes environment values established by earlier `Dotenv` or `Environment` layers,
so layer order also controls interpolation visibility.

Configuration fields participate in argv only when they carry CLI metadata such as `long` or
`short`; `#[argx(flatten)]` composes a nested `Config` across TOML, environment naming, defaults,
and argv. See the [configuration example](crates/argx/examples/configuration.rs) and the
[crate documentation](https://docs.rs/argx/latest/argx/) for the complete contract.

## Handler schemas

Argx exposes `#[argx(schema)]` as a thin Schemars-backed attribute. It derives Schemars'
`JsonSchema` through Argx, so downstream users do not need to depend on Schemars directly.
Import the standalone attribute once with `use argx::argx;`; derive helper attributes keep the same
`#[argx(...)]` spelling. Schema-enabled command trees keep structural traversal separate from
executable handlers:

```rust
use argx::{Parser as _, argx};

#[derive(argx::Parser)]
#[argx(name = "acme", schema)]
struct Cli {
    #[argx(subcommand)]
    command: Commands,
}

#[derive(argx::Subcommand)]
#[argx(schema)]
enum Commands {
    Get(GetCommand),
}

#[derive(argx::Args)]
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

`#[argx(schema)]` delegates Rust data-model schema generation to Schemars. Argx owns invocation
schema projection and static command topology. Structural `Args` and `Subcommand` declarations use
the same `#[argx(schema)]` marker; executable leaves are associated with their result and error
schemas by `#[argx(handler = ...)]`.
For a zero-argument executable command, use an empty `Args` struct rather than a unit subcommand
variant so the leaf has a concrete Rust type.

A parser with `#[argx(schema)]` exposes both discovery forms:

```text
acme schema get
acme get object-7 -S
# or: acme get object-7 --schema
```

Both emit the same Draft 2020-12 JSON Schema. The selected command's invocation is the root schema;
handler result and error schemas are bundled under `$defs.result` and `$defs.error`, with any
Schemars-generated type definitions under `$defs.types.$defs`. Structural paths such as `acme schema`
bundle their subcommand schemas under `$defs.subcommands.$defs`. Handler associations are traversed
statically from the command types into a short-lived local registry; Argx does not use linker
inventory or global registration.

The handler may also stay on the inherent implementation that owns execution:

```rust
#[argx(handler = run)]
impl GetCommand {
    fn run(self) -> Result<GetOutput, GetError> {
        Ok(GetOutput { id: self.id })
    }
}
```

## Examples

The examples are executable documentation. Detailed behavior is documented on
[docs.rs/argx](https://docs.rs/argx/latest/argx/).

| Example | Focus | Try it |
| --- | --- | --- |
| [`basic`](crates/argx/examples/basic.rs) | Smallest complete parser and built-in help | `cargo run --example basic -- --help` |
| [`configuration`](crates/argx/examples/configuration.rs) | Ordered defaults, TOML, environment, and argv configuration | `cargo run --example configuration -- --workers 8` |
| [`subcommands`](crates/argx/examples/subcommands.rs) | Typed command selection and reusable payloads | `cargo run --example subcommands -- add hello --force` |
| [`flatten`](crates/argx/examples/flatten.rs) | Reusable argument groups and help grouping | `cargo run --example flatten -- --help` |
| [`defaults`](crates/argx/examples/defaults.rs) | Typed Rust defaults without text round-tripping | `cargo run --example defaults --` |
| [`constraints`](crates/argx/examples/constraints.rs) | `requires` and `conflicts` relationships | `cargo run --example constraints -- --endpoint https://example.invalid --token secret --workspace demo` |
| [`aliases`](crates/argx/examples/aliases.rs) | Hidden compatibility spellings and canonical help | `cargo run --example aliases -- --colour always rm` |
| [`value_enum`](crates/argx/examples/value_enum.rs) | Finite typed values shared by parsing and generated help | `cargo run --example value_enum -- --help` |
| [`structured_help`](crates/argx/examples/structured_help.rs) | Documentation-derived descriptions, groups, and sections | `cargo run --example structured_help -- --help` |
| [`version`](crates/argx/examples/version.rs) | Lexically scoped short and long version actions | `cargo run --example version -- run --version` |
| [`completions`](crates/argx/examples/completions.rs) | Dynamic shell-completion adapters | `cargo run --example completions -- zsh` |

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
