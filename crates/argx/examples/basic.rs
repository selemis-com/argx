//! Smallest complete Argx application: derive [`Parser`] and call [`Parser::parse`].
//!
//! Run it with:
//!
//! ```text
//! cargo run --example basic -- --help
//! ```

use argx::Parser;

/// Minimal command with no application-defined arguments.
#[derive(Debug, Parser)]
struct Cli;

fn main() {
    let _cli = Cli::parse();
}
