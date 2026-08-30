//! Integrated reference application covering Argx's major public surfaces.
//!
//! This is intentionally broader than the focused examples. It combines typed configuration,
//! reusable/global arguments, defaults, aliases, constraints, value enums, subcommands, structured
//! help, version actions, schema discovery, handler contracts, and dynamic completion generation in
//! one command tree.
//!
//! ```text
//! cargo run --example complete -- get object-7
//! cargo run --example complete -- put object-7 value --endpoint https://example.invalid --token secret
//! cargo run --example complete -- completions zsh
//! cargo run --example complete -- schema get
//! cargo run --example complete -- --version
//! ```
//!
//! Configuration uses defaults and the process environment by default. Set
//! `ARGX_COMPLETE_DOTENV` to add a dotenv file. With the `toml` feature enabled,
//! `ARGX_COMPLETE_TOML` adds a TOML layer as well.

use std::{
    env,
    io::{self, Write},
};

use argx::{
    Args, Defaults, Dotenv, Environment, Parser as _, Subcommand, argx, completion::Shell,
};
use serde::Serialize;

/// Short version displayed by `--version`.
const VERSION: &str = "1.2.3";
/// Extended version displayed by `--version --verbose`.
const LONG_VERSION: &str = "1.2.3 (build abc123)";

/// Process-level settings resolved independently from command selection.
#[derive(Debug, argx::Config)]
#[argx(prefix = "ARGX_COMPLETE")]
struct Settings {
    /// Number of worker tasks.
    #[argx(default = 4)]
    workers: usize,

    /// Default service endpoint.
    #[argx(default = String::from("http://localhost:8080"))]
    endpoint: String,

}

/// Options inherited by every selected command.
#[derive(Debug, Args)]
struct Common {
    /// Enables verbose diagnostics.
    #[argx(short, long, global)]
    verbose: bool,
}

/// Retrieve one object.
#[derive(Debug, Args)]
struct Get {
    /// Object identifier.
    id: String,

    /// Maximum number of related records to return.
    #[argx(long, default = 20)]
    limit: usize,
}

/// Successful result returned by `Get`.
#[derive(Serialize)]
#[argx(schema)]
struct GetOutput {
    /// Returned object identifier.
    id: String,
    /// Applied related-record limit.
    limit: usize,
}

/// Errors returned by `Get`.
#[argx(schema)]
enum GetError {
    /// The requested object does not exist.
    NotFound,
}

/// Executes a `Get` command.
#[argx(handler = Get)]
fn get(command: Get) -> Result<GetOutput, GetError> {
    if command.id == "missing" {
        Err(GetError::NotFound)
    } else {
        Ok(GetOutput { id: command.id, limit: command.limit })
    }
}

/// Store one object.
#[derive(Debug, Args)]
struct Put {
    /// Object identifier.
    id: String,

    /// Object value.
    value: String,

    /// Remote endpoint used for this request.
    #[argx(long, requires = ["token"])]
    endpoint: Option<String>,

    /// Authentication token required by `--endpoint`.
    #[argx(long)]
    token: Option<String>,

    /// Write the request without applying it.
    #[argx(long, conflicts = ["force"])]
    dry_run: bool,

    /// Force replacement of an existing object.
    #[argx(long)]
    force: bool,
}

/// Successful result returned by `Put`.
#[derive(Serialize)]
#[argx(schema)]
struct PutOutput {
    /// Stored object identifier.
    id: String,
}

/// Errors returned by `Put`.
#[argx(schema)]
enum PutError {
    /// The write was rejected.
    Rejected,
}

/// Executes a `Put` command.
#[argx(handler = Put)]
fn put(command: Put) -> Result<PutOutput, PutError> {
    if command.value == "reject" {
        Err(PutError::Rejected)
    } else {
        let _ = (&command.endpoint, &command.token, command.dry_run, command.force);
        Ok(PutOutput { id: command.id })
    }
}

/// Completion adapter request.
#[derive(Debug, Clone, Copy, Args)]
struct Completions {
    /// Shell whose adapter should be generated.
    #[argx(value_enum)]
    shell: Shell,
}

/// Generated completion adapter.
#[derive(Serialize)]
#[argx(schema)]
struct CompletionOutput {
    /// Shell script implementing the dynamic completion adapter.
    script: String,
}

/// Errors returned while generating completions.
#[argx(schema)]
enum CompletionError {
    /// Argx could not render the requested completion adapter.
    Render,
}

/// Generates a completion adapter for the selected shell.
#[argx(handler = Completions)]
fn completions(command: Completions) -> Result<CompletionOutput, CompletionError> {
    Cli::render_completion(command.shell)
        .map(|script| CompletionOutput { script })
        .map_err(|_| CompletionError::Render)
}

/// Commands accepted by the application.
#[derive(Debug, Subcommand)]
#[argx(schema)]
enum Command {
    /// Retrieve one object.
    #[argx(alias = "show")]
    Get(Get),

    /// Store one object.
    Put(Put),

    /// Generate a dynamic shell-completion adapter.
    Completions(Completions),
}

/// Complete Argx reference application.
///
/// Rust documentation supplies the command summary and longer description, while the command
/// model drives parsing, help, completion, and machine-readable schema discovery.
///
/// # Examples
///
///     complete get object-7
///     complete show object-7
///     complete completions bash
#[derive(Debug, argx::Parser)]
#[argx(name = "complete", version = VERSION, long_version = LONG_VERSION, schema)]
struct Cli {
    /// Common options
    #[argx(flatten)]
    common: Common,

    /// Selects one operation.
    #[argx(subcommand)]
    command: Command,
}

/// Resolves process-level settings from the configured layers.
fn settings() -> Result<Settings, argx::ConfigError> {
    let loader = Settings::loader().layer(Defaults);

    #[cfg(feature = "toml")]
    let loader = match env::var_os("ARGX_COMPLETE_TOML") {
        Some(path) => loader.layer(argx::Toml::new(std::path::PathBuf::from(path))),
        None => loader,
    };

    let loader = match env::var_os("ARGX_COMPLETE_DOTENV") {
        Some(path) => loader.layer(Dotenv::new(std::path::PathBuf::from(path))),
        None => loader,
    };

    loader.layer(Environment).resolve()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if Cli::handle_completion() {
        return Ok(());
    }

    let settings = settings()?;
    let cli = Cli::parse();

    if cli.common.verbose {
        eprintln!("workers: {}", settings.workers);
        eprintln!("default endpoint: {}", settings.endpoint);
    }

    match cli.command {
        Command::Get(command) => match get(command) {
            Ok(value) => println!("get: {} (limit {})", value.id, value.limit),
            Err(GetError::NotFound) => {
                eprintln!("object not found");
                std::process::exit(1);
            }
        },
        Command::Put(command) => match put(command) {
            Ok(value) => println!("put: {}", value.id),
            Err(PutError::Rejected) => {
                eprintln!("request rejected");
                std::process::exit(1);
            }
        },
        Command::Completions(command) => {
            let value = match completions(command) {
                Ok(value) => value,
                Err(CompletionError::Render) => {
                    eprintln!("failed to render completion script");
                    std::process::exit(1);
                }
            };
            io::stdout().lock().write_all(value.script.as_bytes())?;
        }
    }

    Ok(())
}
