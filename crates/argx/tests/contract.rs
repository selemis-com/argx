//! Native machine-contract discovery and wire-protocol tests.
//!
//! This layer owns the public projection produced by `Parser::contract`: canonical paths, aliases,
//! invocation contexts, cardinality, value sources, relationships, discovery depth, and serialized
//! protocol spelling. Parsing the representative fixture is useful only as a cross-check that the
//! declaration backing the contract remains invocable; detailed parser semantics live elsewhere.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    #![expect(dead_code, reason = "contract fixtures include metadata-only handler functions")]

    use argx::{
        ConstraintContractKind, ContractDepth, ContractRequest, Parser as _, PrimitiveType,
        TypeContractValue, TypeDefinitionKind,
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

    /// Successful result of retrieving one object.
    #[derive(argx::Contract)]
    struct GetOutput {
        id: String,
    }

    /// Retrieval failure exposed by the command.
    #[derive(argx::Contract)]
    enum GetError {
        NotFound,
    }

    /// Arguments for the object command namespace.
    #[derive(argx::Args)]
    struct ObjectsArgs {
        #[argx(subcommand)]
        command: ObjectCommands,
    }

    /// Arguments for listing objects.
    #[derive(argx::Args)]
    struct ListArgs {}

    /// Commands operating on objects.
    #[derive(argx::Subcommand)]
    enum ObjectCommands {
        /// Retrieve one object.
        #[argx(alias = "show")]
        Get(GetArgs),
        /// List objects.
        List(ListArgs),
    }

    /// Arguments for service status.
    #[derive(argx::Args)]
    struct StatusArgs {}

    /// Top-level commands.
    #[derive(argx::Subcommand)]
    enum Commands {
        /// Object operations.
        #[argx(alias = "obj")]
        Objects(ObjectsArgs),
        /// Show service status.
        Status(StatusArgs),
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

    /// Successful result of echoing one value.
    #[derive(argx::Contract)]
    struct EchoOutput {
        value: String,
    }

    /// Echo failure exposed by the command.
    #[derive(argx::Contract)]
    enum EchoError {
        WriteFailed,
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

    /// Domain value shared by multiple invocation bindings.
    #[derive(Debug, PartialEq, Eq, argx::Contract)]
    enum OutputFormat {
        Json,
        Text,
    }

    impl std::str::FromStr for OutputFormat {
        type Err = &'static str;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            match value {
                "json" => Ok(Self::Json),
                "text" => Ok(Self::Text),
                _ => Err("expected `json` or `text`"),
            }
        }
    }

    /// Reusable typed values used to verify semantic projection through flattening.
    #[derive(argx::Args)]
    struct FormatArgs {
        #[argx(long)]
        format: Option<OutputFormat>,
    }

    /// CLI used to verify named semantic type references and definition deduplication.
    #[derive(argx::Parser)]
    #[argx(name = "typed-contract")]
    struct TypedContractCli {
        #[argx(flatten)]
        format: FormatArgs,
        fallback: OutputFormat,
    }

    /// Generic CLI used to verify semantic type projection does not regress generic parsers.
    #[expect(dead_code, reason = "shape is exercised through generated contract metadata")]
    #[derive(argx::Parser)]
    #[argx(name = "generic-contract")]
    struct GenericContractCli<T> {
        value: T,
    }

    /// Generic execution result used to verify concrete command monomorphizations stay distinct.
    #[expect(dead_code, reason = "shape is exercised through generated contract metadata")]
    #[derive(argx::Contract)]
    struct GenericOutput<T> {
        value: T,
    }

    /// Nested command used to verify type definitions follow discovery detail.
    #[expect(dead_code, reason = "shape is exercised through generated contract metadata")]
    #[derive(argx::Args)]
    struct TypedLeafArgs {
        format: OutputFormat,
    }

    /// Empty nested command used to exercise explicit no-payload execution results.
    #[derive(argx::Args)]
    struct EmptyArgs {}

    /// Nested typed command branches.
    #[expect(dead_code, reason = "shape is exercised through generated contract metadata")]
    #[derive(argx::Subcommand)]
    enum TypedNestedCommands {
        Leaf(TypedLeafArgs),
        Empty(EmptyArgs),
    }

    /// Root with no values of its own and a typed descendant.
    #[expect(dead_code, reason = "shape is exercised through generated contract metadata")]
    #[derive(argx::Parser)]
    #[argx(name = "typed-nested")]
    struct TypedNestedCli {
        #[argx(subcommand)]
        command: TypedNestedCommands,
    }

    #[argx::contract(GetArgs)]
    fn get_contract() -> Result<GetOutput, GetError> {
        Err(GetError::NotFound)
    }

    #[argx::contract(ListArgs)]
    const fn list_contract() -> Result<(), ()> {
        Ok(())
    }

    #[argx::contract(StatusArgs)]
    const fn status_contract() -> Result<(), ()> {
        Ok(())
    }

    #[argx::contract(JsonCli)]
    fn json_contract() -> Result<EchoOutput, EchoError> {
        Ok(EchoOutput { value: String::new() })
    }

    #[argx::contract(RepeatedOptionCli)]
    const fn repeated_option_contract() -> Result<(), ()> {
        Ok(())
    }

    #[argx::contract(ProjectionCli)]
    const fn projection_contract() -> Result<(), ()> {
        Ok(())
    }

    #[argx::contract(TypedContractCli)]
    const fn typed_contract() -> Result<(), ()> {
        Ok(())
    }

    #[argx::contract(GenericContractCli<u16>)]
    const fn generic_u16_contract() -> Result<GenericOutput<u16>, ()> {
        Ok(GenericOutput { value: 0 })
    }

    #[argx::contract(GenericContractCli<u32>)]
    const fn generic_u32_contract() -> Result<GenericOutput<u32>, ()> {
        Ok(GenericOutput { value: 0 })
    }

    /// Successful output for the nested typed leaf.
    #[derive(argx::Contract)]
    struct TypedLeafOutput {
        format: OutputFormat,
    }

    #[argx::contract(TypedLeafArgs)]
    const fn typed_leaf_contract() -> Result<TypedLeafOutput, ()> {
        Ok(TypedLeafOutput { format: OutputFormat::Json })
    }

    #[argx::contract(EmptyArgs)]
    const fn empty_contract() -> Result<(), ()> {
        Ok(())
    }

    /// Successful output used to verify execution contract definitions.
    #[derive(Debug, PartialEq, Eq, argx::Contract)]
    struct ExecutionOutput {
        accepted: bool,
    }

    /// Stable execution error used to verify error contract definitions.
    #[derive(Debug, PartialEq, Eq, argx::Contract)]
    enum ExecutionError {
        Rejected,
    }

    /// Root command used to isolate execution result discovery.
    #[derive(argx::Parser)]
    #[argx(name = "execute")]
    struct ExecutionCli {
        value: String,
    }

    /// Runtime-only state deliberately excluded from machine contracts.
    struct RuntimeContext;

    #[argx::contract(ExecutionCli)]
    const fn execute_contract(
        _context: &RuntimeContext,
    ) -> Result<ExecutionOutput, ExecutionError> {
        Ok(ExecutionOutput { accepted: true })
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
        let value = option.value.as_ref().expect("tag takes a value");
        assert_eq!(value.min_values, 1);
        assert_eq!(value.max_values, Some(1));
        assert_eq!(value.value_type, TypeContractValue::String);
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
    fn invocation_values_reference_one_shared_named_type_definition() {
        let cli = TypedContractCli::try_parse_from(["typed-contract", "--format", "json", "text"])
            .expect("typed contract fixture should parse");
        assert_eq!(cli.format.format, Some(OutputFormat::Json));
        assert_eq!(cli.fallback, OutputFormat::Text);

        let contract = TypedContractCli::contract(ContractRequest::root())
            .expect("typed invocation contract should exist");
        assert_eq!(contract.types.version, argx::TYPE_CONTRACT_VERSION);
        assert_eq!(contract.types.definitions.len(), 1);
        let definition = &contract.types.definitions[0];
        assert_eq!(definition.id, "type-0");
        assert_eq!(definition.name, "OutputFormat");
        assert!(matches!(
            &definition.kind,
            TypeDefinitionKind::Enum { variants }
                if variants.iter().map(|variant| variant.name.as_str()).eq(["Json", "Text"])
        ));

        let invocation = contract.command.invocation.expect("root command should be detailed");
        let context = &invocation.contexts[0];
        let option_type =
            &context.options[0].value.as_ref().expect("format takes a value").value_type;
        let positional_type = &context.arguments[0].value.value_type;
        let expected = TypeContractValue::Reference { definition: "type-0".to_owned() };
        assert_eq!(option_type, &expected);
        assert_eq!(positional_type, &expected);
    }

    #[test]
    fn execution_contracts_share_semantic_types_and_ignore_runtime_parameters() {
        let result = execute_contract(&RuntimeContext)
            .expect("execution fixture should remain an ordinary callable function");
        assert!(result.accepted);

        let contract = ExecutionCli::contract(ContractRequest::root())
            .expect("execution contract should exist");
        let execution = contract.command.execution.expect("root command is invocable");
        assert_eq!(
            execution.success,
            TypeContractValue::Reference { definition: "type-0".to_owned() },
        );
        assert_eq!(
            execution.error,
            TypeContractValue::Reference { definition: "type-1".to_owned() },
        );
        assert_eq!(contract.types.definitions.len(), 2);
        assert_eq!(contract.types.definitions[0].name, "ExecutionOutput");
        assert_eq!(contract.types.definitions[1].name, "ExecutionError");
        assert!(
            contract.types.definitions.iter().all(|definition| definition.name != "RuntimeContext")
        );
    }

    #[test]
    fn unit_execution_contracts_explicitly_mean_no_semantic_payload() {
        let contract = TypedNestedCli::contract(ContractRequest::new(["empty"]))
            .expect("empty command contract should exist");
        let execution = contract.command.execution.expect("empty command is invocable");

        assert_eq!(execution.success, TypeContractValue::Unit);
        assert_eq!(execution.error, TypeContractValue::Unit);
    }

    #[test]
    fn generic_parser_and_execution_contracts_resolve_the_concrete_monomorphization() {
        for (contract, primitive) in [
            (
                GenericContractCli::<u16>::contract(ContractRequest::root())
                    .expect("u16 generic parser contract should exist"),
                PrimitiveType::U16,
            ),
            (
                GenericContractCli::<u32>::contract(ContractRequest::root())
                    .expect("u32 generic parser contract should exist"),
                PrimitiveType::U32,
            ),
        ] {
            let execution = contract.command.execution.as_ref().expect("root command is invocable");
            assert_eq!(
                execution.success,
                TypeContractValue::Reference { definition: "type-0".to_owned() },
            );
            assert_eq!(contract.types.definitions.len(), 1);
            assert_eq!(contract.types.definitions[0].name, "GenericOutput");
            let TypeDefinitionKind::Struct { fields } = &contract.types.definitions[0].kind else {
                panic!("GenericOutput must resolve to a struct definition");
            };
            assert_eq!(fields[0].value_type, TypeContractValue::Primitive { primitive },);

            let invocation =
                contract.command.invocation.as_ref().expect("root command is detailed");
            assert_eq!(
                invocation.contexts[0].arguments[0].value.value_type,
                TypeContractValue::Primitive { primitive },
            );
        }
    }

    #[test]
    fn type_definitions_follow_the_discovery_detail_that_is_returned() {
        let shallow = TypedNestedCli::contract(ContractRequest::root())
            .expect("shallow nested contract should exist");
        assert!(shallow.types.definitions.is_empty());
        assert!(shallow.command.subcommands[0].invocation.is_none());

        let selected = TypedNestedCli::contract(ContractRequest::new(["leaf"]))
            .expect("selected typed descendant should exist");
        assert_eq!(selected.types.definitions.len(), 2);
        assert_eq!(selected.types.definitions[0].name, "OutputFormat");
        assert_eq!(selected.types.definitions[1].name, "TypedLeafOutput");
        assert_eq!(
            selected
                .command
                .execution
                .as_ref()
                .expect("selected command should expose execution")
                .success,
            TypeContractValue::Reference { definition: "type-1".to_owned() },
        );
        let invocation = selected.command.invocation.expect("selected command should be detailed");
        let value_type = &invocation.contexts[1].arguments[0].value.value_type;
        assert_eq!(value_type, &TypeContractValue::Reference { definition: "type-0".to_owned() },);

        let recursive = TypedNestedCli::contract(ContractRequest::root().recursive())
            .expect("recursive nested contract should exist");
        assert_eq!(recursive.types.definitions.len(), 2);
        assert!(recursive.command.subcommands[0].invocation.is_some());
        assert!(recursive.command.subcommands[1].invocation.is_some());
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
        assert!(contract.command.execution.is_none());
        assert_eq!(contract.command.subcommands.len(), 2);

        let objects = &contract.command.subcommands[0];
        assert!(objects.path.iter().map(String::as_str).eq(["objects"]));
        assert!(objects.aliases.iter().map(String::as_str).eq(["obj"]));
        assert!(!objects.invocable);
        assert!(objects.invocation.is_none());
        assert!(objects.execution.is_none());
        assert!(objects.subcommands.is_empty());

        let status = &contract.command.subcommands[1];
        assert!(status.path.iter().map(String::as_str).eq(["status"]));
        assert!(status.invocable);
        assert!(status.invocation.is_none());
        assert!(status.execution.is_none());
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
        assert!(command.execution.is_some());

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
        assert_eq!(root.options[0].value.as_ref().expect("config takes a value").min_values, 1);
        assert_eq!(
            root.options[0].value.as_ref().expect("config takes a value").max_values,
            Some(1),
        );
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
        assert!(objects.subcommands[0].execution.is_some());
        assert!(objects.subcommands[1].execution.is_some());
        assert!(objects.subcommands[0].path.iter().map(String::as_str).eq(["objects", "get"]));
        assert!(objects.subcommands[1].path.iter().map(String::as_str).eq(["objects", "list"]));
    }

    #[test]
    fn nested_json_wire_shape_preserves_complete_invocation_contexts() {
        let contract = Cli::contract(ContractRequest::new(["obj", "show"]))
            .expect("nested aliases should resolve during contract lookup");
        let json =
            contract.to_json_pretty().expect("nested Argx contract should serialize as JSON");

        assert_eq!(
            json,
            include_str!("fixtures/contracts/v1/nested.json"),
            "contract v1 wire fixture changed",
        );
    }

    #[test]
    fn unknown_dynamic_paths_fail_without_guessing() {
        let error = Cli::contract(ContractRequest::new(["objects", "missing"]))
            .expect_err("unknown command path should fail");

        assert_eq!(error.to_string(), "unknown contract command `missing` below `objects`");
    }

    #[test]
    fn unknown_contract_paths_escape_control_characters_in_diagnostics() {
        let error = Cli::contract(ContractRequest::new(["missing\n\u{1b}[31m"]))
            .expect_err("unknown command path should fail");
        let rendered = error.to_string();

        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.contains(r"\n"));
    }

    #[test]
    fn unknown_root_command_paths_report_without_a_parent_path() {
        let request = ContractRequest::new(["missing"]);
        assert!(request.path().iter().map(String::as_str).eq(["missing"]));

        let error = Cli::contract(request).expect_err("unknown root command should fail");
        assert_eq!(error.to_string(), "unknown contract command `missing`");
    }

    #[test]
    fn repeated_discovery_is_deterministic_and_self_contained() {
        let first = TypedNestedCli::contract(ContractRequest::root().recursive())
            .expect("first recursive contract should exist");
        let second = TypedNestedCli::contract(ContractRequest::root().recursive())
            .expect("second recursive contract should exist");

        assert_eq!(first, second);
    }

    #[test]
    fn json_wire_shape_is_versioned_and_stable() {
        let contract =
            JsonCli::contract(ContractRequest::root()).expect("root contract should exist");
        let json = contract.to_json_pretty().expect("Argx contract should serialize as JSON");

        assert_eq!(
            json,
            include_str!("fixtures/contracts/v1/root.json"),
            "contract v1 wire fixture changed",
        );
    }
}
