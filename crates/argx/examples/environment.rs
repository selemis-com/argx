//! Environment-backed value example.

use std::path::PathBuf;

use argx::Parser;

/// Port used when neither `--port` nor `ARGX_PORT` is provided.
const DEFAULT_PORT: u16 = 3000;

/// Command with environment-backed values.
#[derive(Debug, Parser)]
struct Cli {
    /// HTTP port, optionally read from `ARGX_PORT`.
    #[argx(long, env = "ARGX_PORT", default = DEFAULT_PORT)]
    port: u16,
    /// Optional configuration path, read from `ARGX_CONFIG` when absent from argv.
    #[argx(long, env = "ARGX_CONFIG")]
    config: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    eprintln!("port: {}", cli.port);
    if let Some(config) = cli.config {
        eprintln!("config: {}", config.display());
    }
}
