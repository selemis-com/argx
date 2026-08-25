use argx::Parser as _;

#[derive(argx::Parser)]
#[argx(about = "Basic parser")]
struct Cli {
    #[argx(short, long, alias = "chatty", aliases = ["debug", "trace"], help = "Verbose output")]
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
    #[argx(about = "Run the command", alias = "execute", aliases = ["start", "go"])]
    Run,
}

fn assert_parser<T: argx::Parser>() {}

fn main() {
    assert_parser::<Cli>();
    let command = <Cli as argx::__private::CommandArgs>::COMMAND;
    assert_eq!(command.flags.len(), 1);
    assert_eq!(command.args.len(), 1);
    assert_eq!(command.about, Some("Basic parser"));
    assert_eq!(command.flags[0].help, Some("Verbose output"));
    assert_eq!(command.flags[0].aliases, ["chatty", "debug", "trace"]);
    assert!(Cli::render_help().contains("Usage: cli [OPTIONS] <INPUT>"));

    let cli = Cli { verbose: false, input: String::new() };
    assert!(!cli.verbose);
    assert!(cli.input.is_empty());
    let common = Common { output: None };
    assert!(common.output.is_none());
    let _ = Command::Run;
}
