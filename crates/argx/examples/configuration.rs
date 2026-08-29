//! Resolves one typed configuration from explicitly ordered layers.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example configuration -- --workers 8
//! ```
//!
//! The example keeps file layers optional so it can run without setup. Set `ARGX_EXAMPLE_ENDPOINT`
//! or pass `--endpoint` to override the declared default.

use argx::{Argv, Defaults, Environment};

/// Application configuration shared across defaults, environment, and argv.
#[derive(Debug, argx::Config)]
#[argx(prefix = "ARGX_EXAMPLE")]
struct Config {
    /// Number of worker tasks.
    #[argx(long, default = 4)]
    workers: usize,

    /// Service endpoint.
    #[argx(long, default = String::from("http://localhost:8080"))]
    endpoint: String,
}

fn main() -> Result<(), argx::ConfigError> {
    let config = match Config::loader()
        .layer(Defaults)
        .layer(Environment)
        .layer(Argv::current())
        .resolve()
    {
        Ok(config) => config,
        Err(argx::ConfigError::Arguments(error)) => error.exit(),
        Err(error) => return Err(error),
    };

    println!("workers: {}", config.workers);
    println!("endpoint: {}", config.endpoint);
    Ok(())
}
