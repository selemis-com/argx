#![allow(dead_code)]

use argx::Parser as _;

struct RuntimeContext;

#[derive(argx::Contract)]
struct Output;

#[derive(argx::Contract)]
enum Failure {
    Failed,
}

type HandlerResult = Result<Output, Failure>;

#[derive(argx::Parser)]
struct SyncCommand;

#[argx::contract(SyncCommand)]
fn sync_handler(_command: SyncCommand, _context: &RuntimeContext) -> HandlerResult {
    Ok(Output)
}

#[derive(argx::Parser)]
struct AsyncCommand;

#[argx::contract(AsyncCommand)]
async fn async_handler(_context: &RuntimeContext) -> Result<(), Failure> {
    Ok(())
}

#[derive(argx::Parser)]
struct ConstCommand;

#[argx::contract(ConstCommand)]
const fn const_handler() -> Result<(), ()> {
    Ok(())
}

#[derive(argx::Args)]
struct ConditionalCommand;

#[argx::contract(ConditionalCommand)]
#[cfg(any())]
fn disabled_handler() -> Result<(), ()> {
    Ok(())
}

#[argx::contract(ConditionalCommand)]
fn enabled_handler() -> Result<(), ()> {
    Ok(())
}

fn main() {
    let _ = SyncCommand::contract(argx::ContractRequest::root());
    let _ = AsyncCommand::contract(argx::ContractRequest::root());
    let _ = ConstCommand::contract(argx::ContractRequest::root());
}
