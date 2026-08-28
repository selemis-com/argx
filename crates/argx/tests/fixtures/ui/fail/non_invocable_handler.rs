#[derive(argx::Args)]
struct RunArgs;

#[argx::handler(RunArgs)]
fn run_handler() -> Result<(), ()> {
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

#[argx::handler(GroupArgs)]
fn invalid_group_handler() -> Result<(), ()> {
    Ok(())
}

fn main() {}
