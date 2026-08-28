//! Structured help authored directly through Rust documentation.
//!
//! Argx treats documentation as command metadata rather than maintaining separate help strings.
//! The first paragraph supplies the short summary, prose before the first level-one heading becomes
//! the command description, and level-one headings become appended help sections.
//!
//! The most useful way to inspect this example is:
//!
//! ```text
//! cargo run --example structured_help -- --help
//! ```
//!
//! Notice that the `Output` documentation on the flattened field creates a dedicated argument
//! group and that the authored sections are rendered after Argx's generated sections.

use argx::{Args, Parser};

/// Output controls shared by the command.
#[derive(Debug, Args)]
struct Output {
    /// Emit structured JSON output.
    #[argx(long)]
    json: bool,
    /// Include one output field; repeat the option to select more than one.
    #[argx(long)]
    field: Vec<String>,
}

/// Inspect objects in a workspace.
///
/// The command keeps its longer explanation alongside the Rust type that defines it.
///
/// # Examples
///
///     structured-help --field id --field title
///     structured-help --json
#[derive(Debug, Parser)]
#[argx(name = "structured-help")]
struct Cli {
    /// Output
    #[argx(flatten)]
    output: Output,
}

fn main() {
    let cli = Cli::parse();

    if cli.output.json {
        eprintln!("json output enabled");
    }
    for field in cli.output.field {
        eprintln!("field: {field}");
    }
}
