#[derive(argx::Parser)]
struct DuplicateLong {
    #[argx(long = "same")]
    first: bool,
    #[argx(long = "same")]
    second: bool,
}

#[derive(argx::Parser)]
struct DuplicateShort {
    #[argx(short = 'x')]
    first: bool,
    #[argx(short = 'x')]
    second: bool,
}

#[derive(argx::Parser)]
struct InvalidLong {
    #[argx(long = "--bad")]
    value: bool,
}

#[derive(argx::Parser)]
struct RequiredAfterOptional {
    optional: Option<String>,
    required: String,
}

#[derive(argx::Parser)]
struct VariadicBeforeAnotherPositional {
    many: Vec<String>,
    later: Option<String>,
}

fn main() {}
