#[derive(argx::Parser)]
struct RequiredDefaultTypeMismatch {
    #[argx(long, default = "3000")]
    port: u16,
}

#[derive(argx::Parser)]
struct OptionalDefaultTypeMismatch {
    #[argx(long, default = Some(3000_u16))]
    port: Option<u16>,
}

fn main() {}
