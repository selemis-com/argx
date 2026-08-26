//! Short and long version metadata on root and child commands.
//!
//! A command with `version` and/or `long_version` receives built-in `-V` and `--version` actions.
//! Version actions are lexical: the selected command must declare version metadata itself.
//!
//! ```text
//! cargo run --example version -- -V
//! cargo run --example version -- --version
//! cargo run --example version -- run --version
//! ```
//!
//! `internal` deliberately has no version metadata, so `internal --version` is rejected instead of
//! silently falling back to the root command's version action.

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

/// Root command with independently configured version metadata.
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
