#[derive(argx::Args)]
struct Command;

#[argx::handler(Command)]
fn first() -> Result<(), ()> {
    Ok(())
}

#[argx::handler(Command)]
fn second() -> Result<(), ()> {
    Ok(())
}

fn main() {}
