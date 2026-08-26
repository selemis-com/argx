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

#[derive(argx::Contract)]
struct GenericOutput<T> {
    value: T,
}

#[derive(argx::Parser)]
struct GenericCommand<T> {
    value: T,
}

#[argx::contract(GenericCommand<u16>)]
fn generic_u16_handler(_command: GenericCommand<u16>) -> Result<GenericOutput<u16>, ()> {
    Ok(GenericOutput { value: 0 })
}

#[argx::contract(GenericCommand<u32>)]
fn generic_u32_handler(_command: GenericCommand<u32>) -> Result<GenericOutput<u32>, ()> {
    Ok(GenericOutput { value: 0 })
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
    let _ = GenericCommand::<u16>::contract(argx::ContractRequest::root());
    let _ = GenericCommand::<u32>::contract(argx::ContractRequest::root());
}
