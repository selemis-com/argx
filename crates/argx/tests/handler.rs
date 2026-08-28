//! Handler schema association tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {

    use argx::argx;
    #[derive(argx::Args)]
    struct RunArgs {
        value: String,
    }

    #[argx(schema)]
    struct Output {
        value: String,
    }

    #[argx(schema)]
    #[derive(Debug)]
    enum RunError {
        Failed,
    }

    #[argx(handler = RunArgs)]
    fn run(args: RunArgs) -> Result<Output, RunError> {
        if args.value.is_empty() { Err(RunError::Failed) } else { Ok(Output { value: args.value }) }
    }

    #[test]
    fn handlers_associate_invocation_result_and_error_schemas() {
        let schemas = <RunArgs as argx::HandlerSchemaSource>::schemas();
        let invocation =
            serde_json::to_value(&schemas.invocation).expect("invocation schema should serialize");
        let result = serde_json::to_value(&schemas.result).expect("result schema should serialize");
        let error = serde_json::to_value(&schemas.error).expect("error schema should serialize");

        assert_eq!(invocation["properties"]["value"]["type"], "string");
        assert!(result["properties"]["value"].is_object());
        assert!(error.is_object());
        let result = run(RunArgs { value: String::from("ok") }).expect("handler should succeed");
        assert_eq!(result.value, "ok");
    }
}
