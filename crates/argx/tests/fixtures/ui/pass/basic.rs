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
    #[argx(long = "destination")]
    output: Option<String>,
}

#[derive(argx::Subcommand)]
enum Command {
    #[argx(about = "Run the command", alias = "execute", aliases = ["start", "go"])]
    Run,
}

#[derive(argx::Parser)]
struct Relations {
    #[argx(long, requires = ["token", "quiet"])]
    endpoint: Option<String>,
    #[argx(long)]
    token: Option<String>,
    #[argx(long, conflicts = ["quiet", "endpoint"])]
    verbose: bool,
    #[argx(long)]
    quiet: bool,
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
    let relations = Relations { endpoint: None, token: None, verbose: false, quiet: false };
    assert!(relations.endpoint.is_none());
    assert!(relations.token.is_none());
    assert!(!relations.verbose);
    assert!(!relations.quiet);
}
