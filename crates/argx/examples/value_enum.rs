//! Finite command-line values with generated help.
//!
//! `ValueEnum` keeps the accepted vocabulary in the enum declaration instead of duplicating it in
//! field documentation. Argx uses the same values for typed parsing, generated help, and machine
//! contracts.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example value_enum -- --output json-lines
//! cargo run --example value_enum -- --help
//! ```
//!
//! Help lists `human-readable`, `json`, and `json-lines` automatically.

use argx::Parser;

/// Output format accepted by the command.
#[derive(Debug, argx::ValueEnum)]
enum Output {
    /// Human-readable text.
    HumanReadable,
    /// One JSON document.
    Json,
    /// Newline-delimited JSON.
    JsonLines,
}

/// Command whose output format has a finite vocabulary.
#[derive(Debug, Parser)]
struct Cli {
    /// Output format.
    #[argx(long, value_enum)]
    output: Output,
}

fn main() {
    let cli = Cli::parse();

    match cli.output {
        Output::HumanReadable => eprintln!("human-readable output"),
        Output::Json => eprintln!("JSON output"),
        Output::JsonLines => eprintln!("JSON Lines output"),
    }
}
