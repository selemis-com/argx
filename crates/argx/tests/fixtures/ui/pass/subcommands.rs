use argx::Parser as _;

#[derive(Debug, PartialEq, Eq, argx::Args)]
struct Shared {
    #[argx(long)]
    dry_run: bool,
}

#[derive(Debug, PartialEq, Eq, argx::Args)]
struct Add {
    #[argx(flatten)]
    shared: Shared,
    value: String,
}

#[derive(Debug, PartialEq, Eq, argx::Subcommand)]
enum ConfigCommand {
    Get,
}

#[derive(Debug, PartialEq, Eq, argx::Args)]
struct Config {
    #[argx(subcommand)]
    command: ConfigCommand,
}

#[derive(Debug, PartialEq, Eq, argx::Subcommand)]
enum Command {
    Add(Add),
    Config(Config),
    #[argx(name = "show-status")]
    Status,
}

#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct Cli {
    #[argx(long)]
    verbose: bool,
    #[argx(subcommand)]
    command: Command,
}

fn main() {
    assert_eq!(
        Cli::try_parse_from(["argx-test", "add", "--dry-run", "value"]),
        Ok(Cli {
            verbose: false,
            command: Command::Add(Add {
                shared: Shared { dry_run: true },
                value: String::from("value"),
            }),
        }),
    );
    assert_eq!(
        Cli::try_parse_from(["argx-test", "config", "get"]),
        Ok(Cli {
            verbose: false,
            command: Command::Config(Config { command: ConfigCommand::Get }),
        }),
    );
    assert_eq!(
        Cli::try_parse_from(["argx-test", "show-status"]),
        Ok(Cli { verbose: false, command: Command::Status }),
    );
}
