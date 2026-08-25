#[derive(argx::Parser)]
struct Cli;

#[derive(argx::Args)]
struct Common;

#[derive(argx::Subcommand)]
enum Command {
    Run,
}

fn assert_parser<T: argx::Parser>() {}

fn main() {
    assert_parser::<Cli>();
    let _ = Common;
    let _ = Command::Run;
}
