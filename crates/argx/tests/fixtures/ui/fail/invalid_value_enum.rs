#[derive(argx::ValueEnum)]
struct NotAnEnum;

#[derive(argx::ValueEnum)]
enum Generic<T> {
    Value(std::marker::PhantomData<T>),
}

#[derive(argx::ValueEnum)]
enum Empty {}

#[derive(argx::ValueEnum)]
enum Payload {
    Value(String),
}

#[allow(non_camel_case_types)]
#[derive(argx::ValueEnum)]
enum Duplicate {
    Foo,
    foo,
}

#[derive(argx::Parser)]
struct Switch {
    #[argx(long, value_enum)]
    enabled: bool,
}

#[derive(argx::Parser)]
struct GenericField<T> {
    #[argx(value_enum)]
    value: T,
}

#[derive(argx::ValueEnum)]
enum Valid {
    Value,
}

#[derive(argx::Parser)]
struct InvalidAttributeSyntax {
    #[argx(value_enum = true)]
    value: Valid,
}

fn main() {}
