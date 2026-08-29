#[derive(argx::Parser)]
struct BoolDefault {
    #[argx(long, default = true)]
    enabled: bool,
}

#[derive(argx::Parser)]
struct PositionalDefault {
    #[argx(default = 1_u16)]
    value: u16,
}

#[derive(argx::Parser)]
struct CollectionDefault {
    #[argx(long, default = 1_u16)]
    values: Vec<u16>,
}

#[derive(argx::Parser)]
struct DuplicateDefault {
    #[argx(long, default = 1_u16, default = 2_u16)]
    value: u16,
}

fn main() {}
