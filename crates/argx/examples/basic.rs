//! Minimal derive-facade example.

use argx::Parser;

/// Minimal CLI used to verify the derive facade in an executable.
#[derive(Debug, Parser)]
struct Cli;

fn main() {
    let _ = Cli;
}
