//! Reusable flattened argument-group example.

use std::path::PathBuf;

use argx::{Args, Parser};

/// Arguments shared by one or more commands.
#[derive(Debug, Args)]
struct Common {
    /// Enables verbose output.
    #[argx(long)]
    verbose: bool,
    /// Optional configuration file.
    #[argx(long)]
    config: Option<PathBuf>,
}

/// Command composed from a reusable argument group.
#[derive(Debug, Parser)]
struct Cli {
    /// Shared arguments flattened into this command.
    #[argx(flatten)]
    common: Common,
}

fn main() {
    let cli = Cli::parse();

    if cli.common.verbose {
        eprintln!("verbose mode enabled");
    }
    if let Some(config) = cli.common.config {
        eprintln!("config: {}", config.display());
    }
}
