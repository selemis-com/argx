//! Smallest complete Argx application.
//!
//! This example shows the minimum integration point: derive [`Parser`] for the root command and
//! call [`Parser::parse`] from `main`. Even an otherwise empty command receives Argx's built-in
//! `-h` / `--help` action.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example basic -- --help
//! ```
//!
//! Start here when validating installation or when embedding Argx into a new binary before adding
//! application arguments.

use argx::Parser;

/// Minimal command with no application-defined arguments.
#[derive(Debug, Parser)]
struct Cli;

fn main() {
    let _cli = Cli::parse();
}
