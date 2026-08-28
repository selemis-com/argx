use argx::argx;

#[derive(argx::Args)]
struct Command;

#[argx(schema)]
struct Output {
    value: String,
}

#[argx(schema)]
enum Error {
    Failed,
}

#[argx(handler = Command)]
fn handler(_command: Command) -> Result<Output, Error> {
    Ok(Output { value: String::from("ok") })
}

fn main() {
    let Ok(output) = handler(Command) else { panic!("handler failed") };
    assert_eq!(output.value, "ok");
    let _ = Error::Failed;
}
