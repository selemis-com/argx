#[derive(argx::Parser)]
struct PositionalCount {
    #[argx(count)]
    verbosity: u8,
}

#[derive(argx::Parser)]
struct WrongCountType {
    #[argx(short, count)]
    verbosity: i32,
}

#[derive(argx::Parser)]
struct CountValuePolicy {
    #[argx(short, count, allow_negative_numbers)]
    verbosity: u8,
}

#[derive(argx::Parser)]
struct CountValueEnum {
    #[argx(short, count, value_enum)]
    verbosity: u8,
}

#[derive(argx::Parser)]
struct CountTakesNoValue {
    #[argx(short, count = true)]
    verbosity: u8,
}

fn main() {}
