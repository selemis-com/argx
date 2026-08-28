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
fn handler() -> Result<Output, Error> {
    Ok(Output { value: String::from("ok") })
}

fn main() {
    let _ = <Command as argx::HandlerSchemaSource>::schemas();
    let Ok(output) = handler() else { panic!("handler failed") };
    assert_eq!(output.value, "ok");
    let _ = Error::Failed;
}
