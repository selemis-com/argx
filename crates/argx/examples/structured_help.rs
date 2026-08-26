//! Structured help authored directly through Rust documentation.

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
///
/// # Machine-readable usage
///
/// Use the application's schema command when a structured command contract is needed.
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
