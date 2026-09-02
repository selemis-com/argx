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

<p align="center">
  <a href="#installation">Installation</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#configuration">Configuration</a> ·
  <a href="#schema-discovery">Schema discovery</a> ·
  <a href="#examples">Examples</a> ·
  <a href="#support">Support</a> ·
  <a href="#contributing">Contributing</a>
</p>

## Overview

Argx is a derive-first command-line parser and configuration library for Rust. Define your CLI and configuration with Rust types, and Argx derives parsing, help, diagnostics, completions, schema discovery, and layered configuration from those definitions.

See [docs.rs/argx](https://docs.rs/argx/latest/argx/) for the complete API and behavioral reference.

## Installation

```sh
cargo add argx
```

### Features

The `derive` feature is enabled by default. Enable `toml` when using TOML configuration layers:

```sh
cargo add argx --features toml
```

Enable `chrono`, `url`, or `uuid` when command values and schema-enabled types use those crates.
Argx preserves recognized formats in invocation schemas and enables the matching Schemars
integrations. Chrono `DateTime` and `NaiveDate` values receive standard `date-time` and `date`
formats. `NaiveTime` and `NaiveDateTime` remain lexical strings because JSON Schema has no standard
format that faithfully represents their timezone-free values:

```sh
cargo add argx --features chrono,url,uuid
```

## Quick start

```rust
use argx::{Args, Parser, Subcommand};

#[derive(Parser)]
#[argx(name = "acme")]
struct Cli {
    #[argx(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the service.
    Serve(Serve),

    /// Print service status.
    Status,
}

#[derive(Args)]
struct Serve {
    /// Port to listen on.
    #[argx(long, default = 8080)]
    port: u16,
}

fn main() {
    match Cli::parse().command {
        Command::Serve(args) => println!("listening on {}", args.port),
        Command::Status => println!("running"),
    }
}
```

Rust documentation becomes CLI help, while field types define parsing.

```text
$ acme serve --port 3000
listening on 3000

$ acme --help
Usage: acme [OPTIONS] <COMMAND>

Commands:
  serve   Start the service.
  status  Print service status.

Options:
  -h, --help  Print help
```

Nested commands get their own generated help:

```text
$ acme serve --help
Start the service.

Usage: acme serve [OPTIONS]

Options:
      --port <PORT>  Port to listen on.
  -h, --help         Print help
```

The same derived command model also powers shell completion and schema discovery.

## Configuration

`#[derive(argx::Config)]` resolves one typed value from explicitly ordered layers:

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

Layers are applied in declaration order, with later layers overriding only values they provide.
Defaults are explicit through `Defaults`. `Environment` reads mapped environment variables and
`Argv` reads fields with CLI metadata. `Dotenv` and optional `Toml` layers read only the paths you
provide. Argx performs no configuration-file discovery.

See the [configuration example](crates/argx/examples/configuration.rs) for environment naming, flattening, interpolation, and collection values.

## Schema discovery

Mark each command that participates in schema discovery with `#[argx(schema)]`.

Argx-owned schema keys use lower camel case consistently. Commands can also expose application-defined semantic metadata without teaching Argx what the keys mean. Metadata keys are preserved exactly as authored. For example, `#[argx(metadata({ "readOnly": true, "requiredScopes": ["objects:read"] }))]` produces:

```json
"x-argx-metadata": {
  "readOnly": true,
  "requiredScopes": ["objects:read"]
}
```

Metadata values may be `null`, booleans, finite numbers, strings, arrays, or nested objects. This keeps effects, scopes, safety hints, and other application semantics available to machine consumers while leaving their interpretation to the application.

At the root, Argx exposes immediate subcommands as ordinary referenced properties:

```text
acme schema
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "acme",
  "type": "object",
  "properties": {
    "objects": {
      "$ref": "#/$defs/commands/$defs/objects"
    }
  },
  "required": ["objects"],
  "additionalProperties": false,
  "$defs": {
    "commands": {
      "$defs": {
        "objects": {
          "title": "objects",
          "description": "Manage objects.",
          "type": "object"
        }
      }
    }
  }
}
```

Default structural schemas intentionally stop at the immediate command boundary. The referenced
child is an open object, so the document validates the selected command name without claiming to
validate descendants that were not projected. Request that command's schema to continue discovery.

Leaf commands expose the concrete invocation contract together with typed result and error schemas:

```text
acme objects get object-7 --schema
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "get",
  "description": "Get an object.",
  "type": "object",
  "properties": {
    "id": {
      "description": "Object identifier.",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "additionalProperties": false,
  "$defs": {
    "result": {
      "$ref": "#/$defs/types/$defs/GetOutput",
      "title": "GetOutput"
    },
    "error": {
      "$ref": "#/$defs/types/$defs/GetError",
      "title": "GetError"
    },
    "types": {
      "$defs": {
        "GetOutput": {
          "type": "object",
          "properties": {
            "id": {
              "type": "string"
            }
          },
          "required": [
            "id"
          ]
        },
        "GetError": {
          "type": "string",
          "enum": [
            "NotFound"
          ]
        }
      }
    }
  }
}
```

`--full` recursively bundles child command schemas and closes every projected command object. The
result validates the complete canonical invocation tree: canonical subcommand names are nested
object properties, while each option or positional value is represented at the command scope that
owns it. Parsed primitive values use their semantic JSON types, so Rust integers, floats, and
booleans project as JSON Schema `integer`, `number`, and `boolean` values rather than argv strings.
Inherited globals are hoisted only into the selected schema root.

See the [schema example](crates/argx/examples/schema.rs) for a complete command tree.

## Examples

Runnable examples cover the main Argx features. Start with `basic` for the smallest integration point or `complete` for an integrated example.

| Example | Focus | Try it |
| --- | --- | --- |
| [`basic`](crates/argx/examples/basic.rs) | Minimal parser and built-in help | `cargo run --example basic -- --help` |
| [`arguments`](crates/argx/examples/arguments.rs) | Options, defaults, constraints, and finite values | `cargo run --example arguments -- input.txt --format json` |
| [`commands`](crates/argx/examples/commands.rs) | Subcommands, flattening, aliases, and versions | `cargo run --example commands -- --verbose add hello --force` |
| [`configuration`](crates/argx/examples/configuration.rs) | Ordered configuration layers | `cargo run --example configuration -- --workers 8` |
| [`schema`](crates/argx/examples/schema.rs) | Schema discovery and handler contracts | `cargo run --example schema -- schema objects get` |
| [`completions`](crates/argx/examples/completions.rs) | Dynamic shell completion | `cargo run --example completions -- zsh` |
| [`complete`](crates/argx/examples/complete.rs) | Integrated reference application | `cargo run --example complete -- get object-7` |

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
