//! Handler schema association tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    #[derive(argx::Args)]
    struct RunArgs {
        value: String,
    }

    #[argx::schema]
    struct Output {
        value: String,
    }

    #[argx::schema]
    #[derive(Debug)]
    enum RunError {
        Failed,
    }

    #[argx::handler(RunArgs)]
    fn run(args: RunArgs) -> Result<Output, RunError> {
        if args.value.is_empty() { Err(RunError::Failed) } else { Ok(Output { value: args.value }) }
    }

    #[test]
    fn schema_attribute_and_handlers_emit_schemars_schemas() {
        let schemas = <RunArgs as argx::HandlerSchemaSource>::schemas();
        let success =
            serde_json::to_value(&schemas.success).expect("success schema should serialize");
        let error = serde_json::to_value(&schemas.error).expect("error schema should serialize");

        assert!(success["properties"]["value"].is_object());
        assert!(error.is_object());
        let result = run(RunArgs { value: String::from("ok") }).expect("handler should succeed");
        assert_eq!(result.value, "ok");
    }
}
