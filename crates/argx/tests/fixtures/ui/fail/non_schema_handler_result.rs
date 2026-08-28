#[derive(argx::Args)]
struct Command;
struct Output;

#[argx::handler(Command)]
fn handler() -> Result<Output, ()> {
    Ok(Output)
}

fn main() {}
