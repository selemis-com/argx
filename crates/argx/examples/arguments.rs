//! Named and positional arguments, defaults, constraints, aliases, and finite values.
//!
//! This example collects the most common field-level features in one command instead of splitting
//! each attribute into its own executable. Hidden aliases remain accepted without appearing in
//! generated help, defaults are typed Rust expressions, and `requires` / `conflicts` are validated
//! after parsing.
//!
//! ```text
//! cargo run --example arguments -- input.txt --format json --colour always
//! cargo run --example arguments -- input.txt --endpoint https://example.invalid --token secret
//! cargo run --example arguments -- --help
//! ```

use std::path::PathBuf;

use argx::Parser;

/// Output format accepted by the command.
#[derive(Debug, argx::ValueEnum)]
enum Format {
    /// Human-readable text.
    Human,
    /// One JSON document.
    Json,
    /// Newline-delimited JSON.
    JsonLines,
}

/// Inspect one input using typed command-line arguments.
#[derive(Debug, Parser)]
struct Cli {
    /// Input file to inspect.
    input: PathBuf,

    /// Output format.
    #[argx(long, value_enum, default = Format::Human)]
    format: Format,

    /// Optional output color.
    #[argx(long, alias = "colour")]
    color: Option<String>,

    /// Tag to include; repeat the option to include more than one.
    #[argx(long)]
    tag: Vec<String>,

    /// Remote endpoint to call.
    #[argx(long, requires = ["token"])]
    endpoint: Option<String>,

    /// Authentication token used for remote calls.
    #[argx(long)]
    token: Option<String>,

    /// Write the result to a file.
    #[argx(long, conflicts = ["stdout"])]
    destination: Option<PathBuf>,

    /// Write the result to standard output.
    #[argx(long)]
    stdout: bool,
}

fn main() {
    let cli = Cli::parse();

    eprintln!("input: {}", cli.input.display());
    let format = match cli.format {
        Format::Human => "human",
        Format::Json => "json",
        Format::JsonLines => "json-lines",
    };
    eprintln!("format: {format}");
    if let Some(color) = cli.color {
        eprintln!("color: {color}");
    }
    for tag in cli.tag {
        eprintln!("tag: {tag}");
    }
    if let Some(endpoint) = cli.endpoint {
        eprintln!("endpoint: {endpoint}");
    }
    if cli.token.is_some() {
        eprintln!("token: provided");
    }
    if let Some(destination) = cli.destination {
        eprintln!("destination: {}", destination.display());
    }
    eprintln!("stdout: {}", cli.stdout);
}
