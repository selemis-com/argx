//! Machine-readable invocation, result, and error schemas.
//!
//! `#[argx(schema)]` enables schema discovery for a command tree. `#[argx(handler = ...)]`
//! associates an executable leaf with its typed result and error contracts without runtime
//! registration.
//!
//! ```text
//! cargo run --example schema -- schema get
//! cargo run --example schema -- get object-7 --schema
//! cargo run --example schema -- get object-7
//! ```

use argx::{Parser as _, argx};

/// Retrieve one object.
#[derive(argx::Args)]
struct Get {
    /// Object identifier.
    id: String,
}

/// Successful result returned by `Get`.
#[argx(schema)]
struct GetOutput {
    /// Returned object identifier.
    id: String,
}

/// Errors returned by `Get`.
#[argx(schema)]
enum GetError {
    /// The requested object does not exist.
    NotFound,
}

/// Executes a `Get` command.
#[argx(handler = Get)]
fn get(command: Get) -> Result<GetOutput, GetError> {
    if command.id == "missing" { Err(GetError::NotFound) } else { Ok(GetOutput { id: command.id }) }
}

/// Commands exposed by the schema-enabled application.
#[derive(argx::Subcommand)]
#[argx(schema)]
enum Command {
    /// Retrieve one object.
    Get(Get),
}

/// Schema-enabled command tree.
#[derive(argx::Parser)]
#[argx(name = "schema", schema)]
struct Cli {
    /// Selects one schema-enabled command.
    #[argx(subcommand)]
    command: Command,
}

fn main() {
    match Cli::parse().command {
        Command::Get(command) => match get(command) {
            Ok(output) => println!("id: {}", output.id),
            Err(GetError::NotFound) => {
                eprintln!("object not found");
                std::process::exit(1);
            }
        },
    }
}
