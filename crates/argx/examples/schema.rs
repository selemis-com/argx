//! Machine-readable invocation, result, and error schemas.
//!
//! Structural discovery is shallow by default, while reaching a leaf automatically exposes its
//! complete invocation, result, error, and referenced type schemas. `--full` recursively expands a
//! structural command.
//!
//! ```text
//! cargo run --example schema -- schema
//! cargo run --example schema -- schema objects
//! cargo run --example schema -- schema objects get
//! cargo run --example schema -- schema objects --full
//! cargo run --example schema -- objects get object-7 --schema
//! cargo run --example schema -- objects get object-7
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

/// List objects.
#[derive(argx::Args)]
struct List;

/// Successful result returned by `List`.
#[argx(schema)]
struct ListOutput {
    /// Returned object identifiers.
    ids: Vec<String>,
}

/// Errors returned by `List`.
#[argx(schema)]
enum ListError {
    /// Listing objects failed.
    Unavailable,
}

/// Executes a `List` command.
#[argx(handler = List)]
fn list(_: List) -> Result<ListOutput, ListError> {
    let ids = vec!["object-7".to_owned()];
    if ids.is_empty() { Err(ListError::Unavailable) } else { Ok(ListOutput { ids }) }
}

/// Object commands.
#[derive(argx::Subcommand)]
#[argx(schema)]
enum ObjectCommand {
    /// Retrieve one object.
    Get(Get),

    /// List objects.
    List(List),
}

/// Manage objects.
#[derive(argx::Args)]
#[argx(schema)]
struct Objects {
    /// Selects an object operation.
    #[argx(subcommand)]
    command: ObjectCommand,
}

/// Commands exposed by the schema-enabled application.
#[derive(argx::Subcommand)]
#[argx(schema)]
enum Command {
    /// Manage objects.
    Objects(Objects),
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
        Command::Objects(objects) => match objects.command {
            ObjectCommand::Get(command) => match get(command) {
                Ok(output) => println!("id: {}", output.id),
                Err(GetError::NotFound) => {
                    eprintln!("object not found");
                    std::process::exit(1);
                }
            },
            ObjectCommand::List(command) => match list(command) {
                Ok(output) => println!("{}", output.ids.join("\n")),
                Err(ListError::Unavailable) => {
                    eprintln!("objects unavailable");
                    std::process::exit(1);
                }
            },
        },
    }
}
