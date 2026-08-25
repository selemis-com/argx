#[derive(argx::Parser)]
struct NestedOption {
    value: Option<Option<String>>,
}

#[derive(argx::Parser)]
struct NestedVec {
    values: Vec<Vec<String>>,
}

fn main() {}
