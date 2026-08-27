#[derive(argx::Args)]
struct RunArgs;

#[argx::contract(RunArgs)]
fn run_contract() -> Result<(), ()> {
    Ok(())
}

#[derive(argx::Subcommand)]
enum Commands {
    Run(RunArgs),
}

#[derive(argx::Args)]
struct GroupArgs {
    #[argx(subcommand)]
    command: Commands,
}

#[argx::contract(GroupArgs)]
fn invalid_group_contract() -> Result<(), ()> {
    Ok(())
}

fn main() {}
