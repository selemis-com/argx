use argx::Parser as _;

#[derive(argx::Args)]
struct RunArgs;

#[argx::contract(RunArgs)]
fn run_contract() -> Result<(), ()> {
    Ok(())
}

#[derive(argx::Subcommand)]
enum Commands {
    Run(RunArgs),
    Status,
}

#[derive(argx::Parser)]
struct Cli {
    #[argx(subcommand)]
    command: Commands,
}

fn main() {
    let _ = Cli::contract(argx::ContractRequest::root());
}
