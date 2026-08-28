//! Machine-readable schema discovery integration tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    #![expect(dead_code, reason = "schema fixtures are exercised through generated metadata")]

    use argx::{Parser as _, argx};
    use serde_json::Value;

    #[derive(argx::Args)]
    struct GetCommand {
        /// Object identifier.
        id: String,
    }

    #[argx(schema)]
    struct GetOutput {
        /// Returned object identifier.
        id: String,
    }

    #[argx(schema)]
    enum GetError {
        Missing,
    }

    #[argx(handler = GetCommand)]
    fn get(command: GetCommand) -> Result<GetOutput, GetError> {
        Ok(GetOutput { id: command.id })
    }

    #[derive(argx::Args, argx::CommandSchema)]
    struct AdminCommand {
        #[argx(subcommand)]
        command: AdminCommands,
    }

    #[derive(argx::Subcommand, argx::CommandSchema)]
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

    #[derive(argx::Subcommand, argx::CommandSchema)]
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

    #[test]
    fn pseudo_command_and_selected_schema_action_are_equivalent() {
        let by_command = discovered(["schema", "get"]);
        let by_flag = discovered(["get", "example", "--schema"]);

        assert_eq!(by_command, by_flag);
        assert_eq!(by_command["command"]["path"], serde_json::json!(["get"]));
        assert_eq!(by_command["command"]["invocable"], true);
        assert_eq!(by_command["invocation"]["properties"]["id"]["type"], "string");
        assert_eq!(by_command["invocation"]["properties"]["--profile"]["type"], "string",);
        assert_eq!(by_command["result"]["properties"]["id"]["type"], "string");
        assert!(by_command["error"].is_object());
    }

    #[test]
    fn structural_paths_expose_immediate_children_without_fake_handler_schemas() {
        let root = discovered(["schema"]);
        assert_eq!(root["command"]["path"], serde_json::json!([]));
        assert_eq!(root["command"]["invocable"], false);
        assert!(root.get("result").is_none());
        assert_eq!(root["subcommands"].as_array().map(Vec::len), Some(2));

        let admin = discovered(["schema", "admin"]);
        assert_eq!(admin["command"]["path"], serde_json::json!(["admin"]));
        assert_eq!(admin["command"]["invocable"], false);
        assert_eq!(admin["subcommands"][0]["name"], "status");
        assert_eq!(admin["subcommands"][0]["invocable"], true);

        let status = discovered(["schema", "admin", "status"]);
        assert_eq!(status["command"]["invocable"], true);
        assert_eq!(status["result"]["properties"]["healthy"]["type"], "boolean");
    }

    #[test]
    fn schema_enabled_help_advertises_the_virtual_action() {
        assert!(Cli::render_help().contains("--schema"));
        let error = match Cli::try_parse_args(["get", "--help"]) {
            Err(error) => error,
            Ok(_) => panic!("help should be terminal"),
        };
        let argx::Error::DisplayHelp { help } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        assert!(help.contains("--schema"));
    }

    #[test]
    fn direct_root_schema_keeps_schema_available_as_a_positional_value() {
        let direct = Direct::try_parse_args(["schema"]).expect("schema is positional data here");
        assert_eq!(direct.value, "schema");

        let error = match Direct::try_parse_args(["--schema"]) {
            Err(error) => error,
            Ok(_) => panic!("schema action should be terminal"),
        };
        let argx::Error::DisplaySchema { schema } = error else {
            panic!("unexpected parser result: {error:?}");
        };
        let document: Value = serde_json::from_str(&schema).expect("schema must be JSON");
        assert_eq!(document["command"]["path"], serde_json::json!([]));
        assert_eq!(document["command"]["invocable"], true);
    }
}
