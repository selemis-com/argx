#[derive(argx::Args)]
struct Command;

#[argx::contract(Command)]
fn first() -> Result<(), ()> {
    Ok(())
}

#[argx::contract(Command)]
fn second() -> Result<(), ()> {
    Ok(())
}

fn main() {}
