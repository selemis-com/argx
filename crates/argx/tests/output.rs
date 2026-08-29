//! Built-in output format and field-selection integration tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use argx::{Parser as _, argx};
    use serde::Serialize;
    use serde_json::json;

    #[derive(Debug, argx::Parser)]
    #[argx(schema)]
    struct Cli {
        #[argx(subcommand)]
        command: Command,
    }

    #[derive(Debug, argx::Subcommand)]
    #[argx(schema)]
    enum Command {
        Get(Get),
    }

    #[derive(Debug, argx::Args)]
    struct Get;

    #[derive(Debug, Serialize)]
    #[argx(schema)]
    struct Item {
        id: u64,
        name: String,
        nested: Nested,
    }

    #[derive(Debug, Serialize)]
    #[argx(schema)]
    struct Nested {
        value: String,
        ignored: bool,
    }

    #[derive(Debug, Serialize)]
    #[argx(schema)]
    struct HandlerError {
        message: String,
    }

    #[argx::argx(handler = run)]
    impl Get {
        fn run(self) -> Result<Item, HandlerError> {
            Ok(Item {
                id: 7,
                name: "example".to_owned(),
                nested: Nested { value: "kept".to_owned(), ignored: true },
            })
        }
    }

    #[test]
    fn output_and_fields_are_global_and_fields_are_comma_delimited_and_repeatable() {
        let invocation = Cli::try_parse_invocation_from([
            "tool",
            "get",
            "-O",
            "json",
            "-F",
            "id,nested.value",
            "--fields=name",
        ])
        .unwrap();

        assert_eq!(invocation.output.format(), argx::OutputFormat::Json);
        assert_eq!(invocation.output.fields(), ["id", "nested.value", "name"]);

        let Command::Get(command) = invocation.command.command;
        let value = command.run().unwrap();
        assert_eq!(
            invocation.output.value(&value).unwrap(),
            json!({"id": 7, "name": "example", "nested": {"value": "kept"}})
        );
    }

    #[test]
    fn output_defaults_to_text() {
        let invocation = Cli::try_parse_invocation_from(["tool", "get"]).unwrap();
        assert_eq!(invocation.output.format(), argx::OutputFormat::Text);
    }

    #[test]
    fn invalid_and_repeated_output_formats_are_rejected() {
        assert!(Cli::try_parse_invocation_from(["tool", "get", "-O", "yaml"]).is_err());
        let error =
            Cli::try_parse_invocation_from(["tool", "get", "--output", "json", "--output", "text"])
                .unwrap_err();
        assert!(matches!(error, argx::Error::DuplicateArgument { name } if name == "--output"));
    }

    #[test]
    fn fields_require_json_output() {
        let error = Cli::try_parse_invocation_from(["tool", "get", "-F", "id"]).unwrap_err();
        assert!(matches!(
            error,
            argx::Error::MissingRequirement { name, required_by }
                if name == "--output json" && required_by == "--fields"
        ));
    }

    #[test]
    fn invalid_field_is_rejected_before_handler_execution() {
        let error = Cli::try_parse_invocation_from(["tool", "get", "-O", "json", "-F", "missing"])
            .unwrap_err();
        assert!(matches!(error, argx::Error::InvalidOutputField { field } if field == "missing"));
    }

    #[test]
    fn built_in_output_options_appear_in_help() {
        let help = Cli::render_help();
        assert!(help.contains("-O, --output <FORMAT>"));
        assert!(help.contains("-F, --fields <FIELDS>"));
    }
}
