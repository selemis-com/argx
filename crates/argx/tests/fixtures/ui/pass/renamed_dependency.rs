use cli_args::Parser as _;

#[derive(cli_args::ValueEnum)]
enum Mode {
    Fast,
    DryRun,
}

#[derive(cli_args::Parser)]
struct Cli {
    #[argx(long)]
    verbose: bool,
    value: String,
}

#[cli_args::contract(Cli)]
fn cli_contract(_command: Cli) -> Result<(), ()> {
    Ok(())
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
    let contract = Cli::contract(cli_args::ContractRequest::root()).expect("contract");
    assert_eq!(contract.root, "cli");
    assert_eq!(contract.command.invocation.expect("invocation")[0].positionals.len(), 1);

    let _ = cli_contract(Cli { verbose: false, value: String::from("contract") });

    let cli = Cli { verbose: false, value: String::from("value") };
    assert!(!cli.verbose);
    assert_eq!(cli.value, "value");
    let _ = Common;
    let _ = Command::Run;
    assert_eq!(<Mode as cli_args::ValueEnum>::VALUES, &["fast", "dry-run"]);
}
