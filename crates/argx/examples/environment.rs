//! Environment fallbacks and their precedence relative to `argv` and defaults.
//!
//! A configured environment variable is consulted only when the corresponding option was absent
//! from `argv`. A typed default is then used when neither source supplied a value.
//!
//! On supported Unix targets:
//!
//! ```text
//! ARGX_PORT=8080 ARGX_CONFIG=./argx.toml cargo run --example environment --
//! ```
//!
//! Passing `--port 9000` in the same invocation overrides `ARGX_PORT`; the environment is a
//! fallback rather than a second occurrence of the option.

use std::path::PathBuf;

use argx::Parser;

/// Port used when neither `--port` nor `ARGX_PORT` is provided.
const DEFAULT_PORT: u16 = 3000;

/// Command with environment-backed scalar values.
#[derive(Debug, Parser)]
struct Cli {
    /// HTTP port, optionally read from `ARGX_PORT`.
    #[argx(long, env = "ARGX_PORT", default = DEFAULT_PORT)]
    port: u16,
    /// Optional configuration path, read from `ARGX_CONFIG` when absent from `argv`.
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
