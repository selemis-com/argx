//! Generates a dynamic shell-completion adapter.
//!
//! Argx completion scripts are intentionally small: the shell adapter sends the command line back
//! to the executable, and Argx resolves candidates through the same command model and argv parser
//! used for ordinary invocations.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example completions -- bash
//! cargo run --example completions -- fish
//! cargo run --example completions -- zsh
//! ```
//!
//! The generated script is written to stdout, so it can be inspected or saved directly:
//!
//! ```text
//! cargo run --quiet --example completions -- zsh | less
//! cargo run --quiet --example completions -- zsh > /tmp/_completions
//! ```

use std::io::{self, Write};

use argx::{Parser, completion::Shell};

/// Completion-script generator for this example binary.
#[derive(Debug, Parser)]
#[argx(name = "completions")]
struct Cli {
    /// Shell whose completion adapter should be generated.
    #[argx(value_enum)]
    shell: Shell,
}

fn main() {
    let cli = Cli::parse();
    let script = match Cli::render_completion(cli.shell) {
        Ok(script) => script,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = io::stdout().lock().write_all(script.as_bytes()) {
        eprintln!("failed to write completion script: {error}");
        std::process::exit(1);
    }
}
