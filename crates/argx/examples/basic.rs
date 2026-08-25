//! Minimal typed parser example.

use argx::Parser;

/// Minimal command with no declared arguments.
#[derive(Debug, Parser)]
struct Cli;

fn main() {
    let _cli = Cli::parse();
}
