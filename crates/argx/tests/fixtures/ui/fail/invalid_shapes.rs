#[derive(argx::Parser)]
enum InvalidParser {
    Run,
}

#[derive(argx::Args)]
enum InvalidArgs {
    Run,
}

#[derive(argx::Subcommand)]
struct InvalidSubcommand;

fn main() {}
