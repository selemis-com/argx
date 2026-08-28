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
        let mut generator = schemars::SchemaGenerator::default();
        let schemas = <RunArgs as argx::HandlerSchemaSource>::handler_schemas(&mut generator);
        let result = serde_json::to_value(&schemas.result).expect("result schema should serialize");
        let error = serde_json::to_value(&schemas.error).expect("error schema should serialize");

        assert_eq!(result["$ref"], "#/$defs/Output");
        assert_eq!(error["$ref"], "#/$defs/RunError");
        assert!(generator.definitions().contains_key("Output"));
        assert!(generator.definitions().contains_key("RunError"));
        let result = run(RunArgs { value: String::from("ok") }).expect("handler should succeed");
        assert_eq!(result.value, "ok");
    }
}
