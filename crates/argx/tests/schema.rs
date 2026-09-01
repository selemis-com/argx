//! Machine-readable schema and handler-association integration tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    #![expect(dead_code, reason = "schema fixtures are exercised through generated metadata")]

    use argx::{Parser as _, Schema as _, argx};
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
        /// Select compact output.
        #[argx(long, global, conflicts = "expanded")]
        compact: bool,
        /// Select expanded output.
        #[argx(long, global)]
        expanded: bool,
        /// Enable verbose output for one profile.
        #[argx(long, global, requires = "profile")]
        verbose: bool,
        #[argx(subcommand)]
        command: Commands,
    }

    #[derive(argx::Args)]
    struct OutputFileArgs {
        #[argx(long = "file")]
        file: Option<std::path::PathBuf>,
        #[argx(long, requires = "file")]
        force: bool,
    }

    #[derive(argx::Args)]
    struct RelationshipBodyCommand {
        #[argx(flatten)]
        output: OutputFileArgs,
    }

    #[argx(schema)]
    struct RelationshipOutput;

    #[argx(schema)]
    enum RelationshipError {
        Failed,
    }

    #[argx(handler = RelationshipBodyCommand)]
    fn relationship_body(
        _command: RelationshipBodyCommand,
    ) -> Result<RelationshipOutput, RelationshipError> {
        Ok(RelationshipOutput)
    }

    #[derive(argx::Args)]
    #[argx(any_of = ["name", "description"])]
    struct PatchFields {
        #[argx(long)]
        name: Option<String>,
        #[argx(long)]
        description: Option<String>,
    }

    #[derive(argx::Args)]
    struct RelationshipUpdateCommand {
        #[argx(flatten)]
        patch: PatchFields,
        #[argx(long, conflicts = "description")]
        clear_description: bool,
    }

    #[argx(handler = RelationshipUpdateCommand)]
    fn relationship_update(
        _command: RelationshipUpdateCommand,
    ) -> Result<RelationshipOutput, RelationshipError> {
        Ok(RelationshipOutput)
    }

    #[derive(argx::Subcommand)]
    #[argx(schema)]
    enum RelationshipCommands {
        Body(RelationshipBodyCommand),
        Update(RelationshipUpdateCommand),
    }

    #[derive(argx::Parser)]
    #[argx(name = "relationships", schema)]
    struct RelationshipCli {
        #[argx(subcommand)]
        command: RelationshipCommands,
    }

    #[derive(argx::Parser)]
    #[argx(name = "echo", schema)]
    struct Direct {
        value: String,
    }

    #[derive(argx::Parser)]
    #[argx(name = "typed", schema)]
    struct TypedValues {
        count: i64,
        #[argx(long)]
        small: Option<i8>,
        #[argx(long)]
        offset: Option<u16>,
        #[argx(long)]
        wide: Option<u128>,
        #[argx(long)]
        pointer: Option<usize>,
        #[argx(long)]
        ratio: Option<f64>,
        #[argx(long)]
        enabled: Option<bool>,
    }

    #[argx(schema)]
    struct TypedOutput;

    #[argx(schema)]
    enum TypedError {
        Failed,
    }

    #[argx(handler = TypedValues)]
    fn typed(_command: TypedValues) -> Result<TypedOutput, TypedError> {
        Ok(TypedOutput)
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
        let argv = std::iter::once(std::ffi::OsString::from("argx-test"))
            .chain(argv.into_iter().map(Into::into));
        let error = match Cli::try_parse_from(argv) {
            Err(error) => error,
            Ok(_) => panic!("schema request should be terminal"),
        };
        let argx::Error::DisplaySchema { schema } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        serde_json::from_str(&schema).expect("schema discovery must emit JSON")
    }

    fn discovered_relationship<I, T>(argv: I) -> Value
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString>,
    {
        let argv = std::iter::once(std::ffi::OsString::from("argx-test"))
            .chain(argv.into_iter().map(Into::into));
        let error = match RelationshipCli::try_parse_from(argv) {
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
    fn invocation_schema_uses_semantic_primitive_types() {
        let error = match TypedValues::try_parse_from(["typed", "--schema"]) {
            Err(error) => error,
            Ok(_) => panic!("schema request should be terminal"),
        };
        let argx::Error::DisplaySchema { schema } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        let schema: Value = serde_json::from_str(&schema).expect("schema must be JSON");

        assert_eq!(schema["properties"]["count"]["type"], "integer");
        assert_eq!(schema["properties"]["count"]["minimum"], i64::MIN);
        assert_eq!(schema["properties"]["count"]["maximum"], i64::MAX);
        assert_eq!(schema["properties"]["--small"]["minimum"], i8::MIN);
        assert_eq!(schema["properties"]["--small"]["maximum"], i8::MAX);
        assert_eq!(schema["properties"]["--offset"]["minimum"], 0);
        assert_eq!(schema["properties"]["--offset"]["maximum"], u16::MAX);
        assert_eq!(schema["properties"]["--wide"]["minimum"], 0);
        assert!(schema["properties"]["--wide"].get("maximum").is_none());
        assert_eq!(schema["properties"]["--pointer"]["minimum"], 0);
        assert_eq!(schema["properties"]["--pointer"]["maximum"], usize::MAX);
        assert_eq!(schema["properties"]["--ratio"]["type"], "number");
        assert_eq!(schema["properties"]["--enabled"]["type"], "boolean");
    }

    #[test]
    fn schema_attribute_exposes_standalone_schema() {
        #[argx(schema)]
        struct Output {
            value: String,
        }

        let schema = Output::schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["value"]["type"], "string");
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
    fn selected_paths_preserve_visible_ancestor_constraints() {
        let get = discovered(["schema", "get"]);

        assert_eq!(get["properties"]["--compact"]["const"], true);
        assert_eq!(get["properties"]["--expanded"]["const"], true);
        assert_eq!(get["allOf"].as_array().map(Vec::len), Some(1));
        assert_eq!(get["dependentRequired"]["--verbose"], serde_json::json!(["--profile"]));
        assert_eq!(
            get["allOf"][0]["not"]["required"],
            serde_json::json!(["--compact", "--expanded"]),
        );

        let full = discovered(["schema", "--full"]);
        assert_eq!(full["allOf"].as_array().map(Vec::len), Some(1));
        assert!(full["$defs"]["commands"]["$defs"]["get"].get("allOf").is_none());
    }

    #[test]
    fn relationship_projection_survives_flattening_and_full_expansion() {
        let body = discovered_relationship(["schema", "body"]);
        assert_eq!(body["properties"]["--file"]["type"], "string");
        assert_eq!(body["dependentRequired"]["--force"], serde_json::json!(["--file"]),);

        let update = discovered_relationship(["schema", "update"]);
        assert_eq!(
            update["allOf"][0]["anyOf"],
            serde_json::json!([
                { "required": ["--name"] },
                { "required": ["--description"] },
            ]),
        );
        assert_eq!(
            update["allOf"][1]["not"]["required"],
            serde_json::json!(["--clear-description", "--description"]),
        );

        let full = discovered_relationship(["schema", "--full"]);
        let body = &full["$defs"]["commands"]["$defs"]["body"];
        assert_eq!(body["dependentRequired"]["--force"], serde_json::json!(["--file"]),);
        let update = &full["$defs"]["commands"]["$defs"]["update"];
        assert_eq!(
            update["allOf"][0]["anyOf"],
            serde_json::json!([
                { "required": ["--name"] },
                { "required": ["--description"] },
            ]),
        );
        assert_eq!(
            update["allOf"][1]["not"]["required"],
            serde_json::json!(["--clear-description", "--description"]),
        );
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
        assert_eq!(root["$defs"]["commands"]["$defs"].as_object().map(Map::len), Some(2));
        assert_eq!(root["properties"]["get"]["$ref"], "#/$defs/commands/$defs/get");
        assert_eq!(root["properties"]["admin"]["$ref"], "#/$defs/commands/$defs/admin");
        assert_eq!(root["oneOf"].as_array().map(Vec::len), Some(2));

        let get = &root["$defs"]["commands"]["$defs"]["get"];
        assert_eq!(get["title"], "get");
        assert_eq!(get["description"], "Retrieve one object.");
        assert_eq!(get["type"], "object");
        assert!(get.get("additionalProperties").is_none());
        assert!(get.get("$defs").is_none());

        let admin = discovered(["schema", "admin"]);
        assert_eq!(admin["title"], "admin");
        assert!(admin["$defs"].get("result").is_none());
        assert_eq!(admin["properties"]["status"]["$ref"], "#/$defs/commands/$defs/status",);
        assert!(
            admin["required"]
                .as_array()
                .is_some_and(|required| { required.iter().any(|property| property == "status") })
        );
        let status = &admin["$defs"]["commands"]["$defs"]["status"];
        assert_eq!(status["title"], "status");
        assert_eq!(status["type"], "object");
        assert!(status.get("additionalProperties").is_none());

        let status = discovered(["schema", "admin", "status"]);
        assert_eq!(status["properties"]["--profile"]["type"], "string");
        assert_eq!(
            status["$defs"]["types"]["$defs"]["StatusOutput"]["properties"]["healthy"]["type"],
            "boolean",
        );

        let full = discovered(["schema", "--full"]);
        assert_eq!(full["properties"]["--profile"]["type"], "string");
        let get = &full["$defs"]["commands"]["$defs"]["get"];
        assert!(get["properties"].get("--profile").is_none());
        assert_eq!(
            get["$defs"]["result"]["$ref"],
            "#/$defs/commands/$defs/get/$defs/types/$defs/GetOutput",
        );
        assert_eq!(
            full["$defs"]["commands"]["$defs"]["admin"]["properties"]["status"]["$ref"],
            "#/$defs/commands/$defs/admin/$defs/commands/$defs/status",
        );
        assert_eq!(
            full["$defs"]["commands"]["$defs"]["admin"]["$defs"]["commands"]["$defs"]["status"]["$defs"]
                ["types"]["$defs"]["StatusOutput"]["properties"]["healthy"]["type"],
            "boolean",
        );
    }

    #[test]
    fn schema_pseudo_command_supports_help() {
        for argv in
            [&["schema", "--help"][..], &["schema", "-h"][..], &["schema", "admin", "--help"][..]]
        {
            let error =
                match Cli::try_parse_from(std::iter::once("argx-test").chain(argv.iter().copied()))
                {
                    Err(error) => error,
                    Ok(_) => panic!("schema help should be terminal"),
                };
            let argx::Error::DisplayHelp { help } = error else {
                panic!("unexpected parser result: {error:?}");
            };
            assert!(help.contains("Print machine-readable schema"));
            assert!(help.contains("Usage: tool schema [COMMAND]... [--full]"));
            assert!(help.contains("[COMMAND]..."));
            assert!(help.contains("--full"));
            assert!(help.contains("-h, --help"));
        }
    }

    #[test]
    fn schema_enabled_help_advertises_the_virtual_action() {
        let root_help = match Cli::try_parse_from(["argx-test", "--help"]) {
            Err(argx::Error::DisplayHelp { help }) => help,
            Ok(_) => panic!("help should be terminal"),
            Err(error) => panic!("unexpected parser result: {error:?}"),
        };
        assert!(root_help.contains("-S, --schema"));
        let error = match Cli::try_parse_from(["argx-test", "get", "--help"]) {
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
        let direct = Direct::try_parse_from(["argx-test", "schema"])
            .expect("schema is positional data here");
        assert_eq!(direct.value, "schema");

        let error = match Direct::try_parse_from(["argx-test", "-S"]) {
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
        let schemas =
            <HandlerArgs as argx::__private::HandlerSchemaSource>::handler_schemas(&mut generator);
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
