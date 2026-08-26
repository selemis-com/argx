use argx::Parser as _;

#[derive(argx::Parser)]
struct Cli;

fn main() {
    let _ = Cli::contract(argx::ContractRequest::root());
}
