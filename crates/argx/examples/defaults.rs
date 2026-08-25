//! Typed default value example.

use argx::Parser;

/// Default port used when `--port` is omitted.
const DEFAULT_PORT: u16 = 3000;

/// Command with typed Rust defaults.
#[derive(Debug, Parser)]
struct Cli {
    /// Port to listen on.
    #[argx(long, default = DEFAULT_PORT)]
    port: u16,

    /// Optional deployment profile with a default value.
    #[argx(long, default = String::from("development"))]
    profile: Option<String>,
}

fn main() {
    let Cli { port: _port, profile: _profile } = Cli::parse();
}
