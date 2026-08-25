//! Nested subcommand example.

use argx::{Args, Parser, Subcommand};

/// Arguments accepted by the `add` command.
#[derive(Debug, Args)]
struct Add {
    /// Forces the operation.
    #[argx(long)]
    force: bool,
    /// Value to add.
    value: String,
}

/// Commands accepted by the application.
#[derive(Debug, Subcommand)]
enum Command {
    /// Adds one value.
    Add(Add),
    /// Removes one value using the same argument shape.
    Remove(Add),
    /// Shows status without additional arguments.
    Status,
}

/// Root command.
#[derive(Debug, Parser)]
struct Cli {
    /// Selects one operation.
    #[argx(subcommand)]
    command: Command,
}

fn main() {
    let cli = Cli::try_parse_args(["status"]).expect("static example arguments are valid");
    match cli.command {
        Command::Add(add) => {
            if add.force {
                eprintln!("force add: {}", add.value);
            } else {
                eprintln!("add: {}", add.value);
            }
        }
        Command::Remove(remove) => {
            eprintln!("remove: {}", remove.value);
        }
        Command::Status => {}
    }
}
