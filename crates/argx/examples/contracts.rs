//! Prints a machine-readable contract for an invocable command.
//!
//! Argx derives invocation metadata from the same command model used by parsing and attaches the
//! handler's semantic success and error types through `#[argx::contract(...)]`.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example contracts -- get object-7
//! ```
//!
//! The selected command contract is written as pretty-printed JSON on stdout.

use std::io::{self, Write};

use argx::{Args, ContractRequest, Parser, Subcommand};

/// Contract-discovery example.
#[derive(Debug, Parser)]
#[argx(name = "contracts")]
struct Cli {
    /// Selects one operation.
    #[argx(subcommand)]
    command: Command,
}

/// Supported example commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Retrieve one object.
    Get(GetArgs),
}

/// Arguments accepted by `get`.
#[derive(Debug, Args)]
struct GetArgs {
    /// Object identifier.
    id: String,
}

/// Successful `get` result.
#[derive(Debug, argx::Contract)]
struct GetOutput {
    /// Retrieved object identifier.
    id: String,
}

/// Failed `get` result.
#[derive(Debug, argx::Contract)]
enum GetError {
    /// The requested object does not exist.
    NotFound,
}

/// Retrieves one object by identifier.
#[argx::contract(GetArgs)]
fn get(args: GetArgs) -> Result<GetOutput, GetError> {
    if args.id == "missing" { Err(GetError::NotFound) } else { Ok(GetOutput { id: args.id }) }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Get(args) => match get(args) {
            Ok(output) => drop(output.id),
            Err(GetError::NotFound) => {}
        },
    }

    let contract = match Cli::contract(ContractRequest::new(["get"])) {
        Ok(contract) => contract,
        Err(error) => {
            eprintln!("failed to discover contract: {error}");
            std::process::exit(1);
        }
    };
    let json = match contract.to_json_pretty() {
        Ok(json) => json,
        Err(error) => {
            eprintln!("failed to serialize contract: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = writeln!(io::stdout().lock(), "{json}") {
        eprintln!("failed to write contract: {error}");
        std::process::exit(1);
    }
}
