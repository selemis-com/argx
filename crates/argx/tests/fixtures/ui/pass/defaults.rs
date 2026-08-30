use argx::Parser as _;

const DEFAULT_PORT: u16 = 3000;

#[derive(argx::Parser)]
struct Cli {
    #[argx(long, default = DEFAULT_PORT)]
    port: u16,
    #[argx(long, default = String::from("development"))]
    profile: Option<String>,
}

fn main() {
    let cli = Cli::try_parse_from(["argx-test"]).expect("defaults should compile");
    assert_eq!(cli.port, DEFAULT_PORT);
    assert_eq!(cli.profile.as_deref(), Some("development"));
}
