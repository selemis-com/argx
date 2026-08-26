//! Hidden compatibility aliases for options and subcommands.
//!
//! Aliases are accepted by the parser but intentionally omitted from generated help, keeping one
//! canonical interface visible to people. Machine contracts retain aliases explicitly so tooling
//! can still discover every accepted spelling.
//!
//! Both of these invocations select the same canonical command and option:
//!
//! ```text
//! cargo run --example aliases -- --color always remove
//! cargo run --example aliases -- --colour always rm
//! ```
//!
//! Run `cargo run --example aliases -- --help` to see that only `--color` and `remove` are
//! advertised.

use argx::{Parser, Subcommand};

/// Commands accepted by the application.
#[derive(Debug, Subcommand)]
enum Command {
    /// Removes an item.
    #[argx(alias = "rm", aliases = ["delete", "del"])]
    Remove,
    /// Shows status.
    Status,
}

/// Command with compatibility aliases for one option and one child command.
#[derive(Debug, Parser)]
struct Cli {
    /// Optional output color.
    #[argx(long, alias = "colour", aliases = ["hue", "tone"])]
    color: Option<String>,
    /// Selects one operation.
    #[argx(subcommand)]
    command: Command,
}

fn main() {
    let cli = Cli::parse();

    if let Some(color) = cli.color {
        eprintln!("color: {color}");
    }

    match cli.command {
        Command::Remove => eprintln!("command: remove"),
        Command::Status => eprintln!("command: status"),
    }
}
