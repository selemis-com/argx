struct MissingType;

#[argx::contract]
fn missing_type() -> Result<(), ()> {
    Ok(())
}

struct GenericCommand;

#[argx::contract(GenericCommand)]
fn generic_handler<T>() -> Result<(), ()> {
    Ok(())
}

struct MissingResult;

#[argx::contract(MissingResult)]
fn missing_result() {}

struct OpaqueResult;

#[argx::contract(OpaqueResult)]
fn opaque_result() -> impl Sized {
    Ok::<(), ()>(())
}

struct UnknownAttribute;

#[argx::contract(UnknownAttribute, success = ())]
fn unknown_attribute() -> Result<(), ()> {
    Ok(())
}

struct NotAFunction;

#[argx::contract(NotAFunction)]
struct InvalidTarget;

fn main() {}
