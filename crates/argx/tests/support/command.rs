//! Shared support for executable integration tests.

use std::{path::PathBuf, sync::OnceLock};

use snapbox::cmd::Command;

/// Compiled path for the basic public example.
static BASIC_EXAMPLE: OnceLock<PathBuf> = OnceLock::new();
/// Compiled path for the version public example.
static VERSION_EXAMPLE: OnceLock<PathBuf> = OnceLock::new();
/// Compiled path for the subcommand public example.
static SUBCOMMANDS_EXAMPLE: OnceLock<PathBuf> = OnceLock::new();

/// Returns a deterministic command targeting one real public example.
pub(crate) fn example_command(name: &str) -> Command {
    let binary = match name {
        "basic" => BASIC_EXAMPLE.get_or_init(|| compile_example("basic")),
        "version" => VERSION_EXAMPLE.get_or_init(|| compile_example("version")),
        "subcommands" => SUBCOMMANDS_EXAMPLE.get_or_init(|| compile_example("subcommands")),
        other => panic!("unsupported CLI-test example `{other}`"),
    };
    Command::new(binary).env("NO_COLOR", "1")
}

/// Compiles one public example for executable integration testing.
fn compile_example(name: &str) -> PathBuf {
    snapbox::cmd::compile_example(name, ["--all-features"])
        .unwrap_or_else(|error| panic!("failed to compile {name} example: {error}"))
}
