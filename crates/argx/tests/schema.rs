//! Machine-readable schema and handler-association integration tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    #![expect(dead_code, reason = "schema fixtures are exercised through generated metadata")]

    use argx::{Parser as _, argx};
    use serde_json::{Map, Value};

    #[derive(argx::Args)]
    struct GetCommand {
        /// Object identifier.
        id: String,
    }

    #[argx(schema)]
    struct GetOutput {
        /// Returned object identifier.
        id: String,
        /// Additional object details.
        detail: GetDetail,
    }

    #[argx(schema)]
    struct GetDetail {
        /// Labels attached to the object.
        labels: Vec<String>,
    }

    #[argx(schema)]
    enum GetError {
        Missing,
    }

    #[argx(handler = GetCommand)]
    fn get(command: GetCommand) -> Result<GetOutput, GetError> {
        Ok(GetOutput { id: command.id, detail: GetDetail { labels: Vec::new() } })
    }

    #[derive(argx::Args)]
    #[argx(schema)]
    struct AdminCommand {
        #[argx(subcommand)]
        command: AdminCommands,
    }

    #[derive(argx::Subcommand)]
    #[argx(schema)]
    enum AdminCommands {
        /// Inspect service status.
        Status(StatusCommand),
    }

    #[derive(argx::Args)]
    struct StatusCommand;

    #[argx(schema)]
    struct StatusOutput {
        healthy: bool,
    }

    #[argx(schema)]
    enum StatusError {
        Unavailable,
    }

    #[argx(handler = run)]
    impl StatusCommand {
        const fn run(self) -> Result<StatusOutput, StatusError> {
            Ok(StatusOutput { healthy: true })
        }
    }

    #[derive(argx::Subcommand)]
    #[argx(schema)]
    enum Commands {
        /// Retrieve one object.
        Get(GetCommand),
        /// Administrative commands.
        Admin(AdminCommand),
    }

    #[derive(argx::Parser)]
    #[argx(name = "tool", schema)]
    struct Cli {
        /// Profile inherited by every child command.
        #[argx(long, global)]
        profile: Option<String>,
        #[argx(subcommand)]
        command: Commands,
    }

    #[derive(argx::Parser)]
    #[argx(name = "echo", schema)]
    struct Direct {
        value: String,
    }

    #[argx(schema)]
    struct DirectOutput {
        value: String,
    }

    #[argx(schema)]
    enum DirectError {
        Failed,
    }

    #[argx(handler = Direct)]
    fn direct(command: Direct) -> Result<DirectOutput, DirectError> {
        Ok(DirectOutput { value: command.value })
    }

    fn discovered<I, T>(argv: I) -> Value
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString>,
    {
        let error = match Cli::try_parse_args(argv) {
            Err(error) => error,
            Ok(_) => panic!("schema request should be terminal"),
        };
        let argx::Error::DisplaySchema { schema } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        serde_json::from_str(&schema).expect("schema discovery must emit JSON")
    }

    fn schema_keyword_count(value: &Value) -> usize {
        match value {
            Value::Object(object) => {
                usize::from(object.contains_key("$schema"))
                    + object.values().map(schema_keyword_count).sum::<usize>()
            }
            Value::Array(values) => values.iter().map(schema_keyword_count).sum(),
            _ => 0,
        }
    }

    #[test]
    fn pseudo_command_and_selected_schema_action_are_equivalent() {
        let by_command = discovered(["schema", "get"]);
        let by_long = discovered(["get", "example", "--schema"]);
        let by_short = discovered(["get", "example", "-S"]);

        assert_eq!(by_command, by_long);
        assert_eq!(by_command, by_short);
        assert_eq!(by_command["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(schema_keyword_count(&by_command), 1);
        assert_eq!(by_command["title"], "get");
        assert_eq!(by_command["description"], "Retrieve one object.");
        assert_eq!(by_command["properties"]["id"]["type"], "string");
        assert_eq!(by_command["properties"]["--profile"]["type"], "string");
        assert_eq!(by_command["$defs"]["result"]["$ref"], "#/$defs/types/$defs/GetOutput");
        assert_eq!(by_command["$defs"]["error"]["$ref"], "#/$defs/types/$defs/GetError");
        assert!(by_command["$defs"]["types"]["$defs"].get("GetOutput").is_some());
        assert!(by_command["$defs"]["types"]["$defs"].get("GetError").is_some());
    }

    #[test]
    fn full_schema_expands_handler_types_from_both_discovery_forms() {
        let by_command = discovered(["schema", "get", "--full"]);
        let by_action = discovered(["get", "example", "-S", "--full"]);

        assert_eq!(by_command, by_action);
        assert_eq!(by_command["$defs"]["result"]["$ref"], "#/$defs/types/$defs/GetOutput");
        assert_eq!(by_command["$defs"]["error"]["$ref"], "#/$defs/types/$defs/GetError");
        assert_eq!(
            by_command["$defs"]["types"]["$defs"]["GetOutput"]["properties"]["detail"]["$ref"],
            "#/$defs/types/$defs/GetDetail",
        );
        assert_eq!(
            by_command["$defs"]["types"]["$defs"]["GetDetail"]["properties"]["labels"]["type"],
            "array",
        );
    }

    #[test]
    fn structural_paths_are_shallow_by_default_and_recursive_when_full() {
        let root = discovered(["schema"]);
        assert_eq!(root["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(schema_keyword_count(&root), 1);
        assert_eq!(root["title"], "tool");
        assert!(root["$defs"].get("result").is_none());
        assert_eq!(root["$defs"]["subcommands"]["$defs"].as_object().map(Map::len), Some(2));

        let get = &root["$defs"]["subcommands"]["$defs"]["get"];
        assert_eq!(get["title"], "get");
        assert_eq!(get["description"], "Retrieve one object.");
        assert!(get.get("$defs").is_none());

        let admin = discovered(["schema", "admin"]);
        assert_eq!(admin["title"], "admin");
        assert!(admin["$defs"].get("result").is_none());
        let status = &admin["$defs"]["subcommands"]["$defs"]["status"];
        assert_eq!(status["title"], "status");
        assert!(status.get("$defs").is_none());

        let status = discovered(["schema", "admin", "status"]);
        assert_eq!(
            status["$defs"]["types"]["$defs"]["StatusOutput"]["properties"]["healthy"]["type"],
            "boolean",
        );

        let full = discovered(["schema", "--full"]);
        let get = &full["$defs"]["subcommands"]["$defs"]["get"];
        assert_eq!(
            get["$defs"]["result"]["$ref"],
            "#/$defs/subcommands/$defs/get/$defs/types/$defs/GetOutput",
        );
        assert_eq!(
            full["$defs"]["subcommands"]["$defs"]["admin"]["$defs"]["subcommands"]["$defs"]["status"]
                ["$defs"]["types"]["$defs"]["StatusOutput"]["properties"]["healthy"]["type"],
            "boolean",
        );
    }

    #[test]
    fn schema_enabled_help_advertises_the_virtual_action() {
        assert!(Cli::render_help().contains("-S, --schema"));
        let error = match Cli::try_parse_args(["get", "--help"]) {
            Err(error) => error,
            Ok(_) => panic!("help should be terminal"),
        };
        let argx::Error::DisplayHelp { help } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        assert!(help.contains("-S, --schema"));
    }

    #[test]
    fn direct_root_schema_keeps_schema_available_as_a_positional_value() {
        let direct = Direct::try_parse_args(["schema"]).expect("schema is positional data here");
        assert_eq!(direct.value, "schema");

        let error = match Direct::try_parse_args(["-S"]) {
            Err(error) => error,
            Ok(_) => panic!("schema action should be terminal"),
        };
        let argx::Error::DisplaySchema { schema } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        let document: Value = serde_json::from_str(&schema).expect("schema must be JSON");
        assert_eq!(document["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(document["title"], "echo");
        assert_eq!(document["$defs"]["result"]["title"], "DirectOutput");
        assert_eq!(document["$defs"]["error"]["title"], "DirectError");
        assert!(document["$defs"].get("types").is_some());

        let error = match Direct::try_parse_args(["-S", "--full"]) {
            Err(error) => error,
            Ok(_) => panic!("full schema action should be terminal"),
        };
        let argx::Error::DisplaySchema { schema } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        let document: Value = serde_json::from_str(&schema).expect("schema must be JSON");
        assert_eq!(document["$defs"]["result"]["title"], "DirectOutput");
        assert_eq!(document["$defs"]["error"]["title"], "DirectError");
        assert!(document["$defs"].get("types").is_some());
    }

    #[derive(argx::Args)]
    struct HandlerArgs {
        value: String,
    }

    #[argx(schema)]
    struct HandlerOutput {
        value: String,
    }

    #[argx(schema)]
    #[derive(Debug)]
    enum HandlerError {
        Failed,
    }

    #[argx(handler = HandlerArgs)]
    fn handler(args: HandlerArgs) -> Result<HandlerOutput, HandlerError> {
        if args.value.is_empty() {
            Err(HandlerError::Failed)
        } else {
            Ok(HandlerOutput { value: args.value })
        }
    }

    #[test]
    fn handlers_associate_invocation_result_and_error_schemas() {
        let mut generator = schemars::SchemaGenerator::default();
        let schemas = <HandlerArgs as argx::HandlerSchemaSource>::handler_schemas(&mut generator);
        let result = serde_json::to_value(&schemas.result).expect("result schema should serialize");
        let error = serde_json::to_value(&schemas.error).expect("error schema should serialize");

        assert_eq!(result["$ref"], "#/$defs/HandlerOutput");
        assert_eq!(error["$ref"], "#/$defs/HandlerError");
        assert!(generator.definitions().contains_key("HandlerOutput"));
        assert!(generator.definitions().contains_key("HandlerError"));
        let result =
            handler(HandlerArgs { value: String::from("ok") }).expect("handler should succeed");
        assert_eq!(result.value, "ok");
    }
}
