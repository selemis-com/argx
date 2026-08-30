//! Typed parser and binding contract tests.
//!
//! This layer starts from derive-generated declarations and owns the semantic transition from raw
//! parser events to Rust values: cardinality, conversion, flattening, typed defaults, constraints,
//! subcommand binding, globals, aliases, and error precedence. Exact executable stdout/stderr
//! policy belongs to `process.rs`; compiler rejection of invalid declarations belongs to `ui.rs`.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use std::{ffi::OsString, path::PathBuf, process::Command};

    use argx::{Error, Parser as _};

    fn root_help<P: argx::Parser>() -> String {
        match P::try_parse_from(["argx-test", "-h"]) {
            Err(Error::DisplayHelp { help }) => help,
            Ok(_) => panic!("help request unexpectedly parsed"),
            Err(error) => panic!("unexpected help error: {error:?}"),
        }
    }

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
        #[argx(long = "destination")]
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
    struct ParsedMany {
        #[argx(long)]
        numbers: Vec<u16>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct DelimitedMany {
        #[argx(long, delimited)]
        labels: Vec<String>,
        #[argx(long, delimited)]
        numbers: Option<Vec<u16>>,
    }

    #[derive(Debug, PartialEq, Eq, argx::ValueEnum)]
    enum OutputMode {
        HumanReadable,
        Json,
        Quiet,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "value-enum")]
    struct ValueEnumCli {
        /// Output mode.
        #[argx(long = "mode", value_enum)]
        output: Option<OutputMode>,
        #[argx(long, value_enum)]
        include: Vec<OutputMode>,
        /// Fallback mode.
        #[argx(value_enum)]
        fallback: OutputMode,
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

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct ProcessEntryCli {
        #[argx(long)]
        exact: bool,
        test: String,
    }

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
    #[argx(name = "aliases")]
    struct AliasCli {
        #[argx(long, alias = "colour", aliases = ["hue", "tone"])]
        color: Option<String>,
        #[argx(subcommand)]
        command: AliasCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum AliasCommand {
        #[argx(alias = "rm", aliases = ["delete", "del"])]
        Remove,
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
        #[argx(long = "destination", help = "Output path")]
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

    #[derive(Debug, PartialEq, Eq, argx::ValueEnum)]
    enum HelpFormat {
        Human,
        Json,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "help-detail")]
    struct HelpDetailCli {
        /// Output format.
        ///
        /// Controls how records are rendered to stdout.
        #[argx(long, value_enum, default = HelpFormat::Human)]
        format: HelpFormat,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct HelpConfig {
        /// Use local configuration.
        #[argx(long)]
        local: bool,
        /// Configuration key.
        key: String,
    }

    /// Inspect and modify widgets.
    ///
    /// Commands operate on the selected workspace.
    ///
    /// # Examples
    ///
    /// documented-help list
    /// documented-help get widget-1
    ///
    /// # Machine-readable usage
    ///
    /// Use `documented-help schema` to inspect command contracts.
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "documented-help")]
    struct DocumentedHelpCli {
        #[argx(long)]
        workspace: Option<String>,
    }

    /// Payload details used when the selectable variant owns the short summary.
    ///
    /// # Examples
    ///
    /// documented-subcommand run --force
    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct DocumentedPayload {
        #[argx(long)]
        force: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum DocumentedSubcommand {
        /// Run the documented operation.
        Run(DocumentedPayload),
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "documented-subcommand")]
    struct DocumentedSubcommandCli {
        #[argx(subcommand)]
        command: DocumentedSubcommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct GroupedOutputArgs {
        /// Emit structured output.
        #[argx(long, global)]
        json: bool,
        /// Select output fields.
        #[argx(long, global)]
        field: Vec<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum GroupedHelpCommand {
        /// Show current status.
        Status,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "grouped-help")]
    struct GroupedHelpCli {
        /// Output.
        #[argx(flatten)]
        output: GroupedOutputArgs,
        #[argx(subcommand)]
        command: GroupedHelpCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct GroupedInputArgs {
        /// Input file.
        input: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "grouped-positional")]
    struct GroupedPositionalCli {
        /// Input
        #[argx(flatten)]
        input: GroupedInputArgs,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ColorOutputArgs {
        /// Enable colored output.
        #[argx(long)]
        color: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct FormatOutputArgs {
        /// Select the output format.
        #[argx(long)]
        format: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "merged-groups")]
    struct MergedGroupCli {
        /// Output
        #[argx(flatten)]
        color: ColorOutputArgs,
        /// Output
        #[argx(flatten)]
        format: FormatOutputArgs,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct NestedGroupValues {
        /// Select the network interface.
        #[argx(long)]
        interface: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct NestedGroupArgs {
        /// Network
        #[argx(flatten)]
        values: NestedGroupValues,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "propagated-group")]
    struct PropagatedGroupCli {
        #[argx(flatten)]
        nested: NestedGroupArgs,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "overridden-group")]
    struct OverriddenGroupCli {
        /// Runtime
        #[argx(flatten)]
        nested: NestedGroupArgs,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ReusedGroupedArgs {
        /// Shared setting.
        #[argx(long, global)]
        shared: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ReusedGroupedLeaf {
        #[argx(flatten)]
        shared: ReusedGroupedArgs,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum ReusedGroupedCommand {
        Leaf(ReusedGroupedLeaf),
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    #[argx(name = "reused-group")]
    struct ReusedGroupedCli {
        /// Root settings
        #[argx(flatten)]
        shared: ReusedGroupedArgs,
        #[argx(subcommand)]
        command: ReusedGroupedCommand,
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

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct CountCli {
        #[argx(short = 'v', long, count, default = 3)]
        verbosity: u8,
        #[argx(short = 'q', long)]
        quiet: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct GlobalCountCli {
        #[argx(short = 'v', long, count, global)]
        verbosity: u8,
        #[argx(subcommand)]
        command: GlobalCountCommand,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum GlobalCountCommand {
        Run,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct DefaultShared {
        #[argx(long, default = 4_u16)]
        jobs: u16,
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

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct ConstraintCli {
        #[argx(long, requires = ["token", "format"])]
        endpoint: Option<String>,
        #[argx(long = "auth-token", alias = "token")]
        token: Option<String>,
        #[argx(long = "destination", conflicts = ["stdout", "mode"])]
        output: Option<PathBuf>,
        #[argx(long)]
        stdout: bool,
        #[argx(long, default = String::from("json"))]
        format: String,
        #[argx(long, requires = "format")]
        schema: Option<PathBuf>,
        #[argx(long, default = String::from("auto"), requires = "token", conflicts = "stdout")]
        mode: String,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ConstraintShared {
        #[argx(long)]
        token: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct FlattenedConstraintCli {
        #[argx(long, requires = "token")]
        endpoint: Option<String>,
        #[argx(flatten)]
        shared: ConstraintShared,
    }

    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct ConstraintChildArgs {
        #[argx(long, conflicts = "quiet")]
        verbose: bool,
        #[argx(long)]
        quiet: bool,
    }

    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum ConstraintCommand {
        Run(ConstraintChildArgs),
    }

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct ConstraintSubcommandCli {
        #[argx(subcommand)]
        command: ConstraintCommand,
    }

    #[test]
    fn parses_typed_switches_values_and_positionals() {
        let parsed = Cli::try_parse_from([
            "argx-test",
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
    fn counted_flags_bind_occurrences_and_use_defaults_only_when_absent() {
        assert_eq!(
            CountCli::try_parse_from(["argx-test"]),
            Ok(CountCli { verbosity: 3, quiet: false })
        );
        assert_eq!(
            CountCli::try_parse_from(["argx-test", "-v"]),
            Ok(CountCli { verbosity: 1, quiet: false })
        );
        assert_eq!(
            CountCli::try_parse_from(["argx-test", "-vvv"]),
            Ok(CountCli { verbosity: 3, quiet: false })
        );
        assert_eq!(
            CountCli::try_parse_from(["argx-test", "--verbosity", "--verbosity"]),
            Ok(CountCli { verbosity: 2, quiet: false })
        );
        assert_eq!(
            CountCli::try_parse_from(["argx-test", "-vvq"]),
            Ok(CountCli { verbosity: 2, quiet: true })
        );
    }

    #[test]
    fn counted_flags_saturate_at_u8_max() {
        let flags = std::iter::repeat_n("-v", usize::from(u8::MAX) + 1);
        assert_eq!(
            CountCli::try_parse_from(std::iter::once("argx-test").chain(flags)),
            Ok(CountCli { verbosity: u8::MAX, quiet: false })
        );
    }

    #[test]
    fn global_counted_flags_accumulate_across_command_boundaries() {
        assert_eq!(
            GlobalCountCli::try_parse_from(["argx-test", "-v", "run", "-vv"]),
            Ok(GlobalCountCli { verbosity: 3, command: GlobalCountCommand::Run })
        );
    }

    #[test]
    fn complete_argv_entry_point_discards_only_argv0() {
        let parsed = Cli::try_parse_from(["argx-test", "input.txt"]).expect("valid argv");
        assert_eq!(parsed.input, "input.txt");
    }

    #[test]
    fn empty_complete_argv_is_an_empty_argument_list() {
        assert_eq!(Empty::try_parse_from(std::iter::empty::<&str>()), Ok(Empty));
        assert_eq!(Empty::parse_from(["argx-test"]), Empty);
    }

    #[test]
    fn process_parser_entry_points_use_the_current_process_arguments() {
        let executable = std::env::current_exe().expect("test executable should be available");
        let output = Command::new(executable)
            .arg("--exact")
            .arg("tests::process_parser_entry_points_child")
            .env("ARGX_TEST_PROCESS_ENTRY", "1")
            .output()
            .expect("process entry-point child should run");
        assert!(
            output.status.success(),
            "process entry-point child failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    fn process_parser_entry_points_child() {
        if std::env::var_os("ARGX_TEST_PROCESS_ENTRY").is_none() {
            return;
        }

        let tried = ProcessEntryCli::try_parse().expect("current process arguments should parse");
        assert!(tried.exact);
        assert_eq!(tried.test, "tests::process_parser_entry_points_child");

        let parsed = ProcessEntryCli::parse();
        assert!(parsed.exact);
        assert_eq!(parsed.test, "tests::process_parser_entry_points_child");
    }

    #[test]
    fn missing_required_values_are_reported_during_finalization() {
        assert_eq!(
            Cli::try_parse_from(["argx-test"]),
            Err(Error::MissingRequired { name: "input" })
        );
        assert_eq!(
            RequiredFlag::try_parse_from(["argx-test"]),
            Err(Error::MissingRequired { name: "--destination" })
        );
    }

    #[test]
    fn scalar_occurrences_are_strict_and_collections_repeat() {
        assert_eq!(
            Cli::try_parse_from(["argx-test", "--port", "1", "--port", "2", "input"]),
            Err(Error::DuplicateArgument { name: "--port" })
        );
        assert_eq!(
            Cli::try_parse_from(["argx-test", "-v", "--verbose", "input"]),
            Err(Error::DuplicateArgument { name: "--verbose" })
        );
        assert_eq!(
            Cli::try_parse_from(["argx-test", "--port", "not-a-port", "--port", "2", "input"]),
            Err(Error::DuplicateArgument { name: "--port" })
        );

        let parsed =
            Cli::try_parse_from(["argx-test", "--define", "one", "--define", "two", "input"])
                .expect("repeatable value flag");
        assert_eq!(parsed.define, vec![String::from("one"), String::from("two")]);
    }

    #[test]
    fn delimited_collections_split_commas_and_accumulate_repeated_occurrences() {
        let parsed = DelimitedMany::try_parse_from([
            "tool",
            "--labels",
            "alpha,beta",
            "--labels",
            "gamma",
            "--numbers",
            "1,2",
            "--numbers",
            "3",
        ])
        .unwrap();

        assert_eq!(parsed.labels, ["alpha", "beta", "gamma"]);
        assert_eq!(parsed.numbers, Some(vec![1, 2, 3]));
    }

    #[test]
    fn raw_syntax_errors_take_precedence_over_deferred_cardinality_errors() {
        assert_eq!(
            Cli::try_parse_from(["argx-test", "--port", "1", "--port", "2", "--unknown"]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() })
        );
    }

    #[test]
    fn raw_parser_failures_are_exposed_as_owned_public_errors() {
        assert_eq!(
            Empty::try_parse_from(["argx-test", "--unknown"]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() })
        );
        assert_eq!(
            Cli::try_parse_from(["argx-test", "--port"]),
            Err(Error::MissingValue { name: "--port" })
        );
        assert_eq!(
            Cli::try_parse_from(["argx-test", "--verbose=true", "input"]),
            Err(Error::UnexpectedValue { name: "--verbose" })
        );
        assert_eq!(
            Empty::try_parse_from(["argx-test", "extra"]),
            Err(Error::UnexpectedArgument { token: b"extra".to_vec() })
        );
    }

    #[test]
    fn typed_conversion_reports_the_field_and_bad_value() {
        let error = Cli::try_parse_from(["argx-test", "--port", "not-a-port", "input"])
            .expect_err("invalid integer must fail");
        match error {
            Error::InvalidValue { name, value, reason } => {
                assert_eq!(name, "--port");
                assert_eq!(value, "not-a-port");
                assert!(!reason.is_empty());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bool_positionals_are_values_not_switches() {
        assert_eq!(
            BoolPositional::try_parse_from(["argx-test", "true"]),
            Ok(BoolPositional { enabled: true })
        );
        assert_eq!(
            BoolPositional::try_parse_from(["argx-test", "false"]),
            Ok(BoolPositional { enabled: false })
        );
    }

    #[test]
    fn explicit_value_policies_reach_the_raw_parser() {
        assert_eq!(
            NegativeValues::try_parse_from(["argx-test", "--number", "-12", "-7"]),
            Ok(NegativeValues { number: Some(-12), positional: Some(-7) })
        );
        assert_eq!(
            HyphenValue::try_parse_from(["argx-test", "--raw", "--not-a-flag"]),
            Ok(HyphenValue { raw: Some(String::from("--not-a-flag")) })
        );
    }

    #[test]
    fn optional_collection_preserves_absence_and_empty_values() {
        assert_eq!(OptionalMany::try_parse_from(["argx-test"]), Ok(OptionalMany { tags: None }));
        assert_eq!(
            OptionalMany::try_parse_from(["argx-test", "--tags="]),
            Ok(OptionalMany { tags: Some(vec![String::new()]) })
        );
    }

    #[test]
    fn value_enums_share_one_vocabulary_across_parsing_and_help() {
        assert_eq!("human-readable".parse::<OutputMode>(), Ok(OutputMode::HumanReadable),);
        assert!("HumanReadable".parse::<OutputMode>().is_err());

        let parsed = ValueEnumCli::try_parse_from([
            "argx-test",
            "--mode",
            "json",
            "--include",
            "human-readable",
            "--include",
            "quiet",
            "quiet",
        ])
        .expect("canonical enum values should parse");
        assert_eq!(parsed.output, Some(OutputMode::Json));
        assert_eq!(parsed.include, vec![OutputMode::HumanReadable, OutputMode::Quiet]);
        assert_eq!(parsed.fallback, OutputMode::Quiet);

        let expected = <OutputMode as argx::ValueEnum>::VALUES;
        assert_eq!(expected, &["human-readable", "json", "quiet"]);
        let command = <ValueEnumCli as argx::__private::CommandArgs>::COMMAND;
        let &[output, include, ..] = command.flags else {
            panic!("expected output and include value-enum flags");
        };
        let &[fallback, ..] = command.args else {
            panic!("expected fallback value-enum positional");
        };
        assert_eq!(output.accepted_values, expected);
        assert_eq!(include.accepted_values, expected);
        assert_eq!(fallback.accepted_values, expected);

        let help = root_help::<ValueEnumCli>();
        assert!(help.contains("Output mode. [possible values: human-readable, json, quiet]"));
        assert!(help.contains("[possible values: human-readable, json, quiet]"));

        let error = ValueEnumCli::try_parse_from(["argx-test", "--mode", "yaml", "quiet"])
            .expect_err("unknown enum spelling must fail");
        let Error::InvalidValue { name, value, reason } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(name, "--mode");
        assert_eq!(value, "yaml");
        assert_eq!(reason, "expected one of: human-readable, json, quiet");
    }

    #[test]
    fn repeated_parsed_values_convert_every_occurrence_and_report_the_first_failure() {
        assert_eq!(
            ParsedMany::try_parse_from(["argx-test", "--numbers", "10", "--numbers=20"]),
            Ok(ParsedMany { numbers: vec![10, 20] }),
        );

        let error = ParsedMany::try_parse_from([
            "argx-test",
            "--numbers",
            "10",
            "--numbers",
            "not-a-number",
            "--numbers",
            "30",
        ])
        .expect_err("the first invalid repeated value must fail conversion");
        let Error::InvalidValue { name, value, reason } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(name, "--numbers");
        assert_eq!(value, "not-a-number");
        assert!(!reason.is_empty());
    }

    #[test]
    fn flattened_args_compose_recursively_and_preserve_positional_order() {
        let parsed = FlattenedCli::try_parse_from([
            "argx-test",
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
            EmptyFlatten::try_parse_from(["argx-test", "value"]),
            Ok(EmptyFlatten { empty: EmptyArgs, value: String::from("value") })
        );
    }

    #[test]
    fn identical_flattened_declarations_route_by_their_own_keys() {
        assert_eq!(
            IdenticalFlattened::try_parse_from(["argx-test", "first", "second"]),
            Ok(IdenticalFlattened {
                first: identical_a::Values { value: String::from("first") },
                second: identical_b::Values { value: String::from("second") },
            })
        );
    }

    #[test]
    fn flattened_checks_preserve_global_error_precedence() {
        assert_eq!(
            FlattenPrecedence::try_parse_from([
                "argx-test",
                "--duplicate=not-a-number",
                "--duplicate=2",
            ]),
            Err(Error::DuplicateArgument { name: "--duplicate" })
        );
        assert_eq!(
            FlattenPrecedence::try_parse_from([
                "argx-test",
                "--duplicate=not-a-number",
                "--duplicate=2",
                "--unknown",
            ]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() })
        );
        assert_eq!(
            FlattenPrecedence::try_parse_from(["argx-test"]),
            Err(Error::MissingRequired { name: "--required" })
        );
    }

    #[cfg(unix)]
    #[test]
    fn os_backed_fields_preserve_non_utf8_values() {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        let raw = OsString::from_vec(vec![b'p', 0xff, b't']);
        let parsed = PathCli::try_parse_from([OsString::from("argx-test"), raw.clone()])
            .expect("valid Unix path bytes");
        assert_eq!(parsed.path.as_os_str().as_bytes(), raw.as_os_str().as_bytes());

        let parsed = OsStringCli::try_parse_from([OsString::from("argx-test"), raw.clone()])
            .expect("valid Unix OS string");
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
        let parsed = Attached::try_parse_from([OsString::from("argx-test"), raw])
            .expect("valid Unix path bytes");
        assert_eq!(parsed.path.as_os_str().as_bytes(), &[0xff]);
    }

    #[cfg(unix)]
    #[test]
    fn text_fields_reject_non_utf8_without_lossy_conversion() {
        use std::os::unix::ffi::OsStringExt as _;

        let raw = OsString::from_vec(vec![b'x', 0xff]);
        assert_eq!(
            TextCli::try_parse_from([OsString::from("argx-test"), raw]),
            Err(Error::InvalidUtf8 { name: "value", value: vec![b'x', 0xff] })
        );
    }

    #[test]
    fn generated_help_uses_static_metadata_and_selected_command_scope() {
        let root = root_help::<HelpCli>();
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            root.as_str(),
            snapbox::str![[r#"
Manage things

Usage: tool [OPTIONS] --destination <OUTPUT> <WORKSPACE> <COMMAND>

Arguments:
  <WORKSPACE>  Workspace name.

Commands:
  config  Configure values.
  status  Show current status.

Options:
  -v, --verbose               Enable verbose output.
      --destination <OUTPUT>  Output path
  -h, --help                  Print help (see more with '--help')

"#]],
        );

        let nested =
            HelpCli::try_parse_from(["argx-test", "--destination", "out", "acme", "config", "-h"]);
        let Err(Error::DisplayHelp { help }) = nested else {
            panic!("nested help request did not return generated help")
        };
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            help,
            snapbox::str![[r#"
Configure values.

Usage: tool --destination <OUTPUT> <WORKSPACE> config [OPTIONS] <KEY>

Arguments:
  <KEY>  Configuration key.

Options:
      --local  Use local configuration.
  -h, --help   Print help (see more with '--help')

"#]],
        );

        let status =
            HelpCli::try_parse_from(["argx-test", "--destination", "out", "acme", "status"])
                .expect("status command should parse");
        assert!(!status.verbose);
        assert_eq!(status.output, "out");
        assert_eq!(status.workspace, "acme");
        assert!(matches!(status.command, HelpCommand::Status));

        let config = HelpCli::try_parse_from([
            "argx-test",
            "--destination",
            "out",
            "acme",
            "config",
            "--local",
            "theme",
        ])
        .expect("config command should parse");
        let HelpCommand::Config(config) = config.command else {
            panic!("config command did not construct its payload")
        };
        assert!(config.local);
        assert_eq!(config.key, "theme");
    }

    #[test]
    fn short_and_long_help_use_distinct_detail_levels() {
        let short = match HelpDetailCli::try_parse_from(["argx-test", "-h"]) {
            Err(Error::DisplayHelp { help }) => help,
            result => panic!("unexpected short help result: {result:?}"),
        };
        assert!(short.contains(
            "--format <FORMAT>  Output format. [possible values: human, json] [default: human]"
        ));
        assert!(short.contains("Print help (see more with '--help')"));
        assert!(!short.contains("Controls how records are rendered to stdout."));

        let long = match HelpDetailCli::try_parse_from(["argx-test", "--help"]) {
            Err(Error::DisplayHelp { help }) => help,
            result => panic!("unexpected long help result: {result:?}"),
        };
        assert!(long.contains("Controls how records are rendered to stdout."));
        assert!(long.contains("Possible values:\n          - human\n          - json"));
        assert!(long.contains("[default: human]"));
        assert!(long.contains("Print help (see a summary with '-h')"));

        let commands = match HelpCli::try_parse_from(["argx-test", "--help"]) {
            Err(Error::DisplayHelp { help }) => help,
            result => panic!("unexpected long command help result: {result:?}"),
        };
        assert!(
            commands.contains(
                "Commands:\n  config  Configure values.\n  status  Show current status.\n"
            )
        );
    }

    #[test]
    fn generated_help_renders_structured_sections_from_rust_docs() {
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            root_help::<DocumentedHelpCli>(),
            snapbox::str![[r#"
Inspect and modify widgets.

Usage: documented-help [OPTIONS]

Options:
      --workspace <WORKSPACE>
  -h, --help                   Print help (see more with '--help')

Examples:
documented-help list
documented-help get widget-1

Machine-readable usage:
Use `documented-help schema` to inspect command contracts.

"#]],
        );
    }

    #[test]
    fn subcommand_help_inherits_documented_sections_from_its_payload() {
        let error = DocumentedSubcommandCli::try_parse_from(["argx-test", "run", "--help"])
            .expect_err("help should stop before constructing the payload");
        let Error::DisplayHelp { help } = error else {
            panic!("expected generated help");
        };
        assert!(help.starts_with("Run the documented operation.\n\n"));
        assert!(help.contains("\nExamples:\ndocumented-subcommand run --force\n"));
    }

    #[test]
    fn flatten_field_docs_group_arguments_without_renderer_insertion_metadata() {
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            root_help::<GroupedHelpCli>(),
            snapbox::str![[r#"
Usage: grouped-help [OPTIONS] <COMMAND>

Commands:
  status  Show current status.

Options:
  -h, --help  Print help (see more with '--help')

Output:
      --json           Emit structured output.
      --field <FIELD>  Select output fields.

"#]],
        );
    }

    #[test]
    fn flattened_positional_arguments_move_into_their_documented_group() {
        let help = root_help::<GroupedPositionalCli>();
        assert!(help.contains("\nInput:\n  <INPUT>  Input file.\n"));
        assert!(!help.contains("\nArguments:\n"));
    }

    #[test]
    fn flattened_groups_with_the_same_heading_merge_into_one_section() {
        let help = root_help::<MergedGroupCli>();
        assert_eq!(help.matches("\nOutput:\n").count(), 1);
        assert!(help.contains("  --color"));
        assert!(help.contains("  --format <FORMAT>"));
        assert!(!help.contains("Options:\n      --color"));
    }

    #[test]
    fn inherited_global_arguments_keep_their_documented_group_in_subcommand_help() {
        let error = GroupedHelpCli::try_parse_from(["argx-test", "status", "--help"])
            .expect_err("help should stop before constructing the command");
        let Error::DisplayHelp { help } = error else {
            panic!("expected generated help");
        };
        assert!(help.contains("\nOutput:\n      --json"));
        assert!(help.contains("  --field <FIELD>"));
        assert!(!help.contains("Options:\n      --json"));
    }

    #[test]
    fn undocumented_flattening_propagates_nested_groups_and_documented_flattening_overrides_them() {
        let propagated = root_help::<PropagatedGroupCli>();
        assert!(propagated.contains("\nNetwork:\n      --interface <INTERFACE>"));

        let overridden = root_help::<OverriddenGroupCli>();
        assert!(overridden.contains("\nRuntime:\n      --interface <INTERFACE>"));
        assert!(!overridden.contains("\nNetwork:\n"));
    }

    #[test]
    fn reused_flattened_args_are_grouped_only_by_their_visible_scope() {
        let error = ReusedGroupedCli::try_parse_from(["argx-test", "leaf", "--help"])
            .expect_err("help should stop before constructing the command");
        let Error::DisplayHelp { help } = error else {
            panic!("expected generated help");
        };
        assert!(help.contains("Options:\n      --shared <SHARED>"));
        assert!(!help.contains("Root settings:"));
    }

    #[test]
    fn help_precedes_deferred_binding_errors_but_not_detached_value_rules() {
        assert!(matches!(
            HelpCli::try_parse_from(["argx-test", "--verbose", "--verbose", "--help"]),
            Err(Error::DisplayHelp { .. })
        ));
        assert_eq!(
            HelpCli::try_parse_from(["argx-test", "--destination", "--help"]),
            Err(Error::MissingValue { name: "--destination" }),
        );
        assert_eq!(
            Empty::try_parse_from(["argx-test", "--", "--help"]),
            Err(Error::UnexpectedArgument { token: b"--help".to_vec() }),
        );
    }

    #[test]
    fn version_metadata_is_scoped_to_the_selected_command() {
        assert_eq!(
            MetadataCli::try_parse_from(["argx-test", "-V"]),
            Err(Error::DisplayVersion { version: String::from("meta 1.2.3\n") }),
        );
        assert_eq!(
            MetadataCli::try_parse_from(["argx-test", "--version"]),
            Err(Error::DisplayVersion { version: String::from("meta 1.2.3 (build abc123)\n") }),
        );
        assert_eq!(
            MetadataCli::try_parse_from(["argx-test", "--version=value"]),
            Err(Error::UnexpectedValue { name: "--version" }),
        );
        assert_eq!(
            MetadataCli::try_parse_from(["argx-test", "run", "--version"]),
            Err(Error::DisplayVersion { version: String::from("run 1.2.3 (build abc123)\n") }),
        );
        assert_eq!(
            MetadataCli::try_parse_from(["argx-test", "internal", "--version"]),
            Err(Error::UnknownFlag { token: b"--version".to_vec() }),
        );
        assert_eq!(
            MetadataCli::try_parse_from(["argx-test", "internal"]),
            Ok(MetadataCli { command: MetadataCommand::Internal }),
        );
        assert_eq!(
            ShortVersionOnly::try_parse_from(["argx-test", "--version"]),
            Err(Error::DisplayVersion { version: String::from("short-only 1.2.3\n") }),
        );
        assert_eq!(
            LongVersionOnly::try_parse_from(["argx-test", "-V"]),
            Err(Error::DisplayVersion {
                version: String::from("long-only 1.2.3 (build abc123)\n"),
            }),
        );
        assert_eq!(
            UserVersionFlag::try_parse_from(["argx-test", "--version"]),
            Ok(UserVersionFlag { version: true }),
        );
        assert_eq!(
            UserVersionFlag::try_parse_from(["argx-test", "-V"]),
            Ok(UserVersionFlag { version: true }),
        );
    }

    #[test]
    fn typed_defaults_fill_absent_scalar_flags_and_argv_wins() {
        assert_eq!(
            DefaultCli::try_parse_from(["argx-test"]),
            Ok(DefaultCli { port: 3000, profile: Some(String::from("development")) }),
        );
        assert_eq!(
            DefaultCli::try_parse_from(["argx-test", "--port", "8080", "--profile", "production"]),
            Ok(DefaultCli { port: 8080, profile: Some(String::from("production")) }),
        );
        assert_eq!(
            DefaultCli::try_parse_from(["argx-test", "--port", "1", "--port", "2"]),
            Err(Error::DuplicateArgument { name: "--port" }),
        );
        assert_eq!(
            FlattenedDefaults::try_parse_from(["argx-test"]),
            Ok(FlattenedDefaults { shared: DefaultShared { jobs: 4 } }),
        );
    }

    #[test]
    fn typed_defaults_do_not_repair_invalid_or_incomplete_argv() {
        assert_eq!(
            DefaultCli::try_parse_from(["argx-test", "--port"]),
            Err(Error::MissingValue { name: "--port" }),
        );

        let error = DefaultCli::try_parse_from(["argx-test", "--port", "not-a-port"])
            .expect_err("an explicit invalid value must not fall back to the default");
        let Error::InvalidValue { name, value, .. } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(name, "--port");
        assert_eq!(value, "not-a-port");
    }

    #[test]
    fn typed_defaults_make_bare_value_flags_optional_in_help() {
        let help = root_help::<DefaultCli>();
        assert!(help.contains("Usage: defaults [OPTIONS]"));
        assert!(!help.contains("Usage: defaults --port"));
        assert!(help.contains("--port <PORT>"));
    }

    #[test]
    fn subcommands_bind_unit_payload_and_nested_command_trees() {
        assert_eq!(
            SubcommandCli::try_parse_from([
                "argx-test",
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
            SubcommandCli::try_parse_from(["argx-test", "acme", "config", "get", "theme"]),
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
            SubcommandCli::try_parse_from([
                "argx-test",
                "acme",
                "config",
                "--local",
                "set",
                "--raw",
                "theme",
                "dark",
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
            SubcommandCli::try_parse_from(["argx-test", "acme", "show-status"]),
            Ok(SubcommandCli {
                verbose: false,
                workspace: String::from("acme"),
                command: RootCommand::Status,
            }),
        );
    }

    #[test]
    fn hidden_aliases_parse_without_changing_canonical_help() {
        for flag in ["--color", "--colour", "--hue", "--tone"] {
            assert_eq!(
                AliasCli::try_parse_from(["argx-test", flag, "blue", "rm"]),
                Ok(AliasCli { color: Some(String::from("blue")), command: AliasCommand::Remove }),
            );
        }

        for command in ["remove", "rm", "delete", "del"] {
            assert_eq!(
                AliasCli::try_parse_from(["argx-test", command]),
                Ok(AliasCli { color: None, command: AliasCommand::Remove }),
            );
        }

        assert_eq!(
            AliasCli::try_parse_from(["argx-test", "-h"]),
            Err(Error::DisplayHelp {
                help: String::from(
                    "Usage: aliases [OPTIONS] <COMMAND>\n\n\
Commands:\n  remove\n  status\n\n\
Options:\n      --color <COLOR>\n  -h, --help           Print help (see more with '--help')\n",
                ),
            }),
        );
    }

    #[test]
    fn requires_and_conflicts_use_semantic_argument_identity() {
        assert_eq!(
            ConstraintCli::try_parse_from(["argx-test", "--endpoint", "https://example.test"]),
            Err(Error::MissingRequirement { name: "--auth-token", required_by: "--endpoint" }),
        );
        assert_eq!(
            ConstraintCli::try_parse_from([
                "argx-test",
                "--endpoint",
                "https://example.test",
                "--token",
                "secret",
            ]),
            Ok(ConstraintCli {
                endpoint: Some(String::from("https://example.test")),
                token: Some(String::from("secret")),
                output: None,
                stdout: false,
                format: String::from("json"),
                schema: None,
                mode: String::from("auto"),
            }),
        );

        for args in
            [["--destination", "out.txt", "--stdout"], ["--stdout", "--destination", "out.txt"]]
        {
            assert_eq!(
                ConstraintCli::try_parse_from(std::iter::once("argx-test").chain(args)),
                Err(Error::ConflictingArguments { name: "--destination", other: "--stdout" }),
            );
        }

        assert_eq!(
            ConstraintCli::try_parse_from([
                "argx-test",
                "--destination",
                "out.txt",
                "--mode",
                "manual"
            ]),
            Err(Error::ConflictingArguments { name: "--destination", other: "--mode" }),
        );
    }

    #[test]
    fn defaults_satisfy_requirements_but_do_not_activate_or_conflict() {
        assert_eq!(
            ConstraintCli::try_parse_from(["argx-test", "--schema", "schema.json"]),
            Ok(ConstraintCli {
                endpoint: None,
                token: None,
                output: None,
                stdout: false,
                format: String::from("json"),
                schema: Some(PathBuf::from("schema.json")),
                mode: String::from("auto"),
            }),
        );
        assert_eq!(
            ConstraintCli::try_parse_from(["argx-test"]),
            Ok(ConstraintCli {
                endpoint: None,
                token: None,
                output: None,
                stdout: false,
                format: String::from("json"),
                schema: None,
                mode: String::from("auto"),
            }),
        );
        assert_eq!(
            ConstraintCli::try_parse_from(["argx-test", "--stdout"]),
            Ok(ConstraintCli {
                endpoint: None,
                token: None,
                output: None,
                stdout: true,
                format: String::from("json"),
                schema: None,
                mode: String::from("auto"),
            }),
        );
    }

    #[test]
    fn constraints_resolve_across_flattening_and_selected_subcommands() {
        assert_eq!(
            FlattenedConstraintCli::try_parse_from([
                "argx-test",
                "--endpoint",
                "https://example.test"
            ]),
            Err(Error::MissingRequirement { name: "--token", required_by: "--endpoint" }),
        );
        assert_eq!(
            FlattenedConstraintCli::try_parse_from([
                "argx-test",
                "--endpoint",
                "https://example.test",
                "--token",
                "secret",
            ]),
            Ok(FlattenedConstraintCli {
                endpoint: Some(String::from("https://example.test")),
                shared: ConstraintShared { token: Some(String::from("secret")) },
            }),
        );
        assert_eq!(
            ConstraintSubcommandCli::try_parse_from(["argx-test", "run", "--verbose", "--quiet"]),
            Err(Error::ConflictingArguments { name: "--verbose", other: "--quiet" }),
        );
    }

    #[test]
    fn command_selection_is_exact_and_reports_missing_or_unknown_commands() {
        let root = SubcommandCli::try_parse_from(["argx-test", "acme"]);
        let Err(Error::DisplayHelp { .. }) = root else {
            panic!("missing root subcommand should display help: {root:?}");
        };
        assert_eq!(
            SubcommandCli::try_parse_from(["argx-test", "acme", "bogus"]),
            Err(Error::UnknownCommand { token: b"bogus".to_vec() }),
        );
        assert_eq!(
            SubcommandCli::try_parse_from(["argx-test", "acme", "conf"]),
            Err(Error::UnknownCommand { token: b"conf".to_vec() }),
        );
        let nested = SubcommandCli::try_parse_from(["argx-test", "acme", "config"]);
        let Err(Error::DisplayHelp { .. }) = nested else {
            panic!("missing nested subcommand should display help: {nested:?}");
        };
        assert_eq!(
            SubcommandCli::try_parse_from(["argx-test", "acme", "config", "bogus"]),
            Err(Error::UnknownCommand { token: b"bogus".to_vec() }),
        );
    }

    #[test]
    fn sibling_commands_may_reuse_flag_spellings_in_separate_scopes() {
        assert_eq!(
            SiblingCli::try_parse_from(["argx-test", "start", "--force"]),
            Ok(SiblingCli { command: SiblingCommand::Start(StartArgs { force: true }) }),
        );
        assert_eq!(
            SiblingCli::try_parse_from(["argx-test", "stop", "--force"]),
            Ok(SiblingCli { command: SiblingCommand::Stop(StopArgs { force: true }) }),
        );
    }

    #[test]
    fn descendant_help_uses_the_same_global_shadowing_as_parsing() {
        assert_eq!(
            GlobalCli::try_parse_from(["argx-test", "outer", "leaf", "-h"]),
            Err(Error::DisplayHelp {
                help: String::from(
                    "Usage: global-cli outer leaf [OPTIONS]\n\n\
Options:\n      --verbose\n      --region <REGION>\n      --profile <PROFILE>\n  -h, --help               Print help (see more with '--help')\n",
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
                GlobalCli::try_parse_from(std::iter::once("argx-test").chain(argv)),
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
            GlobalCli::try_parse_from(["argx-test", "outer", "leaf", "--verbose"]),
            Ok(GlobalCli {
                common: GlobalCommon { verbose: false, profile: None },
                command: GlobalCommand::Outer(GlobalOuterArgs {
                    region: None,
                    command: GlobalNestedCommand::Leaf(GlobalLeafArgs { verbose: true }),
                }),
            }),
        );
        assert_eq!(
            GlobalCli::try_parse_from(["argx-test", "--verbose", "outer", "leaf"]),
            Ok(GlobalCli {
                common: GlobalCommon { verbose: true, profile: None },
                command: GlobalCommand::Outer(GlobalOuterArgs {
                    region: None,
                    command: GlobalNestedCommand::Leaf(GlobalLeafArgs { verbose: false }),
                }),
            }),
        );
        assert_eq!(
            GlobalCli::try_parse_from(["argx-test", "--verbose", "outer", "leaf", "--verbose"]),
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
            GlobalCli::try_parse_from(["argx-test", "--region", "eu", "outer", "leaf"]),
            Err(Error::UnknownFlag { token: b"--region".to_vec() }),
        );
        assert_eq!(
            GlobalCli::try_parse_from(["argx-test", "other", "--region", "eu"]),
            Err(Error::UnknownFlag { token: b"--region".to_vec() }),
        );
    }

    #[test]
    fn scalar_global_occurrences_share_one_cardinality_across_command_boundaries() {
        assert_eq!(
            GlobalCli::try_parse_from([
                "argx-test",
                "--profile",
                "first",
                "outer",
                "leaf",
                "--profile",
                "second",
            ]),
            Err(Error::DuplicateArgument { name: "--profile" }),
        );
    }

    #[test]
    fn selected_commands_own_all_following_parser_events() {
        assert_eq!(
            ReusedAcrossScopes::try_parse_from([
                "argx-test",
                "--value=root",
                "child",
                "--value=child"
            ]),
            Ok(ReusedAcrossScopes {
                root: ReusedArgs { value: Some(String::from("root")) },
                command: ReusedCommand::Child(ReusedArgs { value: Some(String::from("child")) }),
            }),
        );
        assert_eq!(
            SubcommandCli::try_parse_from(["argx-test", "acme", "add", "widget", "--verbose"]),
            Err(Error::UnknownFlag { token: b"--verbose".to_vec() }),
        );
        assert_eq!(
            SubcommandCli::try_parse_from(["argx-test", "acme", "--force", "add", "widget"]),
            Err(Error::UnknownFlag { token: b"--force".to_vec() }),
        );
    }

    #[test]
    fn child_syntax_errors_precede_deferred_parent_cardinality_errors() {
        assert_eq!(
            SubcommandCli::try_parse_from([
                "argx-test",
                "--verbose",
                "--verbose",
                "acme",
                "add",
                "--unknown",
            ]),
            Err(Error::UnknownFlag { token: b"--unknown".to_vec() }),
        );
    }

    #[test]
    fn separator_stops_command_selection_in_the_current_scope() {
        assert_eq!(
            SubcommandCli::try_parse_from(["argx-test", "acme", "--", "add"]),
            Err(Error::UnexpectedArgument { token: b"add".to_vec() }),
        );
        assert_eq!(
            SubcommandCli::try_parse_from(["argx-test", "acme", "add", "--", "--force"]),
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
