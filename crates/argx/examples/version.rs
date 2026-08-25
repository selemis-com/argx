//! Command version metadata example.

use argx::{Parser, Subcommand};

/// Short version displayed by `-V`.
const VERSION: &str = "1.2.3";
/// Detailed version displayed by `--version`.
const LONG_VERSION: &str = "1.2.3 (build abc123)";

/// Commands accepted by the application.
#[derive(Debug, Subcommand)]
enum Command {
    /// Runs the versioned command.
    #[argx(version = VERSION, long_version = LONG_VERSION)]
    Run,
    /// Runs an unversioned command.
    Internal,
}

/// Version metadata example.
#[derive(Debug, Parser)]
#[argx(version = VERSION, long_version = LONG_VERSION)]
struct Cli {
    /// Selects one operation.
    #[argx(subcommand)]
    command: Command,
}

fn main() {
    match Cli::parse().command {
        Command::Run | Command::Internal => {}
    }
}
