#[derive(argx::Parser)]
#[argx(alias = "tool")]
struct RootAlias;

#[derive(argx::Parser)]
struct PositionalAlias {
    #[argx(alias = "other")]
    value: String,
}

#[derive(argx::Parser)]
struct EmptyAliases {
    #[argx(long, aliases = [])]
    value: bool,
}

#[derive(argx::Parser)]
struct InvalidAlias {
    #[argx(long, alias = "--bad")]
    value: bool,
}

#[derive(argx::Parser)]
struct ReservedAlias {
    #[argx(long, alias = "help")]
    value: bool,
}

#[derive(argx::Subcommand)]
enum DuplicateSubcommandAlias {
    #[argx(alias = "second")]
    First,
    Second,
}

#[derive(argx::Subcommand)]
enum RepeatedSubcommandAlias {
    #[argx(aliases = ["run", "run"])]
    Execute,
}

fn main() {}
