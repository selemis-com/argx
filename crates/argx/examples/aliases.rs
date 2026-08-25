//! Hidden flag and subcommand aliases example.

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

/// Command with compatibility aliases.
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
