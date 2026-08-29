#[derive(argx::Parser)]
#[argx(long)]
struct InvalidCommandAttribute;

#[derive(argx::Args)]
struct InvalidFieldAttribute {
    #[argx(unknown)]
    value: String,
}

#[derive(argx::Args)]
struct InvalidShort {
    #[argx(short = 'λ')]
    value: bool,
}

#[derive(argx::Args)]
struct ReservedShort {
    #[argx(short = '=')]
    value: bool,
}

#[derive(argx::Subcommand)]
#[argx(alias = "run")]
enum InvalidSubcommandAttribute {
    Run,
}

#[derive(argx::Parser)]
struct HyphenPolicyOnPositional {
    #[argx(allow_hyphen_values)]
    value: String,
}

#[derive(argx::Parser)]
struct ValuePolicyOnSwitch {
    #[argx(long, allow_negative_numbers)]
    verbose: bool,
}

#[derive(argx::Parser)]
struct GlobalPositional {
    #[argx(global)]
    value: String,
}

#[derive(argx::Args)]
struct GlobalFlatten {
    #[argx(flatten, global)]
    value: InvalidFieldAttribute,
}

#[derive(argx::Parser)]
struct GlobalSubcommand {
    #[argx(subcommand, global)]
    command: Commands,
}

#[derive(argx::Subcommand)]
enum Commands {
    Run,
}

#[derive(argx::Parser)]
struct GlobalTakesNoValue {
    #[argx(long, global = true)]
    value: bool,
}

fn main() {}
