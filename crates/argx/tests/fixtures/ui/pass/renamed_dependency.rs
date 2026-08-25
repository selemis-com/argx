#[derive(cli_args::Parser)]
struct Cli;

#[derive(cli_args::Args)]
struct Common;

#[derive(cli_args::Subcommand)]
enum Command {
    Run,
}

fn assert_parser<T: cli_args::Parser>() {}

fn main() {
    assert_parser::<Cli>();
    let _ = Common;
    let _ = Command::Run;
}
