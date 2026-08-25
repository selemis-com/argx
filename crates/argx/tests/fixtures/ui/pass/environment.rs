use argx::Parser as _;

#[derive(argx::Parser)]
struct Cli {
    #[argx(long, env = "ARGX_UI_PORT", default = 3000_u16)]
    port: u16,
    #[argx(long, env = "ARGX_UI_PROFILE")]
    profile: Option<String>,
}

fn main() {
    if let Ok(cli) = Cli::try_parse_args(["--port", "4000"]) {
        let _ = (cli.port, cli.profile);
    }
}
