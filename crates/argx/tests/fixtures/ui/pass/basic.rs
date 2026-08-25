#[derive(argx::Parser)]
struct Cli {
    #[argx(short, long)]
    verbose: bool,
    input: String,
}

#[derive(argx::Args)]
struct Common {
    #[argx(long)]
    output: Option<String>,
}

#[derive(argx::Subcommand)]
enum Command {
    Run,
}

fn assert_parser<T: argx::Parser>() {}

fn main() {
    assert_parser::<Cli>();
    let command = <Cli as argx::__private::CommandArgs>::COMMAND;
    assert_eq!(command.flags.len(), 1);
    assert_eq!(command.args.len(), 1);

    let cli = Cli { verbose: false, input: String::new() };
    assert!(!cli.verbose);
    assert!(cli.input.is_empty());
    let common = Common { output: None };
    assert!(common.output.is_none());
    let _ = Command::Run;
}
