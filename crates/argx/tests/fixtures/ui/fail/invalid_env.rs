#[derive(argx::Parser)]
struct BoolEnvironment {
    #[argx(long, env = "ARGX_ENABLED")]
    enabled: bool,
}

#[derive(argx::Parser)]
struct PositionalEnvironment {
    #[argx(env = "ARGX_VALUE")]
    value: u16,
}

#[derive(argx::Parser)]
struct CollectionEnvironment {
    #[argx(long, env = "ARGX_VALUES")]
    values: Vec<u16>,
}

#[derive(argx::Parser)]
struct DuplicateEnvironment {
    #[argx(long, env = "ARGX_ONE", env = "ARGX_TWO")]
    value: u16,
}

#[derive(argx::Parser)]
struct EmptyEnvironment {
    #[argx(long, env = "")]
    value: u16,
}

#[derive(argx::Parser)]
struct EqualsEnvironment {
    #[argx(long, env = "ARGX=VALUE")]
    value: u16,
}

#[derive(argx::Parser)]
struct MissingEnvironmentName {
    #[argx(long, env)]
    value: u16,
}

fn main() {}
