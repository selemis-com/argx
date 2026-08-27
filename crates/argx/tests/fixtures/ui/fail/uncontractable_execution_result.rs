#[derive(argx::Args)]
struct Command;
struct Output;

#[argx::contract(Command)]
fn handler() -> Result<Output, ()> {
    Ok(Output)
}

fn main() {}
