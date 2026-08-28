struct MissingType;

#[argx::handler]
fn missing_type() -> Result<(), ()> {
    Ok(())
}

struct GenericCommand;

#[argx::handler(GenericCommand)]
fn generic_handler<T>() -> Result<(), ()> {
    Ok(())
}

struct MissingResult;

#[argx::handler(MissingResult)]
fn missing_result() {}

struct OpaqueResult;

#[argx::handler(OpaqueResult)]
fn opaque_result() -> impl Sized {
    Ok::<(), ()>(())
}

struct UnsupportedArguments;

#[argx::handler(UnsupportedArguments, error = ())]
fn unsupported_arguments() -> Result<(), ()> {
    Ok(())
}

struct NotAFunction;

#[argx::handler(NotAFunction)]
struct InvalidTarget;

fn main() {}
