#[derive(cli_args::Parser)]
struct Cli {
    #[argx(long)]
    verbose: bool,
}

#[derive(cli_args::Args)]
struct Common;

#[derive(cli_args::Subcommand)]
enum Command {
    Run,
}

fn assert_parser<T: cli_args::Parser>() {}

fn main() {
    assert_parser::<Cli>();
    let command = <Cli as cli_args::__private::CommandArgs>::COMMAND;
    assert_eq!(command.flags[0].longs, ["verbose"]);

    let cli = Cli { verbose: false };
    assert!(!cli.verbose);
    let _ = Common;
    let _ = Command::Run;
}
