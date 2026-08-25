//! End-to-end typed parser contract tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use std::{ffi::OsString, path::PathBuf, process::Command};

    use argx::{Error, Parser as _};

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct Cli {
        #[argx(short, long)]
        verbose: bool,
        #[argx(long)]
        port: Option<u16>,
        #[argx(long)]
        define: Vec<String>,
        #[argx(long)]
        paths: Option<Vec<PathBuf>>,
        input: String,
        rest: Vec<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct RequiredFlag {
        #[argx(long)]
        output: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct BoolPositional {
        enabled: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct PathCli {
        path: PathBuf,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct OsStringCli {
        value: OsString,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct TextCli {
        value: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct OptionalMany {
        #[argx(long)]
        tags: Option<Vec<String>>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct NegativeValues {
        #[argx(long, allow_negative_numbers)]
        number: Option<i32>,
        #[argx(allow_negative_numbers)]
        positional: Option<i32>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct HyphenValue {
        #[argx(long, allow_hyphen_values)]
        raw: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct SharedArgs {
        #[argx(long)]
        shared: bool,
        #[argx(long)]
        tag: Vec<String>,
        middle: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct NestedArgs {
        #[argx(long)]
        nested: Option<i64>,
        #[argx(flatten)]
        shared: SharedArgs,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ExtraArgs {
        #[argx(long)]
        extra: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct FlattenedCli {
        #[argx(long)]
        root: bool,
        before: String,
        #[argx(flatten)]
        nested: NestedArgs,
        #[argx(flatten)]
        extra: ExtraArgs,
        after: String,
    }

    mod identical_a {
        #[derive(Debug, PartialEq, Eq, argx::Args)]
        pub(super) struct Values {
            pub(super) value: String,
        }
    }

    mod identical_b {
        #[derive(Debug, PartialEq, Eq, argx::Args)]
        pub(super) struct Values {
            pub(super) value: String,
        }
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct IdenticalFlattened {
        #[argx(flatten)]
        first: identical_a::Values,
        #[argx(flatten)]
        second: identical_b::Values,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct RequiredFlattened {
        #[argx(long)]
        required: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct DuplicateFlattened {
        #[argx(long)]
        duplicate: Option<u16>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct FlattenPrecedence {
        #[argx(flatten)]
        required: RequiredFlattened,
        #[argx(flatten)]
        duplicate: DuplicateFlattened,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct EmptyArgs;

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct EmptyFlatten {
        #[argx(flatten)]
        empty: EmptyArgs,
        value: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct Empty;

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct CommandShared {
        #[argx(long)]
        dry_run: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct AddArgs {
        #[argx(flatten)]
        shared: CommandShared,
        #[argx(long)]
        force: bool,
        name: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct GetArgs {
        key: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct SetArgs {
        #[argx(long)]
        raw: bool,
        key: String,
        value: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum ConfigCommand {
        Get(GetArgs),
        Set(SetArgs),
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ConfigArgs {
        #[argx(long)]
        local: bool,
        #[argx(subcommand)]
        command: ConfigCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum RootCommand {
        Add(AddArgs),
        Config(ConfigArgs),
        #[argx(name = "show-status")]
        Status,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct SubcommandCli {
        #[argx(long)]
        verbose: bool,
        workspace: String,
        #[argx(subcommand)]
        command: RootCommand,
    }

    /// Root help summary overridden through the explicit command attribute.
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "tool", about = "Manage things")]
    struct HelpCli {
        /// Enable verbose output.
        #[argx(short, long)]
        verbose: bool,
        /// This doc comment is overridden by explicit help.
        #[argx(long, help = "Output path")]
        output: String,
        /// Workspace name.
        workspace: String,
        #[argx(subcommand)]
        command: HelpCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum HelpCommand {
        /// Configure values.
        Config(HelpConfig),
        /// Show current status.
        Status,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct HelpConfig {
        /// Use local configuration.
        #[argx(long)]
        local: bool,
        /// Configuration key.
        key: String,
    }

    const SHORT_VERSION: &str = "1.2.3";
    const LONG_VERSION: &str = "1.2.3 (build abc123)";

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "meta", version = SHORT_VERSION, long_version = LONG_VERSION)]
    struct MetadataCli {
        #[argx(subcommand)]
        command: MetadataCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum MetadataCommand {
        #[argx(version = SHORT_VERSION, long_version = LONG_VERSION)]
        Run,
        Internal,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "short-only", version = SHORT_VERSION)]
    struct ShortVersionOnly;

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "long-only", long_version = LONG_VERSION)]
    struct LongVersionOnly;

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct UserVersionFlag {
        #[argx(short = 'V', long = "version")]
        version: bool,
    }

    const DEFAULT_PORT: u16 = 3000;

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "defaults")]
    struct DefaultCli {
        #[argx(long, default = DEFAULT_PORT)]
        port: u16,
        #[argx(long, default = String::from("development"))]
        profile: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct DefaultShared {
        #[argx(long, default = 4_u16)]
        jobs: u16,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "environment")]
    struct EnvironmentCli {
        #[argx(long, env = "ARGX_TEST_PORT", default = 3000_u16)]
        port: u16,
        #[argx(long, env = "ARGX_TEST_PROFILE")]
        profile: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct RequiredEnvironmentCli {
        #[argx(long, env = "ARGX_TEST_REQUIRED_PORT")]
        port: u16,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct EnvironmentRequirednessCli {
        #[argx(long)]
        value: Option<u16>,
        #[argx(long, env = "ARGX_TEST_REQUIRED_PORT")]
        port: u16,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct EnvironmentShared {
        #[argx(long, env = "ARGX_TEST_JOBS")]
        jobs: Option<u16>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct FlattenedEnvironmentCli {
        #[argx(flatten)]
        shared: EnvironmentShared,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct EnvironmentPathCli {
        #[argx(long, env = "ARGX_TEST_PATH")]
        path: Option<PathBuf>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum EnvironmentCommand {
        Child,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct GlobalEnvironmentCli {
        #[argx(long, global, env = "ARGX_TEST_REGION")]
        region: Option<String>,
        #[argx(subcommand)]
        command: EnvironmentCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct FlattenedDefaults {
        #[argx(flatten)]
        shared: DefaultShared,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ReusedArgs {
        #[argx(long)]
        value: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum ReusedCommand {
        Child(ReusedArgs),
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct ReusedAcrossScopes {
        #[argx(flatten)]
        root: ReusedArgs,
        #[argx(subcommand)]
        command: ReusedCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct StartArgs {
        #[argx(long)]
        force: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct StopArgs {
        #[argx(long)]
        force: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum SiblingCommand {
        Start(StartArgs),
        Stop(StopArgs),
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct SiblingCli {
        #[argx(subcommand)]
        command: SiblingCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct GlobalCommon {
        #[argx(long, global)]
        verbose: bool,
        #[argx(long, global)]
        profile: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct GlobalLeafArgs {
        #[argx(long)]
        verbose: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum GlobalNestedCommand {
        Leaf(GlobalLeafArgs),
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct GlobalOuterArgs {
        #[argx(long, global)]
        region: Option<String>,
        #[argx(subcommand)]
        command: GlobalNestedCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum GlobalCommand {
        Outer(GlobalOuterArgs),
        Other,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct GlobalCli {
        #[argx(flatten)]
        common: GlobalCommon,
        #[argx(subcommand)]
        command: GlobalCommand,
    }

    #[test]
    fn parses_typed_switches_values_and_positionals() {
        let parsed = Cli::try_parse_args([
            "-v",
            "--port",
            "8080",
            "--define=one",
            "--define",
            "two",
            "--paths",
            "first",
            "--paths=second",
            "input.txt",
            "tail-a",
            "tail-b",
        ])
        .expect("valid command line");

        assert_eq!(
            parsed,
            Cli {
                verbose: true,
                port: Some(8080),
                define: vec![String::from("one"), String::from("two")],
                paths: Some(vec![PathBuf::from("first"), PathBuf::from("second")]),
                input: String::from("input.txt"),
                rest: vec![String::from("tail-a"), String::from("tail-b")],
            }
        );
    }

    #[test]
    fn complete_argv_entry_point_discards_only_argv0() {
        let parsed = Cli::try_parse_from(["argx-test", "input.txt"]).expect("valid argv");
        assert_eq!(parsed.input, "input.txt");

        let parsed = Cli::try_parse_args(["argx-test"]).expect("argv without program name");
        assert_eq!(parsed.input, "argx-test");
    }

    #[test]
    fn empty_complete_argv_is_an_empty_argument_list() {
        assert_eq!(Empty::try_parse_from(std::iter::empty::<&str>()), Ok(Empty));
    }

    #[test]
    fn missing_required_values_are_reported_during_finalization() {
        assert_eq!(
            Cli::try_parse_args(std::iter::empty::<&str>()),
            Err(Error::MissingRequired { name: "input" })
        );
        assert_eq!(
            RequiredFlag::try_parse_args(std::iter::empty::<&str>()),
            Err(Error::MissingRequired { name: "output" })
        );
    }

    #[test]
    fn scalar_occurrences_are_strict_and_collections_repeat() {
        assert_eq!(
            Cli::try_parse_args(["--port", "1", "--port", "2", "input"]),
            Err(Error::DuplicateArgument { name: "port" })
        );
        assert_eq!(
            Cli::try_parse_args(["-v", "--verbose", "input"]),
            Err(Error::DuplicateArgument { name: "verbose" })
        );
        assert_eq!(
            Cli::try_parse_args(["--port", "not-a-port", "--port", "2", "input"]),
            Err(Error::DuplicateArgument { name: "port" })
        );

        let parsed = Cli::try_parse_args(["--define", "one", "--define", "two", "input"])
            .expect("repeatable value flag");
        assert_eq!(parsed.define, vec![String::from("one"), String::from("two")]);
    }

    #[test]
    fn raw_syntax_errors_take_precedence_over_deferred_cardinality_errors() {
        assert_eq!(
            Cli::try_parse_args(["--port", "1", "--port", "2", "--unknown"]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() })
        );
    }

    #[test]
    fn raw_parser_failures_are_exposed_as_owned_public_errors() {
        assert_eq!(
            Empty::try_parse_args(["--unknown"]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() })
        );
        assert_eq!(Cli::try_parse_args(["--port"]), Err(Error::MissingValue { name: "port" }));
        assert_eq!(
            Cli::try_parse_args(["--verbose=true", "input"]),
            Err(Error::UnexpectedValue { name: "verbose" })
        );
        assert_eq!(
            Empty::try_parse_args(["extra"]),
            Err(Error::UnexpectedArgument { token: b"extra".to_vec() })
        );
    }

    #[test]
    fn typed_conversion_reports_the_field_and_bad_value() {
        let error = Cli::try_parse_args(["--port", "not-a-port", "input"])
            .expect_err("invalid integer must fail");
        match error {
            Error::InvalidValue(error) => {
                assert_eq!(error.name, "port");
                assert_eq!(error.value, "not-a-port");
                assert!(!error.reason.is_empty());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bool_positionals_are_values_not_switches() {
        assert_eq!(BoolPositional::try_parse_args(["true"]), Ok(BoolPositional { enabled: true }));
        assert_eq!(
            BoolPositional::try_parse_args(["false"]),
            Ok(BoolPositional { enabled: false })
        );
    }

    #[test]
    fn explicit_value_policies_reach_the_raw_parser() {
        assert_eq!(
            NegativeValues::try_parse_args(["--number", "-12", "-7"]),
            Ok(NegativeValues { number: Some(-12), positional: Some(-7) })
        );
        assert_eq!(
            HyphenValue::try_parse_args(["--raw", "--not-a-flag"]),
            Ok(HyphenValue { raw: Some(String::from("--not-a-flag")) })
        );
    }

    #[test]
    fn optional_collection_preserves_absence_and_empty_values() {
        assert_eq!(
            OptionalMany::try_parse_args(std::iter::empty::<&str>()),
            Ok(OptionalMany { tags: None })
        );
        assert_eq!(
            OptionalMany::try_parse_args(["--tags="]),
            Ok(OptionalMany { tags: Some(vec![String::new()]) })
        );
    }

    #[test]
    fn flattened_args_compose_recursively_and_preserve_positional_order() {
        let parsed = FlattenedCli::try_parse_args([
            "--root",
            "--nested=42",
            "--shared",
            "--tag=one",
            "--tag",
            "two",
            "--extra=value",
            "before",
            "middle",
            "after",
        ])
        .expect("valid recursively flattened command line");

        assert_eq!(
            parsed,
            FlattenedCli {
                root: true,
                before: String::from("before"),
                nested: NestedArgs {
                    nested: Some(42),
                    shared: SharedArgs {
                        shared: true,
                        tag: vec![String::from("one"), String::from("two")],
                        middle: String::from("middle"),
                    },
                },
                extra: ExtraArgs { extra: Some(String::from("value")) },
                after: String::from("after"),
            }
        );
    }

    #[test]
    fn empty_args_groups_flatten_without_changing_binding() {
        assert_eq!(
            EmptyFlatten::try_parse_args(["value"]),
            Ok(EmptyFlatten { empty: EmptyArgs, value: String::from("value") })
        );
    }

    #[test]
    fn identical_flattened_declarations_route_by_their_own_keys() {
        assert_eq!(
            IdenticalFlattened::try_parse_args(["first", "second"]),
            Ok(IdenticalFlattened {
                first: identical_a::Values { value: String::from("first") },
                second: identical_b::Values { value: String::from("second") },
            })
        );
    }

    #[test]
    fn flattened_checks_preserve_global_error_precedence() {
        assert_eq!(
            FlattenPrecedence::try_parse_args(["--duplicate=not-a-number", "--duplicate=2",]),
            Err(Error::DuplicateArgument { name: "duplicate" })
        );
        assert_eq!(
            FlattenPrecedence::try_parse_args([
                "--duplicate=not-a-number",
                "--duplicate=2",
                "--unknown",
            ]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() })
        );
        assert_eq!(
            FlattenPrecedence::try_parse_args(std::iter::empty::<&str>()),
            Err(Error::MissingRequired { name: "required" })
        );
    }

    #[cfg(unix)]
    #[test]
    fn os_backed_fields_preserve_non_utf8_values() {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        let raw = OsString::from_vec(vec![b'p', 0xff, b't']);
        let parsed = PathCli::try_parse_args([raw.clone()]).expect("valid Unix path bytes");
        assert_eq!(parsed.path.as_os_str().as_bytes(), raw.as_os_str().as_bytes());

        let parsed = OsStringCli::try_parse_args([raw.clone()]).expect("valid Unix OS string");
        assert_eq!(parsed.value.as_os_str().as_bytes(), raw.as_os_str().as_bytes());
    }

    #[cfg(unix)]
    #[test]
    fn attached_os_backed_values_preserve_non_utf8_suffixes() {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        #[derive(Debug, PartialEq, Eq, argx::Parser)]
        struct Attached {
            #[argx(long)]
            path: PathBuf,
        }

        let raw = OsString::from_vec(vec![b'-', b'-', b'p', b'a', b't', b'h', b'=', 0xff]);
        let parsed = Attached::try_parse_args([raw]).expect("valid Unix path bytes");
        assert_eq!(parsed.path.as_os_str().as_bytes(), &[0xff]);
    }

    #[cfg(unix)]
    #[test]
    fn text_fields_reject_non_utf8_without_lossy_conversion() {
        use std::os::unix::ffi::OsStringExt as _;

        let raw = OsString::from_vec(vec![b'x', 0xff]);
        assert_eq!(
            TextCli::try_parse_args([raw]),
            Err(Error::InvalidUtf8 { name: "value", value: vec![b'x', 0xff] })
        );
    }

    #[test]
    fn generated_help_uses_static_metadata_and_selected_command_scope() {
        let root = HelpCli::render_help();
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            root.as_str(),
            snapbox::str![[r#"
Manage things

Usage: tool [OPTIONS] --output <OUTPUT> <WORKSPACE> <COMMAND>

Arguments:
  <WORKSPACE>  Workspace name.

Commands:
  config  Configure values.
  status  Show current status.

Options:
  -v, --verbose      Enable verbose output.
  --output <OUTPUT>  Output path
  -h, --help         Print help

"#]],
        );

        assert_eq!(HelpCli::try_parse_args(["--help"]), Err(Error::DisplayHelp { help: root }),);

        let nested = HelpCli::try_parse_args(["--output", "out", "acme", "config", "--help"]);
        let Err(Error::DisplayHelp { help }) = nested else {
            panic!("nested help request did not return generated help")
        };
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            help,
            snapbox::str![[r#"
Configure values.

Usage: tool config [OPTIONS] <KEY>

Arguments:
  <KEY>  Configuration key.

Options:
  --local     Use local configuration.
  -h, --help  Print help

"#]],
        );

        let status = HelpCli::try_parse_args(["--output", "out", "acme", "status"])
            .expect("status command should parse");
        assert!(!status.verbose);
        assert_eq!(status.output, "out");
        assert_eq!(status.workspace, "acme");
        assert!(matches!(status.command, HelpCommand::Status));

        let config =
            HelpCli::try_parse_args(["--output", "out", "acme", "config", "--local", "theme"])
                .expect("config command should parse");
        let HelpCommand::Config(config) = config.command else {
            panic!("config command did not construct its payload")
        };
        assert!(config.local);
        assert_eq!(config.key, "theme");
    }

    #[test]
    fn help_precedes_deferred_binding_errors_but_not_detached_value_rules() {
        assert!(matches!(
            HelpCli::try_parse_args(["--verbose", "--verbose", "--help"]),
            Err(Error::DisplayHelp { .. })
        ));
        assert_eq!(
            HelpCli::try_parse_args(["--output", "--help"]),
            Err(Error::MissingValue { name: "output" }),
        );
        assert_eq!(
            Empty::try_parse_args(["--", "--help"]),
            Err(Error::UnexpectedArgument { token: b"--help".to_vec() }),
        );
    }

    #[test]
    fn version_metadata_is_scoped_to_the_selected_command() {
        assert_eq!(
            MetadataCli::try_parse_args(["-V"]),
            Err(Error::DisplayVersion { version: String::from("meta 1.2.3\n") }),
        );
        assert_eq!(
            MetadataCli::try_parse_args(["--version"]),
            Err(Error::DisplayVersion { version: String::from("meta 1.2.3 (build abc123)\n") }),
        );
        assert_eq!(
            MetadataCli::try_parse_args(["--version=value"]),
            Err(Error::UnexpectedValue { name: "version" }),
        );
        assert_eq!(
            MetadataCli::try_parse_args(["run", "--version"]),
            Err(Error::DisplayVersion { version: String::from("run 1.2.3 (build abc123)\n") }),
        );
        assert_eq!(
            MetadataCli::try_parse_args(["internal", "--version"]),
            Err(Error::UnknownFlag { token: b"--version".to_vec() }),
        );
        assert_eq!(
            MetadataCli::try_parse_args(["internal"]),
            Ok(MetadataCli { command: MetadataCommand::Internal }),
        );
        assert_eq!(
            ShortVersionOnly::try_parse_args(["--version"]),
            Err(Error::DisplayVersion { version: String::from("short-only 1.2.3\n") }),
        );
        assert_eq!(
            LongVersionOnly::try_parse_args(["-V"]),
            Err(Error::DisplayVersion {
                version: String::from("long-only 1.2.3 (build abc123)\n"),
            }),
        );
        assert_eq!(
            UserVersionFlag::try_parse_args(["--version"]),
            Ok(UserVersionFlag { version: true }),
        );
        assert_eq!(UserVersionFlag::try_parse_args(["-V"]), Ok(UserVersionFlag { version: true }),);
    }

    #[test]
    fn typed_defaults_fill_absent_scalar_flags_and_argv_wins() {
        assert_eq!(
            DefaultCli::try_parse_args(std::iter::empty::<&str>()),
            Ok(DefaultCli { port: 3000, profile: Some(String::from("development")) }),
        );
        assert_eq!(
            DefaultCli::try_parse_args(["--port", "8080", "--profile", "production"]),
            Ok(DefaultCli { port: 8080, profile: Some(String::from("production")) }),
        );
        assert_eq!(
            DefaultCli::try_parse_args(["--port", "1", "--port", "2"]),
            Err(Error::DuplicateArgument { name: "port" }),
        );
        assert_eq!(
            FlattenedDefaults::try_parse_args(std::iter::empty::<&str>()),
            Ok(FlattenedDefaults { shared: DefaultShared { jobs: 4 } }),
        );
    }

    #[test]
    fn typed_defaults_do_not_repair_invalid_or_incomplete_argv() {
        assert_eq!(
            DefaultCli::try_parse_args(["--port"]),
            Err(Error::MissingValue { name: "port" }),
        );

        let error = DefaultCli::try_parse_args(["--port", "not-a-port"])
            .expect_err("an explicit invalid value must not fall back to the default");
        let Error::InvalidValue(error) = error else { panic!("unexpected error: {error:?}") };
        assert_eq!(error.name, "port");
        assert_eq!(error.value, "not-a-port");
    }

    #[test]
    fn typed_defaults_make_bare_value_flags_optional_in_help() {
        let help = DefaultCli::render_help();
        assert!(help.contains("Usage: defaults [OPTIONS]"));
        assert!(!help.contains("Usage: defaults --port"));
        assert!(help.contains("--port <PORT>"));
    }

    #[test]
    fn environment_value_sources_follow_argv_env_default_precedence() {
        let executable = std::env::current_exe().expect("test executable should be available");
        for case in [
            "default",
            "environment",
            "argv",
            "invalid-environment",
            "incomplete-argv",
            "required-missing",
            "required-environment",
            "required-before-conversion",
            "flattened",
            "global",
        ] {
            let mut command = Command::new(&executable);
            command
                .arg("--exact")
                .arg("tests::environment_value_source_child")
                .env("ARGX_TEST_CASE", case)
                .env_remove("ARGX_TEST_PORT")
                .env_remove("ARGX_TEST_PROFILE")
                .env_remove("ARGX_TEST_REQUIRED_PORT")
                .env_remove("ARGX_TEST_JOBS")
                .env_remove("ARGX_TEST_PATH")
                .env_remove("ARGX_TEST_REGION");
            match case {
                "environment" | "argv" | "incomplete-argv" => {
                    command.env("ARGX_TEST_PORT", "4000").env("ARGX_TEST_PROFILE", "staging");
                }
                "invalid-environment" => {
                    command.env("ARGX_TEST_PORT", "not-a-port");
                }
                "required-environment" => {
                    command.env("ARGX_TEST_REQUIRED_PORT", "5000");
                }
                "flattened" => {
                    command.env("ARGX_TEST_JOBS", "6");
                }
                "global" => {
                    command.env("ARGX_TEST_REGION", "eu-west");
                }
                "default" | "required-missing" | "required-before-conversion" => {}
                _ => unreachable!("case list is exhaustive"),
            }

            let output = command.output().expect("child environment test should run");
            assert!(
                output.status.success(),
                "environment case {case} failed:\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }

    #[test]
    fn environment_value_source_child() {
        let Ok(case) = std::env::var("ARGX_TEST_CASE") else {
            return;
        };

        match case.as_str() {
            "default" => assert_eq!(
                EnvironmentCli::try_parse_args(std::iter::empty::<&str>()),
                Ok(EnvironmentCli { port: 3000, profile: None }),
            ),
            "environment" => assert_eq!(
                EnvironmentCli::try_parse_args(std::iter::empty::<&str>()),
                Ok(EnvironmentCli { port: 4000, profile: Some(String::from("staging")) }),
            ),
            "argv" => assert_eq!(
                EnvironmentCli::try_parse_args(["--port", "5000", "--profile", "production",]),
                Ok(EnvironmentCli { port: 5000, profile: Some(String::from("production")) }),
            ),
            "invalid-environment" => {
                let error = EnvironmentCli::try_parse_args(std::iter::empty::<&str>())
                    .expect_err("an invalid environment value must not fall back to the default");
                let Error::InvalidValue(error) = error else {
                    panic!("unexpected error: {error:?}");
                };
                assert_eq!(error.name, "port");
                assert_eq!(error.value, "not-a-port");
            }
            "incomplete-argv" => assert_eq!(
                EnvironmentCli::try_parse_args(["--port"]),
                Err(Error::MissingValue { name: "port" }),
            ),
            "required-missing" => assert_eq!(
                RequiredEnvironmentCli::try_parse_args(std::iter::empty::<&str>()),
                Err(Error::MissingRequired { name: "port" }),
            ),
            "required-environment" => assert_eq!(
                RequiredEnvironmentCli::try_parse_args(std::iter::empty::<&str>()),
                Ok(RequiredEnvironmentCli { port: 5000 }),
            ),
            "required-before-conversion" => assert_eq!(
                EnvironmentRequirednessCli::try_parse_args(["--value", "not-a-number"]),
                Err(Error::MissingRequired { name: "port" }),
            ),
            "flattened" => assert_eq!(
                FlattenedEnvironmentCli::try_parse_args(std::iter::empty::<&str>()),
                Ok(FlattenedEnvironmentCli { shared: EnvironmentShared { jobs: Some(6) } }),
            ),
            "global" => assert_eq!(
                GlobalEnvironmentCli::try_parse_args(["child"]),
                Ok(GlobalEnvironmentCli {
                    region: Some(String::from("eu-west")),
                    command: EnvironmentCommand::Child,
                }),
            ),
            _ => panic!("unknown environment test case {case}"),
        }
    }

    #[test]
    #[cfg(unix)]
    fn environment_os_values_preserve_non_utf8_paths() {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        let executable = std::env::current_exe().expect("test executable should be available");
        let expected = OsString::from_vec(b"path-\xff".to_vec());
        let output = Command::new(executable)
            .arg("--exact")
            .arg("tests::environment_os_value_child")
            .env("ARGX_TEST_OS_CHILD", "1")
            .env("ARGX_TEST_PATH", &expected)
            .output()
            .expect("child environment test should run");
        assert!(
            output.status.success(),
            "non-UTF-8 environment case failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        // Keep the byte representation exercised in the parent too so the intended fixture is
        // visibly non-UTF-8 rather than merely an opaque `OsString`.
        assert_eq!(expected.as_os_str().as_bytes(), b"path-\xff");
    }

    #[test]
    #[cfg(unix)]
    fn environment_os_value_child() {
        use std::os::unix::ffi::OsStrExt as _;

        if std::env::var_os("ARGX_TEST_OS_CHILD").is_none() {
            return;
        }
        let path = std::env::var_os("ARGX_TEST_PATH")
            .expect("OS environment child should receive its path value");
        let parsed = EnvironmentPathCli::try_parse_args(std::iter::empty::<&str>())
            .expect("OS environment value should bind");
        assert_eq!(
            parsed.path.as_deref().map(|value| value.as_os_str().as_bytes()),
            Some(path.as_os_str().as_bytes()),
        );
    }

    #[test]
    fn environment_sources_are_documented_in_help() {
        let help = EnvironmentCli::render_help();
        assert!(help.contains("--port <PORT>"));
        assert!(help.contains("[env: ARGX_TEST_PORT]"));
        assert!(help.contains("--profile <PROFILE>"));
        assert!(help.contains("[env: ARGX_TEST_PROFILE]"));
        assert!(!help.contains("Usage: environment --port"));

        let required = RequiredEnvironmentCli::render_help();
        assert!(!required.contains("Usage: required-environment-cli --port"));
        assert!(required.contains("[env: ARGX_TEST_REQUIRED_PORT]"));
    }

    #[test]
    fn subcommands_bind_unit_payload_and_nested_command_trees() {
        assert_eq!(
            SubcommandCli::try_parse_args([
                "--verbose",
                "acme",
                "add",
                "--dry-run",
                "--force",
                "widget",
            ]),
            Ok(SubcommandCli {
                verbose: true,
                workspace: String::from("acme"),
                command: RootCommand::Add(AddArgs {
                    shared: CommandShared { dry_run: true },
                    force: true,
                    name: String::from("widget"),
                }),
            }),
        );

        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "config", "get", "theme"]),
            Ok(SubcommandCli {
                verbose: false,
                workspace: String::from("acme"),
                command: RootCommand::Config(ConfigArgs {
                    local: false,
                    command: ConfigCommand::Get(GetArgs { key: String::from("theme") }),
                }),
            }),
        );

        assert_eq!(
            SubcommandCli::try_parse_args([
                "acme", "config", "--local", "set", "--raw", "theme", "dark",
            ]),
            Ok(SubcommandCli {
                verbose: false,
                workspace: String::from("acme"),
                command: RootCommand::Config(ConfigArgs {
                    local: true,
                    command: ConfigCommand::Set(SetArgs {
                        raw: true,
                        key: String::from("theme"),
                        value: String::from("dark"),
                    }),
                }),
            }),
        );

        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "show-status"]),
            Ok(SubcommandCli {
                verbose: false,
                workspace: String::from("acme"),
                command: RootCommand::Status,
            }),
        );
    }

    #[test]
    fn command_selection_is_exact_and_reports_missing_or_unknown_commands() {
        assert_eq!(
            SubcommandCli::try_parse_args(["acme"]),
            Err(Error::MissingSubcommand { name: "command" }),
        );
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "bogus"]),
            Err(Error::UnknownCommand { token: b"bogus".to_vec() }),
        );
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "conf"]),
            Err(Error::UnknownCommand { token: b"conf".to_vec() }),
        );
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "config"]),
            Err(Error::MissingSubcommand { name: "command" }),
        );
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "config", "bogus"]),
            Err(Error::UnknownCommand { token: b"bogus".to_vec() }),
        );
    }

    #[test]
    fn sibling_commands_may_reuse_flag_spellings_in_separate_scopes() {
        assert_eq!(
            SiblingCli::try_parse_args(["start", "--force"]),
            Ok(SiblingCli { command: SiblingCommand::Start(StartArgs { force: true }) }),
        );
        assert_eq!(
            SiblingCli::try_parse_args(["stop", "--force"]),
            Ok(SiblingCli { command: SiblingCommand::Stop(StopArgs { force: true }) }),
        );
    }

    #[test]
    fn descendant_help_uses_the_same_global_shadowing_as_parsing() {
        assert_eq!(
            GlobalCli::try_parse_args(["outer", "leaf", "--help"]),
            Err(Error::DisplayHelp {
                help: String::from(
                    "Usage: global-cli outer leaf [OPTIONS]\n\n\
Options:\n  --verbose\n  --region\n  --profile\n  -h, --help  Print help\n",
                ),
            }),
        );
    }

    #[test]
    fn globals_bind_to_their_declaring_fields_across_nested_commands() {
        for argv in [
            ["--profile", "dev", "outer", "--region", "eu", "leaf"],
            ["outer", "--profile", "dev", "--region", "eu", "leaf"],
            ["outer", "leaf", "--profile", "dev", "--region", "eu"],
        ] {
            assert_eq!(
                GlobalCli::try_parse_args(argv),
                Ok(GlobalCli {
                    common: GlobalCommon { verbose: false, profile: Some(String::from("dev")) },
                    command: GlobalCommand::Outer(GlobalOuterArgs {
                        region: Some(String::from("eu")),
                        command: GlobalNestedCommand::Leaf(GlobalLeafArgs { verbose: false }),
                    }),
                }),
            );
        }
    }

    #[test]
    fn local_arguments_shadow_inherited_globals_without_mirroring_binding() {
        assert_eq!(
            GlobalCli::try_parse_args(["outer", "leaf", "--verbose"]),
            Ok(GlobalCli {
                common: GlobalCommon { verbose: false, profile: None },
                command: GlobalCommand::Outer(GlobalOuterArgs {
                    region: None,
                    command: GlobalNestedCommand::Leaf(GlobalLeafArgs { verbose: true }),
                }),
            }),
        );
        assert_eq!(
            GlobalCli::try_parse_args(["--verbose", "outer", "leaf"]),
            Ok(GlobalCli {
                common: GlobalCommon { verbose: true, profile: None },
                command: GlobalCommand::Outer(GlobalOuterArgs {
                    region: None,
                    command: GlobalNestedCommand::Leaf(GlobalLeafArgs { verbose: false }),
                }),
            }),
        );
        assert_eq!(
            GlobalCli::try_parse_args(["--verbose", "outer", "leaf", "--verbose"]),
            Ok(GlobalCli {
                common: GlobalCommon { verbose: true, profile: None },
                command: GlobalCommand::Outer(GlobalOuterArgs {
                    region: None,
                    command: GlobalNestedCommand::Leaf(GlobalLeafArgs { verbose: true }),
                }),
            }),
        );
    }

    #[test]
    fn globals_are_downward_only_and_do_not_leak_to_siblings() {
        assert_eq!(
            GlobalCli::try_parse_args(["--region", "eu", "outer", "leaf"]),
            Err(Error::UnknownFlag { token: b"--region".to_vec() }),
        );
        assert_eq!(
            GlobalCli::try_parse_args(["other", "--region", "eu"]),
            Err(Error::UnknownFlag { token: b"--region".to_vec() }),
        );
    }

    #[test]
    fn scalar_global_occurrences_share_one_cardinality_across_command_boundaries() {
        assert_eq!(
            GlobalCli::try_parse_args([
                "--profile",
                "first",
                "outer",
                "leaf",
                "--profile",
                "second",
            ]),
            Err(Error::DuplicateArgument { name: "profile" }),
        );
    }

    #[test]
    fn selected_commands_own_all_following_parser_events() {
        assert_eq!(
            ReusedAcrossScopes::try_parse_args(["--value=root", "child", "--value=child"]),
            Ok(ReusedAcrossScopes {
                root: ReusedArgs { value: Some(String::from("root")) },
                command: ReusedCommand::Child(ReusedArgs { value: Some(String::from("child")) }),
            }),
        );
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "add", "widget", "--verbose"]),
            Err(Error::UnknownFlag { token: b"--verbose".to_vec() }),
        );
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "--force", "add", "widget"]),
            Err(Error::UnknownFlag { token: b"--force".to_vec() }),
        );
    }

    #[test]
    fn child_syntax_errors_precede_deferred_parent_cardinality_errors() {
        assert_eq!(
            SubcommandCli::try_parse_args(["--verbose", "--verbose", "acme", "add", "--unknown",]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() }),
        );
    }

    #[test]
    fn separator_stops_command_selection_in_the_current_scope() {
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "--", "add"]),
            Err(Error::UnexpectedArgument { token: b"add".to_vec() }),
        );
        assert_eq!(
            SubcommandCli::try_parse_args(["acme", "add", "--", "--force"]),
            Ok(SubcommandCli {
                verbose: false,
                workspace: String::from("acme"),
                command: RootCommand::Add(AddArgs {
                    shared: CommandShared { dry_run: false },
                    force: false,
                    name: String::from("--force"),
                }),
            }),
        );
    }
}
