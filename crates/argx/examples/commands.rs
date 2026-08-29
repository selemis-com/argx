//! Subcommands, reusable flattened arguments, structured help, aliases, and version actions.
//!
//! Command topology is expressed with `Parser`, `Args`, and `Subcommand`. A flattened `Args` value
//! contributes fields directly to its containing command, while documentation remains the source
//! for summaries, descriptions, help groups, and appended sections.
//!
//! ```text
//! cargo run --example commands -- --verbose add hello --force
//! cargo run --example commands -- rm hello
//! cargo run --example commands -- --help
//! cargo run --example commands -- --version
//! cargo run --example commands -- status --version
//! ```

use argx::{Args, Parser, Subcommand};

/// Short version displayed by `--version`.
const VERSION: &str = "1.2.3";
/// Extended version displayed by `--version --verbose`.
const LONG_VERSION: &str = "1.2.3 (build abc123)";

/// Options shared by every command.
#[derive(Debug, Args)]
struct Common {
    /// Enables verbose output.
    #[argx(short, long, global)]
    verbose: bool,
}

/// Arguments accepted by the `add` and `remove` commands.
#[derive(Debug, Args)]
struct Item {
    /// Forces the operation.
    #[argx(long)]
    force: bool,

    /// Value to add or remove.
    value: String,
}

/// Commands accepted by the application.
#[derive(Debug, Subcommand)]
enum Command {
    /// Adds one value.
    Add(Item),

    /// Removes one value.
    #[argx(alias = "rm")]
    Remove(Item),

    /// Shows status without additional arguments.
    #[argx(version = VERSION, long_version = LONG_VERSION)]
    Status,
}

/// Manage values in the example workspace.
///
/// The longer command description is authored in the Rust documentation beside its type.
///
/// # Examples
///
///     commands add hello
///     commands --verbose rm hello
#[derive(Debug, Parser)]
#[argx(version = VERSION, long_version = LONG_VERSION)]
struct Cli {
    /// Common options
    #[argx(flatten)]
    common: Common,

    /// Selects one operation.
    #[argx(subcommand)]
    command: Command,
}

fn main() {
    let cli = Cli::parse();
    if cli.common.verbose {
        eprintln!("verbose mode enabled");
    }

    match cli.command {
        Command::Add(item) => {
            if item.force {
                eprintln!("force add: {}", item.value);
            } else {
                eprintln!("add: {}", item.value);
            }
        }
        Command::Remove(item) => eprintln!("remove: {}", item.value),
        Command::Status => eprintln!("status: ok"),
    }
}
