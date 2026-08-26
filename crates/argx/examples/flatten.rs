//! Reusable argument groups composed into a command with `flatten`.
//!
//! `#[derive(Args)]` defines a reusable argument declaration. A `#[argx(flatten)]` field mounts
//! those arguments directly into the containing command: no extra command scope appears in `argv`.
//! The flatten field's documentation also becomes a generated help-group heading.
//!
//! ```text
//! cargo run --example flatten -- --verbose --config ./argx.toml
//! cargo run --example flatten -- --help
//! ```
//!
//! Flattening is most useful for policy or output options shared by several independently derived
//! commands.

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
    /// Shared arguments
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
