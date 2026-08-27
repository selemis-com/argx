use argx::Parser as _;

#[derive(Debug, PartialEq, Eq, argx::ValueEnum)]
enum Mode {
    HumanReadable,
    Json,
}

#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct Cli {
    #[argx(long, value_enum)]
    mode: Option<Mode>,
    #[argx(value_enum)]
    fallback: Mode,
}

fn main() {
    let cli = Cli::try_parse_args(["--mode", "json", "human-readable"]).expect("value enum");
    assert_eq!(cli.mode, Some(Mode::Json));
    assert_eq!(cli.fallback, Mode::HumanReadable);
    assert_eq!(<Mode as argx::ValueEnum>::VALUES, &["human-readable", "json"]);
}
