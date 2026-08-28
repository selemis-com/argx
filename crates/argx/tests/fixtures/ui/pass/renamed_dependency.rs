use cli_args::argx;

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
    let cli = Cli { verbose: false, value: String::from("value") };
    assert!(!cli.verbose);
    assert_eq!(cli.value, "value");
    let _ = Common;
    let _ = Command::Run;
    assert_eq!(<Mode as cli_args::ValueEnum>::VALUES, &["fast", "dry-run"]);
    let _ = <Common as cli_args::HandlerSchemaSource>::schemas();
    let Ok(output) = handler() else { panic!("handler failed") };
    assert_eq!(output.value, "ok");
}

#[argx(schema)]
struct Output {
    value: String,
}

#[argx(handler = Common)]
fn handler() -> Result<Output, ()> {
    Ok(Output { value: String::from("ok") })
}
