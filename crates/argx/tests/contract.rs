//! Native machine-contract discovery tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use argx::{
        ArgumentCardinality, ArgumentSyntax, ConstraintContractKind, ContractDepth,
        ContractRequest, Parser as _,
    };

    /// Reusable authentication arguments.
    #[derive(argx::Args)]
    struct AuthArgs {
        /// Authentication token.
        #[argx(long = "auth-token", alias = "token", env = "TOOL_TOKEN")]
        token: String,
    }

    /// Arguments for retrieving one object.
    #[derive(argx::Args)]
    struct GetArgs {
        /// Optional remote endpoint.
        #[argx(long, requires = "token", conflicts = "stdout")]
        endpoint: Option<String>,
        #[argx(flatten)]
        auth: AuthArgs,
        /// Write the result to standard output.
        #[argx(long)]
        stdout: bool,
        /// Object identifier.
        id: String,
        /// Additional selectors.
        selectors: Vec<String>,
    }

    /// Arguments for the object command namespace.
    #[derive(argx::Args)]
    struct ObjectsArgs {
        #[argx(subcommand)]
        command: ObjectCommands,
    }

    /// Commands operating on objects.
    #[derive(argx::Subcommand)]
    enum ObjectCommands {
        /// Retrieve one object.
        #[argx(alias = "show")]
        Get(GetArgs),
        /// List objects.
        List,
    }

    /// Top-level commands.
    #[derive(argx::Subcommand)]
    enum Commands {
        /// Object operations.
        #[argx(alias = "obj")]
        Objects(ObjectsArgs),
        /// Show service status.
        Status,
    }

    /// Representative nested CLI used by contract discovery tests.
    #[derive(argx::Parser)]
    #[argx(name = "tool", about = "Manage objects")]
    struct Cli {
        /// Configuration file.
        #[argx(long, alias = "cfg", global)]
        config: Option<String>,
        /// Execution profile.
        #[argx(long, env = "TOOL_PROFILE", default = String::from("default"))]
        profile: String,
        /// Enable verbose output.
        #[argx(short, long)]
        verbose: bool,
        #[argx(subcommand)]
        command: Commands,
    }

    /// Small root-only CLI used to lock down JSON protocol spelling.
    #[derive(argx::Parser)]
    #[argx(name = "echo", about = "Echo values")]
    struct JsonCli {
        #[argx(
            long,
            alias = "out",
            env = "ECHO_OUTPUT",
            default = String::from("stdout")
        )]
        output: String,
        value: String,
    }

    #[test]
    fn representative_contract_fixture_remains_parseable() {
        let cli = Cli::try_parse_from([
            "tool",
            "--config",
            "tool.toml",
            "--profile",
            "fixture",
            "--verbose",
            "objects",
            "get",
            "--endpoint",
            "https://example.test",
            "--auth-token",
            "secret",
            "object-1",
            "primary",
            "secondary",
        ])
        .expect("representative argv should parse");

        let Cli { config, profile, verbose, command } = cli;
        assert_eq!(config.as_deref(), Some("tool.toml"));
        assert_eq!(profile, "fixture");
        assert!(verbose);

        let Commands::Objects(ObjectsArgs { command }) = command else {
            panic!("expected objects command");
        };
        let ObjectCommands::Get(GetArgs { endpoint, auth, stdout, id, selectors }) = command else {
            panic!("expected get command");
        };
        let AuthArgs { token } = auth;

        assert_eq!(endpoint.as_deref(), Some("https://example.test"));
        assert_eq!(token, "secret");
        assert!(!stdout);
        assert_eq!(id, "object-1");
        assert_eq!(selectors, ["primary", "secondary"]);
    }

    #[test]
    fn json_contract_fixture_remains_parseable() {
        let cli = JsonCli::try_parse_from(["echo", "--output", "file", "value"])
            .expect("JSON contract fixture argv should parse");
        let JsonCli { output, value } = cli;

        assert_eq!(output, "file");
        assert_eq!(value, "value");
    }

    #[test]
    fn shallow_discovery_returns_selected_detail_and_direct_child_summaries() {
        let contract = Cli::contract(ContractRequest::root()).expect("root contract should exist");

        assert_eq!(contract.contract_version, argx::CONTRACT_VERSION);
        assert_eq!(contract.root, "tool");
        assert_eq!(contract.command.path, Vec::<String>::new());
        assert_eq!(contract.command.name, "tool");
        assert!(!contract.command.invocable);
        assert!(contract.command.invocation.is_some());
        assert_eq!(contract.command.subcommands.len(), 2);

        let objects = &contract.command.subcommands[0];
        assert!(objects.path.iter().map(String::as_str).eq(["objects"]));
        assert!(objects.aliases.iter().map(String::as_str).eq(["obj"]));
        assert!(!objects.invocable);
        assert!(objects.invocation.is_none());
        assert!(objects.subcommands.is_empty());

        let status = &contract.command.subcommands[1];
        assert!(status.path.iter().map(String::as_str).eq(["status"]));
        assert!(status.invocable);
        assert!(status.invocation.is_none());
    }

    #[test]
    fn alias_lookup_returns_canonical_path_and_complete_invocation_contexts() {
        let contract = Cli::contract(ContractRequest::new(["obj", "show"]))
            .expect("aliases should resolve during contract lookup");
        let command = &contract.command;

        assert!(command.path.iter().map(String::as_str).eq(["objects", "get"]));
        assert_eq!(command.name, "get");
        assert!(command.aliases.iter().map(String::as_str).eq(["show"]));
        assert!(command.invocable);

        let invocation = command.invocation.as_ref().expect("selected command must be detailed");
        assert_eq!(invocation.contexts.len(), 3);
        assert_eq!(invocation.contexts[0].path, Vec::<String>::new());
        assert!(invocation.contexts[1].path.iter().map(String::as_str).eq(["objects"]));
        assert!(invocation.contexts[2].path.iter().map(String::as_str).eq(["objects", "get"]));

        let root_arguments = &invocation.contexts[0].arguments;
        assert_eq!(root_arguments.len(), 3);
        assert_eq!(root_arguments[0].id, "flag:--config");
        assert_eq!(root_arguments[0].name, "config");
        assert!(matches!(
            &root_arguments[0].syntax,
            ArgumentSyntax::Named { aliases, global: true, .. }
                if aliases.iter().map(String::as_str).eq(["cfg"])
        ));
        assert_eq!(root_arguments[0].cardinality, ArgumentCardinality::Optional);
        assert!(!root_arguments[0].required);

        assert_eq!(root_arguments[1].id, "flag:--profile");
        assert_eq!(root_arguments[1].environment.as_deref(), Some("TOOL_PROFILE"));
        assert!(root_arguments[1].has_default);
        assert!(!root_arguments[1].required);

        let leaf = &invocation.contexts[2];
        assert_eq!(leaf.arguments.len(), 5);
        assert_eq!(leaf.arguments[0].id, "flag:--endpoint");
        assert_eq!(leaf.arguments[1].id, "flag:--auth-token");
        assert_eq!(leaf.arguments[1].environment.as_deref(), Some("TOOL_TOKEN"));
        assert!(leaf.arguments[1].required);
        assert!(matches!(
            &leaf.arguments[1].syntax,
            ArgumentSyntax::Named { aliases, .. }
                if aliases.iter().map(String::as_str).eq(["token"])
        ));
        assert_eq!(leaf.arguments[3].id, "positional:0");
        assert_eq!(leaf.arguments[3].name, "id");
        assert_eq!(leaf.arguments[3].cardinality, ArgumentCardinality::One);
        assert!(leaf.arguments[3].required);
        assert_eq!(leaf.arguments[4].id, "positional:1");
        assert_eq!(leaf.arguments[4].cardinality, ArgumentCardinality::Many);

        assert_eq!(leaf.constraints.len(), 2);
        assert_eq!(leaf.constraints[0].kind, ConstraintContractKind::Requires);
        assert_eq!(leaf.constraints[0].source, "flag:--endpoint");
        assert_eq!(leaf.constraints[0].target, "flag:--auth-token");
        assert_eq!(leaf.constraints[1].kind, ConstraintContractKind::Conflicts);
        assert_eq!(leaf.constraints[1].source, "flag:--endpoint");
        assert_eq!(leaf.constraints[1].target, "flag:--stdout");
    }

    #[test]
    fn recursive_discovery_expands_the_complete_selected_subtree() {
        let request = ContractRequest::root().recursive();
        assert_eq!(request.depth(), ContractDepth::Recursive);
        let contract = Cli::contract(request).expect("recursive root contract should exist");

        let objects = &contract.command.subcommands[0];
        assert!(objects.invocation.is_some());
        assert_eq!(objects.subcommands.len(), 2);
        assert!(objects.subcommands[0].invocation.is_some());
        assert!(objects.subcommands[1].invocation.is_some());
        assert!(objects.subcommands[0].path.iter().map(String::as_str).eq(["objects", "get"]));
        assert!(objects.subcommands[1].path.iter().map(String::as_str).eq(["objects", "list"]));
    }

    #[test]
    fn unknown_dynamic_paths_fail_without_guessing() {
        let error = Cli::contract(ContractRequest::new(["objects", "missing"]))
            .expect_err("unknown command path should fail");

        assert_eq!(error.to_string(), "unknown contract command `missing` below `objects`");
    }

    #[test]
    fn json_wire_shape_is_versioned_and_stable() {
        let contract =
            JsonCli::contract(ContractRequest::root()).expect("root contract should exist");
        let json = contract.to_json_pretty().expect("Argx contract should serialize as JSON");

        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            json,
            snapbox::str![[r#"
{
  "contractVersion": 1,
  "root": "echo",
  "command": {
    "path": [],
    "name": "echo",
    "about": "Echo values",
    "invocable": true,
    "invocation": {
      "contexts": [
        {
          "path": [],
          "arguments": [
            {
              "id": "flag:--output",
              "name": "output",
              "syntax": {
                "kind": "named",
                "longs": [
                  "output"
                ],
                "aliases": [
                  "out"
                ],
                "global": false
              },
              "cardinality": "one",
              "required": false,
              "environment": "ECHO_OUTPUT",
              "hasDefault": true,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            },
            {
              "id": "positional:0",
              "name": "value",
              "syntax": {
                "kind": "positional",
                "index": 0
              },
              "cardinality": "one",
              "required": true,
              "hasDefault": false,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            }
          ]
        }
      ]
    }
  }
}
"#]],
        );
    }
}
