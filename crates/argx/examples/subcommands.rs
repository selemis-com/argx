//! Typed child-command selection with reusable payload arguments.
//!
//! A derived `Subcommand` enum defines the commands selectable at one scope. Unit variants need no
//! further values, while a variant with one direct `Args` payload delegates that command's fields
//! to the payload type.
//!
//! ```text
//! cargo run --example subcommands -- add hello
//! cargo run --example subcommands -- add hello --force
//! cargo run --example subcommands -- status
//! ```
//!
//! The `Remove(Add)` variant intentionally reuses the same `Args` declaration to demonstrate that
//! argument groups can back more than one command without introducing runtime reflection.

use argx::{Args, Parser, Subcommand};

/// Arguments accepted by the `add` and `remove` commands.
#[derive(Debug, Args)]
struct Add {
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
    Add(Add),
    /// Removes one value using the same argument shape.
    Remove(Add),
    /// Shows status without additional arguments.
    Status,
}

/// Root command that requires one child command selection.
#[derive(Debug, Parser)]
struct Cli {
    /// Selects one operation.
    #[argx(subcommand)]
    command: Command,
}

fn main() {
    let cli = Cli::parse();
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
