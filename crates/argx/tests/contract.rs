//! Native machine-contract discovery tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use argx::{ConstraintContractKind, ContractDepth, ContractRequest, Parser as _};

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

    /// Small CLI used to lock down repeated named-option value semantics.
    #[derive(argx::Parser)]
    #[argx(name = "repeat")]
    struct RepeatedOptionCli {
        #[argx(long)]
        tag: Vec<String>,
    }

    /// Covers contract projections whose preferred names are not long flags.
    #[derive(argx::Parser)]
    #[argx(name = "projection")]
    struct ProjectionCli {
        #[argx(short = 't')]
        token: String,
        #[argx(requires = "token")]
        input: Option<String>,
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
    fn repeated_named_options_describe_occurrence_and_value_multiplicity_separately() {
        let cli = RepeatedOptionCli::try_parse_from(["repeat", "--tag", "one", "--tag", "two"])
            .expect("repeated option fixture should parse");
        assert_eq!(cli.tag, ["one", "two"]);

        let contract = RepeatedOptionCli::contract(ContractRequest::root())
            .expect("repeated option contract should exist");
        let invocation =
            contract.command.invocation.as_ref().expect("root command should be detailed");
        let option = &invocation.contexts[0].options[0];

        assert_eq!(option.name, "--tag");
        assert!(option.repeatable);
        assert_eq!(option.value.expect("tag takes a value").min_values, 1);
        assert_eq!(option.value.expect("tag takes a value").max_values, Some(1));
    }

    #[test]
    fn contract_projects_short_only_options_optional_positionals_and_positional_constraints() {
        let parsed = ProjectionCli::try_parse_from(["projection", "-t", "secret"])
            .expect("projection fixture should remain parseable");
        assert_eq!(parsed.token, "secret");
        assert_eq!(parsed.input, None);

        let request = ContractRequest::root();
        assert!(request.path().is_empty());
        assert_eq!(request.depth(), ContractDepth::Shallow);

        let contract = ProjectionCli::contract(request).expect("root contract should exist");
        let invocation =
            contract.command.invocation.as_ref().expect("root command should be detailed");
        let context = &invocation.contexts[0];

        assert_eq!(context.options[0].name, "-t");
        assert!(context.options[0].required);
        assert_eq!(context.arguments[0].name, "input");
        assert!(!context.arguments[0].required);
        assert_eq!(context.arguments[0].value.min_values, 0);
        assert_eq!(context.arguments[0].value.max_values, Some(1));
        assert_eq!(context.constraints[0].source, "input");
        assert_eq!(context.constraints[0].target, "-t");

        let json = contract.to_json().expect("compact contract JSON should serialize");
        assert!(!json.contains('\n'));
        assert!(json.contains(r#""name":"-t""#));
    }

    #[test]
    fn shallow_discovery_returns_selected_detail_and_direct_child_summaries() {
        let contract = Cli::contract(ContractRequest::root()).expect("root contract should exist");

        assert_eq!(contract.version, argx::CONTRACT_VERSION);
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

        let root = &invocation.contexts[0];
        assert!(root.arguments.is_empty());
        assert_eq!(root.options.len(), 3);
        assert_eq!(root.options[0].name, "--config");
        assert!(root.options[0].aliases.iter().map(String::as_str).eq(["--cfg"]));
        assert!(root.options[0].global);
        assert!(!root.options[0].required);
        assert_eq!(root.options[0].value.expect("config takes a value").min_values, 1);
        assert_eq!(root.options[0].value.expect("config takes a value").max_values, Some(1));
        assert!(!root.options[0].repeatable);

        assert_eq!(root.options[1].name, "--profile");
        assert_eq!(root.options[1].environment.as_deref(), Some("TOOL_PROFILE"));
        assert!(root.options[1].has_default);
        assert!(!root.options[1].required);

        assert_eq!(root.options[2].name, "--verbose");
        assert!(root.options[2].aliases.iter().map(String::as_str).eq(["-v"]));
        assert!(root.options[2].value.is_none());

        let leaf = &invocation.contexts[2];
        assert_eq!(leaf.options.len(), 3);
        assert_eq!(leaf.options[0].name, "--endpoint");
        assert_eq!(leaf.options[1].name, "--auth-token");
        assert_eq!(leaf.options[1].environment.as_deref(), Some("TOOL_TOKEN"));
        assert!(leaf.options[1].required);
        assert!(leaf.options[1].aliases.iter().map(String::as_str).eq(["--token"]));
        assert_eq!(leaf.options[2].name, "--stdout");
        assert!(leaf.options[2].value.is_none());

        assert_eq!(leaf.arguments.len(), 2);
        assert_eq!(leaf.arguments[0].name, "id");
        assert_eq!(leaf.arguments[0].position, 1);
        assert_eq!(leaf.arguments[0].value.min_values, 1);
        assert_eq!(leaf.arguments[0].value.max_values, Some(1));
        assert!(leaf.arguments[0].required);
        assert_eq!(leaf.arguments[1].name, "selectors");
        assert_eq!(leaf.arguments[1].position, 2);
        assert_eq!(leaf.arguments[1].value.min_values, 0);
        assert!(leaf.arguments[1].value.max_values.is_none());

        assert_eq!(leaf.constraints.len(), 2);
        assert_eq!(leaf.constraints[0].kind, ConstraintContractKind::Requires);
        assert_eq!(leaf.constraints[0].source, "--endpoint");
        assert_eq!(leaf.constraints[0].target, "--auth-token");
        assert_eq!(leaf.constraints[1].kind, ConstraintContractKind::Conflicts);
        assert_eq!(leaf.constraints[1].source, "--endpoint");
        assert_eq!(leaf.constraints[1].target, "--stdout");
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
    fn nested_json_wire_shape_preserves_complete_invocation_contexts() {
        let contract = Cli::contract(ContractRequest::new(["obj", "show"]))
            .expect("nested aliases should resolve during contract lookup");
        let json =
            contract.to_json_pretty().expect("nested Argx contract should serialize as JSON");

        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            json,
            snapbox::str![[r#"
{
  "version": 1,
  "root": "tool",
  "command": {
    "path": [
      "objects",
      "get"
    ],
    "name": "get",
    "about": "Retrieve one object.",
    "aliases": [
      "show"
    ],
    "invocable": true,
    "invocation": {
      "contexts": [
        {
          "path": [],
          "options": [
            {
              "name": "--config",
              "aliases": [
                "--cfg"
              ],
              "help": "Configuration file.",
              "global": true,
              "required": false,
              "value": {
                "minValues": 1,
                "maxValues": 1
              },
              "repeatable": false,
              "hasDefault": false,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            },
            {
              "name": "--profile",
              "help": "Execution profile.",
              "global": false,
              "required": false,
              "value": {
                "minValues": 1,
                "maxValues": 1
              },
              "repeatable": false,
              "environment": "TOOL_PROFILE",
              "hasDefault": true,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            },
            {
              "name": "--verbose",
              "aliases": [
                "-v"
              ],
              "help": "Enable verbose output.",
              "global": false,
              "required": false,
              "repeatable": false,
              "hasDefault": false,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            }
          ]
        },
        {
          "path": [
            "objects"
          ]
        },
        {
          "path": [
            "objects",
            "get"
          ],
          "arguments": [
            {
              "name": "id",
              "help": "Object identifier.",
              "position": 1,
              "required": true,
              "value": {
                "minValues": 1,
                "maxValues": 1
              },
              "allowNegativeNumbers": false
            },
            {
              "name": "selectors",
              "help": "Additional selectors.",
              "position": 2,
              "required": false,
              "value": {
                "minValues": 0
              },
              "allowNegativeNumbers": false
            }
          ],
          "options": [
            {
              "name": "--endpoint",
              "help": "Optional remote endpoint.",
              "global": false,
              "required": false,
              "value": {
                "minValues": 1,
                "maxValues": 1
              },
              "repeatable": false,
              "hasDefault": false,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            },
            {
              "name": "--auth-token",
              "aliases": [
                "--token"
              ],
              "help": "Authentication token.",
              "global": false,
              "required": true,
              "value": {
                "minValues": 1,
                "maxValues": 1
              },
              "repeatable": false,
              "environment": "TOOL_TOKEN",
              "hasDefault": false,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            },
            {
              "name": "--stdout",
              "help": "Write the result to standard output.",
              "global": false,
              "required": false,
              "repeatable": false,
              "hasDefault": false,
              "allowHyphenValues": false,
              "allowNegativeNumbers": false
            }
          ],
          "constraints": [
            {
              "kind": "requires",
              "source": "--endpoint",
              "target": "--auth-token"
            },
            {
              "kind": "conflicts",
              "source": "--endpoint",
              "target": "--stdout"
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

    #[test]
    fn unknown_dynamic_paths_fail_without_guessing() {
        let error = Cli::contract(ContractRequest::new(["objects", "missing"]))
            .expect_err("unknown command path should fail");

        assert_eq!(error.to_string(), "unknown contract command `missing` below `objects`");
    }

    #[test]
    fn unknown_root_command_paths_report_without_a_parent_path() {
        let request = ContractRequest::new(["missing"]);
        assert!(request.path().iter().map(String::as_str).eq(["missing"]));

        let error = Cli::contract(request).expect_err("unknown root command should fail");
        assert_eq!(error.to_string(), "unknown contract command `missing`");
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
  "version": 1,
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
              "name": "value",
              "position": 1,
              "required": true,
              "value": {
                "minValues": 1,
                "maxValues": 1
              },
              "allowNegativeNumbers": false
            }
          ],
          "options": [
            {
              "name": "--output",
              "aliases": [
                "--out"
              ],
              "global": false,
              "required": false,
              "value": {
                "minValues": 1,
                "maxValues": 1
              },
              "repeatable": false,
              "environment": "ECHO_OUTPUT",
              "hasDefault": true,
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
