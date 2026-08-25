//! Shared support for executable integration tests.

use std::{path::PathBuf, sync::OnceLock};

use snapbox::cmd::Command;

/// Compiled path for the basic public example.
static BASIC_EXAMPLE: OnceLock<PathBuf> = OnceLock::new();

/// Returns the compiled path for the real `basic` example.
pub(crate) fn basic_example_path() -> &'static PathBuf {
    BASIC_EXAMPLE.get_or_init(|| {
        snapbox::cmd::compile_example("basic", std::iter::empty::<&str>())
            .unwrap_or_else(|error| panic!("failed to compile basic example: {error}"))
    })
}

/// Returns a deterministic command targeting the real `basic` example.
pub(crate) fn basic_example_command() -> Command {
    Command::new(basic_example_path()).env("NO_COLOR", "1")
}
