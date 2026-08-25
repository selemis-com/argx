//! Argument relationship example.

use std::path::PathBuf;

use argx::Parser;

/// Command with a small set of argument relationships.
#[derive(Debug, Parser)]
struct Cli {
    /// Remote endpoint to call.
    #[argx(long, requires = ["token", "workspace"])]
    endpoint: Option<String>,
    /// Authentication token used for remote calls.
    #[argx(long)]
    token: Option<String>,
    /// Workspace to operate in.
    #[argx(long)]
    workspace: Option<String>,
    /// Write the result to a file.
    #[argx(long, conflicts = ["stdout", "validate_only"])]
    output: Option<PathBuf>,
    /// Write the result to standard output.
    #[argx(long)]
    stdout: bool,
    /// Validate the request without writing output.
    #[argx(long = "validate-only")]
    validate_only: bool,
}

fn main() {
    let cli = Cli::parse();

    if let Some(endpoint) = cli.endpoint {
        eprintln!("endpoint: {endpoint}");
    }
    if cli.token.is_some() {
        eprintln!("token: provided");
    }
    if let Some(workspace) = cli.workspace {
        eprintln!("workspace: {workspace}");
    }
    if let Some(output) = cli.output {
        eprintln!("output: {}", output.display());
    }
    eprintln!("stdout: {}", cli.stdout);
    eprintln!("validate only: {}", cli.validate_only);
}
