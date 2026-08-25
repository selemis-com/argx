#[derive(argx::Parser)]
struct Child {
    #[argx(long)]
    value: bool,
}

#[derive(argx::Parser)]
struct Parent {
    #[argx(flatten)]
    child: Child,
}

fn main() {}
