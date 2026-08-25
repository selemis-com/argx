//! End-to-end typed parser contract tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

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

    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct Empty;

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
}
