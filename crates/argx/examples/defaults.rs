//! Typed Rust defaults for scalar named options.
//!
//! Defaults are evaluated as Rust expressions by generated binding code. They therefore satisfy an
//! omitted option without converting a textual default through the command-line parser.
//!
//! With no arguments, this example resolves `port` to `3000` and `profile` to `development`:
//!
//! ```text
//! cargo run --example defaults --
//! ```
//!
//! Explicit `argv` values take precedence:
//!
//! ```text
//! cargo run --example defaults -- --port 8080 --profile production
//! ```

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
    let cli = Cli::parse();

    eprintln!("port: {}", cli.port);
    if let Some(profile) = cli.profile {
        eprintln!("profile: {profile}");
    }
}
