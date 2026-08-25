//! Raw argv parser contract tests.

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use argx::__private::{Arg, ArgvParser, Command, Error, Event, Flag};

    static VERBOSE: Flag<'static> =
        Flag { key: 1, name: "verbose", longs: &["verbose"], shorts: b"v", ..Flag::BOOL };
    static FORCE: Flag<'static> =
        Flag { key: 2, name: "force", longs: &["force"], shorts: b"f", ..Flag::BOOL };
    static OUTPUT: Flag<'static> =
        Flag { key: 3, name: "output", longs: &["output"], shorts: b"o", ..Flag::VALUE };
    static DEFINE: Flag<'static> =
        Flag { key: 4, name: "define", longs: &["define"], shorts: b"D", ..Flag::VALUE };
    static INPUT: Arg<'static> = Arg { key: 5, name: "input", ..Arg::REQUIRED };
    static REST: Arg<'static> = Arg {
        key: 6,
        name: "rest",
        required: false,
        variadic: true,
        allow_negative_numbers: false,
    };
    static COMMAND: Command<'static> = Command {
        name: "example",
        flags: &[&VERBOSE, &FORCE, &OUTPUT, &DEFINE],
        args: &[&INPUT, &REST],
        ..Command::EMPTY
    };

    fn argv<'a>(values: &[&'a str]) -> Vec<&'a OsStr> {
        values.iter().map(|value| OsStr::new(*value)).collect()
    }

    #[test]
    fn parses_long_switches_and_value_forms() {
        for values in [&["--output", "result.txt"][..], &["--output=result.txt"][..]] {
            let argv = argv(values);
            let mut parser = ArgvParser::new(&COMMAND, &argv);
            assert_eq!(
                parser.next_event(),
                Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"result.txt") }))
            );
            assert_eq!(parser.next_event(), None);
        }

        let argv = argv(&["--verbose"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &VERBOSE, value: None })));
    }

    #[test]
    fn attached_long_values_preserve_empty_and_later_equals() {
        for (token, expected) in [("--output=", ""), ("--output=a=b", "a=b")] {
            let argv = argv(&[token]);
            let mut parser = ArgvParser::new(&COMMAND, &argv);
            assert_eq!(
                parser.next_event(),
                Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(expected.as_bytes()) })),
                "{token}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn attached_values_preserve_non_utf8_os_strings() {
        use std::os::unix::ffi::OsStrExt as _;

        let token = OsStr::from_bytes(b"--output=\xff");
        let argv = [token];
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"\xff") }))
        );
    }

    #[test]
    fn parses_short_bundles_and_attached_values() {
        let argv = argv(&["-vforesult.txt"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &VERBOSE, value: None })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &FORCE, value: None })));
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"result.txt") }))
        );
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn short_attached_value_strips_at_most_one_equals() {
        for (token, expected) in [("-o=value", "value"), ("-o==value", "=value")] {
            let argv = argv(&[token]);
            let mut parser = ArgvParser::new(&COMMAND, &argv);
            assert_eq!(
                parser.next_event(),
                Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(expected.as_bytes()) })),
                "{token}"
            );
        }
    }

    #[test]
    fn short_bundle_is_rejected_atomically() {
        let argv = argv(&["-vx"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-vx" })));
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn detached_value_refuses_flag_like_token_by_default() {
        let argv = argv(&["--output", "--verbose"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(parser.next_event(), Some(Err(Error::MissingFlagValue { flag: &OUTPUT })));
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn attached_value_can_be_flag_like_without_special_policy() {
        let argv = argv(&["--output=--verbose"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"--verbose") }))
        );
    }

    #[test]
    fn hyphen_value_policy_consumes_flag_like_detached_values() {
        static RAW: Flag<'static> = Flag {
            key: 20,
            name: "raw",
            longs: &["raw"],
            allow_hyphen_values: true,
            ..Flag::VALUE
        };
        static RAW_COMMAND: Command<'static> =
            Command { name: "raw", flags: &[&RAW, &VERBOSE], ..Command::EMPTY };

        let argv = argv(&["--raw", "--", "--verbose"]);
        let mut parser = ArgvParser::new(&RAW_COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &RAW, value: Some(b"--") })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &VERBOSE, value: None })));
    }

    #[test]
    fn hyphen_value_policy_applies_to_short_detached_values() {
        static RAW: Flag<'static> =
            Flag { key: 24, name: "raw", shorts: b"r", allow_hyphen_values: true, ..Flag::VALUE };
        static RAW_COMMAND: Command<'static> =
            Command { name: "raw", flags: &[&RAW], ..Command::EMPTY };

        let argv = argv(&["-r", "--unknown"]);
        let mut parser = ArgvParser::new(&RAW_COMMAND, &argv);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &RAW, value: Some(b"--unknown") }))
        );
    }

    #[test]
    fn negative_numbers_are_flag_like_without_an_explicit_policy() {
        let args = argv(&["--output", "-1"]);
        let mut parser = ArgvParser::new(&COMMAND, &args);
        assert_eq!(parser.next_event(), Some(Err(Error::MissingFlagValue { flag: &OUTPUT })));

        let argv = argv(&["-1"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-1" })));
    }

    #[test]
    fn negative_number_policy_is_narrow() {
        static NUMBER: Flag<'static> = Flag {
            key: 21,
            name: "number",
            longs: &["number"],
            allow_negative_numbers: true,
            ..Flag::VALUE
        };
        static NUMBER_COMMAND: Command<'static> =
            Command { name: "number", flags: &[&NUMBER], ..Command::EMPTY };

        let args = argv(&["--number", "-1.5e2"]);
        let mut parser = ArgvParser::new(&NUMBER_COMMAND, &args);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &NUMBER, value: Some(b"-1.5e2") }))
        );

        let argv = argv(&["--number", "-not-a-number"]);
        let mut parser = ArgvParser::new(&NUMBER_COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Err(Error::MissingFlagValue { flag: &NUMBER })));
    }

    #[test]
    fn positional_negative_number_policy_does_not_hide_declared_digit_short() {
        static PRINT0: Flag<'static> = Flag { key: 22, name: "print0", shorts: b"0", ..Flag::BOOL };
        static VALUE: Arg<'static> =
            Arg { key: 23, name: "value", allow_negative_numbers: true, ..Arg::REQUIRED };
        static NUMBER_COMMAND: Command<'static> =
            Command { name: "number", flags: &[&PRINT0], args: &[&VALUE], ..Command::EMPTY };

        let args = argv(&["-0"]);
        let mut parser = ArgvParser::new(&NUMBER_COMMAND, &args);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &PRINT0, value: None })));

        let argv = argv(&["-1"]);
        let mut parser = ArgvParser::new(&NUMBER_COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &VALUE, value: b"-1" })));
    }

    #[test]
    fn separator_stops_flag_interpretation() {
        let argv = argv(&["input.txt", "--", "--verbose", "-x"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &INPUT, value: b"input.txt" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &REST, value: b"--verbose" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &REST, value: b"-x" })));
    }

    #[test]
    fn separator_at_end_produces_no_event() {
        let argv = argv(&["--"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn second_separator_is_positional_data() {
        let argv = argv(&["--", "input.txt", "--"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &INPUT, value: b"input.txt" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &REST, value: b"--" })));
    }

    #[test]
    fn lone_dash_is_a_positional_value() {
        let argv = argv(&["-"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &INPUT, value: b"-" })));
    }

    #[test]
    fn positionals_bind_in_order_and_variadic_stays_current() {
        let argv = argv(&["input.txt", "one", "two"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &INPUT, value: b"input.txt" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &REST, value: b"one" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &REST, value: b"two" })));
    }

    #[test]
    fn repeated_flags_are_reported_as_independent_events() {
        let argv = argv(&["--define", "one", "-Dtwo"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &DEFINE, value: Some(b"one") }))
        );
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &DEFINE, value: Some(b"two") }))
        );
    }

    #[test]
    fn long_names_are_exact_and_switches_reject_attached_values() {
        let args = argv(&["--verb"]);
        let mut parser = ArgvParser::new(&COMMAND, &args);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"--verb" })));

        let argv = argv(&["--verbose=true"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Err(Error::UnexpectedFlagValue { flag: &VERBOSE })));
    }

    #[test]
    fn extra_positionals_are_rejected_and_errors_are_terminal() {
        static ONE: Command<'static> = Command { name: "one", args: &[&INPUT], ..Command::EMPTY };
        let argv = argv(&["one", "two", "three"]);
        let mut parser = ArgvParser::new(&ONE, &argv);

        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &INPUT, value: b"one" })));
        assert_eq!(parser.next_event(), Some(Err(Error::UnexpectedArg { token: b"two" })));
        assert_eq!(parser.next_event(), None);
    }
}
