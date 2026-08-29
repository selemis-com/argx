//! Raw argv parser contract tests.
//!
//! This layer owns lexical behavior only: token classification, short bundles, attached and
//! detached values, separators, command selection, built-in actions, and lexical scope. It works
//! directly with static runtime metadata and intentionally does not exercise derive-generated typed
//! binding, environment/default resolution, or rendered public diagnostics.

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use argx::__private::{
        Action, ActionKind, Arg, ArgvParser, Command, Error, Event, Flag, HELP_ACTION,
    };

    static VERBOSE: Flag<'static> =
        Flag { key: 1, name: "verbose", longs: &["verbose"], shorts: b"v", ..Flag::BOOL };
    static FORCE: Flag<'static> =
        Flag { key: 2, name: "force", longs: &["force"], shorts: b"f", ..Flag::BOOL };
    static OUTPUT: Flag<'static> =
        Flag { key: 3, name: "path", longs: &["path"], shorts: b"o", ..Flag::VALUE };
    static DEFINE: Flag<'static> =
        Flag { key: 4, name: "define", longs: &["define"], shorts: b"D", ..Flag::VALUE };
    static INPUT: Arg<'static> = Arg { key: 5, name: "input", ..Arg::REQUIRED };
    static REST: Arg<'static> = Arg {
        key: 6,
        name: "rest",
        help: None,
        required: false,
        variadic: true,
        accepted_values: &[],
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
        for values in [&["--path", "result.txt"][..], &["--path=result.txt"][..]] {
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
        for (token, expected) in [("--path=", ""), ("--path=a=b", "a=b")] {
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

        let token = OsStr::from_bytes(b"--path=\xff");
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
        let argv = argv(&["--path", "--verbose"]);
        let mut parser = ArgvParser::new(&COMMAND, &argv);

        assert_eq!(parser.next_event(), Some(Err(Error::MissingFlagValue { flag: &OUTPUT })));
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn attached_value_can_be_flag_like_without_special_policy() {
        let argv = argv(&["--path=--verbose"]);
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
        let args = argv(&["--path", "-1"]);
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
    fn multi_digit_negative_numbers_remain_positional_with_digit_short_flags() {
        static PRINT1: Flag<'static> = Flag { key: 24, name: "print1", shorts: b"1", ..Flag::BOOL };
        static PRINT2: Flag<'static> = Flag { key: 25, name: "print2", shorts: b"2", ..Flag::BOOL };
        static VALUE: Arg<'static> =
            Arg { key: 26, name: "value", allow_negative_numbers: true, ..Arg::REQUIRED };
        static NUMBER_COMMAND: Command<'static> = Command {
            name: "number",
            flags: &[&PRINT1, &PRINT2],
            args: &[&VALUE],
            ..Command::EMPTY
        };

        let argv = argv(&["-12"]);
        let mut parser = ArgvParser::new(&NUMBER_COMMAND, &argv);
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &VALUE, value: b"-12" })));
        assert_eq!(parser.next_event(), None);
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
    fn built_in_help_is_terminal_scope_control_and_respects_separator_rules() {
        let values = argv(&["--help"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Action { action: &HELP_ACTION, long: true }))
        );
        assert_eq!(parser.next_event(), None);

        let values = argv(&["-vh"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &VERBOSE, value: None })));
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Action { action: &HELP_ACTION, long: false }))
        );
        assert_eq!(parser.next_event(), None);

        let values = argv(&["--help=value"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        let Some(Err(Error::UnexpectedActionValue { action })) = parser.next_event() else {
            panic!("attached help value did not produce the expected error")
        };
        assert_eq!(action.name, "help");

        let values = argv(&["--", "--help"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &INPUT, value: b"--help" })));

        static RAW: Flag<'static> = Flag {
            key: 29,
            name: "raw",
            longs: &["raw"],
            allow_hyphen_values: true,
            ..Flag::VALUE
        };
        static RAW_COMMAND: Command<'static> =
            Command { name: "raw", flags: &[&RAW], ..Command::EMPTY };
        let values = argv(&["--raw", "--help"]);
        let mut parser = ArgvParser::new(&RAW_COMMAND, &values);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &RAW, value: Some(b"--help") }))
        );
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn version_actions_are_scope_local_and_select_short_or_long_text() {
        static VERSION: Action<'static> = Action {
            name: "version",
            diagnostic: "--version",
            help: "Print version",
            longs: &["version"],
            shorts: b"V",
            kind: ActionKind::Version { short: "1.2.3", long: "1.2.3 (build abc)" },
        };
        static VERSIONED: Command<'static> =
            Command { name: "versioned", actions: &[&HELP_ACTION, &VERSION], ..Command::EMPTY };

        let args = argv(&["-V"]);
        let mut parser = ArgvParser::new(&VERSIONED, &args);
        assert_eq!(parser.next_event(), Some(Ok(Event::Action { action: &VERSION, long: false })));
        assert_eq!(parser.next_event(), None);

        let args = argv(&["--version"]);
        let mut parser = ArgvParser::new(&VERSIONED, &args);
        assert_eq!(parser.next_event(), Some(Ok(Event::Action { action: &VERSION, long: true })));
        assert_eq!(parser.next_event(), None);

        let args = argv(&["--version=value"]);
        let mut parser = ArgvParser::new(&VERSIONED, &args);
        assert_eq!(
            parser.next_event(),
            Some(Err(Error::UnexpectedActionValue { action: &VERSION }))
        );

        let args = argv(&["--version"]);
        let mut parser = ArgvParser::new(&COMMAND, &args);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"--version" })));
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

    #[test]
    fn subcommands_switch_parser_scope_and_emit_selection_events() {
        static CHILD_VALUE: Arg<'static> = Arg { key: 31, name: "value", ..Arg::REQUIRED };
        static NESTED: Command<'static> = Command { name: "nested", key: 33, ..Command::EMPTY };
        static CHILD: Command<'static> = Command {
            name: "child",
            args: &[&CHILD_VALUE],
            subcommands: &[&NESTED],
            key: 32,
            ..Command::EMPTY
        };
        static ROOT_VALUE: Arg<'static> = Arg { key: 30, name: "root", ..Arg::REQUIRED };
        static ROOT: Command<'static> = Command {
            name: "root",
            about: None,
            flags: &[&VERBOSE],
            args: &[&ROOT_VALUE],
            subcommands: &[&CHILD],
            key: 29,
            ..Command::EMPTY
        };

        let argv = argv(&["--verbose", "root-value", "child", "child-value", "nested"]);
        let mut parser = ArgvParser::new(&ROOT, &argv);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &VERBOSE, value: None })));
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Arg { arg: &ROOT_VALUE, value: b"root-value" }))
        );
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &CHILD })));
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Arg { arg: &CHILD_VALUE, value: b"child-value" }))
        );
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &NESTED })));
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn globals_are_inherited_downward_and_nearer_spellings_shadow_them() {
        static ROOT_GLOBAL: Flag<'static> = Flag {
            key: 61,
            name: "jobs",
            longs: &["jobs", "workers"],
            shorts: b"j",
            global: true,
            ..Flag::VALUE
        };
        static MID_GLOBAL: Flag<'static> =
            Flag { key: 62, name: "region", longs: &["region"], global: true, ..Flag::VALUE };
        static LEAF_JOBS: Flag<'static> =
            Flag { key: 63, name: "jobs", longs: &["jobs"], ..Flag::VALUE };
        static LEAF: Command<'static> =
            Command { name: "leaf", flags: &[&LEAF_JOBS], key: 66, ..Command::EMPTY };
        static MID: Command<'static> = Command {
            name: "mid",
            flags: &[&MID_GLOBAL],
            subcommands: &[&LEAF],
            key: 65,
            ..Command::EMPTY
        };
        static ROOT: Command<'static> = Command {
            name: "root",
            flags: &[&ROOT_GLOBAL],
            subcommands: &[&MID],
            key: 64,
            ..Command::EMPTY
        };

        let args = argv(&[
            "--jobs=before",
            "mid",
            "--region=between",
            "leaf",
            "--jobs=local",
            "--workers=inherited",
            "--region=after",
        ]);
        let mut parser = ArgvParser::new(&ROOT, &args);

        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &ROOT_GLOBAL, value: Some(b"before") }))
        );
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &MID })));
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &MID_GLOBAL, value: Some(b"between") }))
        );
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &LEAF })));
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &LEAF_JOBS, value: Some(b"local") }))
        );
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &ROOT_GLOBAL, value: Some(b"inherited") }))
        );
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &MID_GLOBAL, value: Some(b"after") }))
        );
        assert_eq!(parser.next_event(), None);

        let args = argv(&["mid", "leaf", "-j", "short"]);
        let mut parser = ArgvParser::new(&ROOT, &args);
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &MID })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &LEAF })));
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &ROOT_GLOBAL, value: Some(b"short") }))
        );
    }

    #[test]
    fn long_aliases_follow_global_scope_and_shadowing_rules() {
        static ROOT_GLOBAL: Flag<'static> = Flag {
            key: 70,
            name: "profile",
            longs: &["profile"],
            aliases: &["context"],
            global: true,
            ..Flag::VALUE
        };
        static CHILD_LOCAL: Flag<'static> =
            Flag { key: 71, name: "context", longs: &["context"], ..Flag::BOOL };
        static CHILD: Command<'static> = Command {
            name: "child",
            aliases: &["c"],
            flags: &[&CHILD_LOCAL],
            key: 72,
            ..Command::EMPTY
        };
        static ROOT: Command<'static> = Command {
            name: "root",
            flags: &[&ROOT_GLOBAL],
            subcommands: &[&CHILD],
            key: 73,
            ..Command::EMPTY
        };

        let args = argv(&["--context", "root", "c", "--context"]);
        let mut parser = ArgvParser::new(&ROOT, &args);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &ROOT_GLOBAL, value: Some(b"root") })),
        );
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &CHILD })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &CHILD_LOCAL, value: None })),);
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn descendant_globals_are_not_visible_before_their_declaring_command() {
        static CHILD_GLOBAL: Flag<'static> = Flag {
            key: 71,
            name: "child-global",
            longs: &["child-global"],
            global: true,
            ..Flag::BOOL
        };
        static CHILD: Command<'static> =
            Command { name: "child", flags: &[&CHILD_GLOBAL], key: 73, ..Command::EMPTY };
        static ROOT: Command<'static> =
            Command { name: "root", subcommands: &[&CHILD], key: 72, ..Command::EMPTY };

        let args = argv(&["--child-global", "child"]);
        let mut parser = ArgvParser::new(&ROOT, &args);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"--child-global" })));

        let args = argv(&["child", "--child-global"]);
        let mut parser = ArgvParser::new(&ROOT, &args);
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &CHILD })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &CHILD_GLOBAL, value: None })));
    }

    #[test]
    fn nearest_inherited_global_wins_when_ancestors_reuse_a_spelling() {
        static ROOT_SCOPE: Flag<'static> =
            Flag { key: 81, name: "root-scope", longs: &["scope"], global: true, ..Flag::BOOL };
        static MID_SCOPE: Flag<'static> =
            Flag { key: 82, name: "mid-scope", longs: &["scope"], global: true, ..Flag::BOOL };
        static LEAF: Command<'static> = Command { name: "leaf", key: 85, ..Command::EMPTY };
        static MID: Command<'static> = Command {
            name: "mid",
            flags: &[&MID_SCOPE],
            subcommands: &[&LEAF],
            key: 84,
            ..Command::EMPTY
        };
        static ROOT: Command<'static> = Command {
            name: "root",
            flags: &[&ROOT_SCOPE],
            subcommands: &[&MID],
            key: 83,
            ..Command::EMPTY
        };

        let args = argv(&["--scope", "mid", "leaf", "--scope"]);
        let mut parser = ArgvParser::new(&ROOT, &args);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &ROOT_SCOPE, value: None })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &MID })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &LEAF })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &MID_SCOPE, value: None })));
    }

    #[test]
    fn subcommand_matching_is_exact_and_separator_disables_selection() {
        static CHILD: Command<'static> = Command { name: "child", key: 41, ..Command::EMPTY };
        static ROOT: Command<'static> =
            Command { name: "root", subcommands: &[&CHILD], key: 40, ..Command::EMPTY };

        let args = argv(&["chi"]);
        let mut parser = ArgvParser::new(&ROOT, &args);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownCommand { token: b"chi" })));
        assert_eq!(parser.next_event(), None);

        let args = argv(&["--", "child"]);
        let mut parser = ArgvParser::new(&ROOT, &args);
        assert_eq!(parser.next_event(), Some(Err(Error::UnexpectedArg { token: b"child" })));
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn detached_flag_values_are_consumed_before_subcommand_matching() {
        static VALUE: Flag<'static> =
            Flag { key: 45, name: "value", longs: &["value"], ..Flag::VALUE };
        static CHILD: Command<'static> = Command { name: "child", key: 47, ..Command::EMPTY };
        static ROOT: Command<'static> = Command {
            name: "root",
            flags: &[&VALUE],
            subcommands: &[&CHILD],
            key: 46,
            ..Command::EMPTY
        };

        let argv = argv(&["--value", "child", "child"]);
        let mut parser = ArgvParser::new(&ROOT, &argv);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &VALUE, value: Some(b"child") })),
        );
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &CHILD })));
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn exact_subcommand_names_take_precedence_over_positionals() {
        static CHILD: Command<'static> = Command { name: "child", key: 51, ..Command::EMPTY };
        static VALUE: Arg<'static> =
            Arg { key: 52, name: "value", required: false, variadic: true, ..Arg::REQUIRED };
        static ROOT: Command<'static> = Command {
            name: "root",
            args: &[&VALUE],
            subcommands: &[&CHILD],
            key: 50,
            ..Command::EMPTY
        };

        let argv = argv(&["word", "child"]);
        let mut parser = ArgvParser::new(&ROOT, &argv);
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &VALUE, value: b"word" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Command { command: &CHILD })));
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn action_bundles_are_fully_preflighted_before_becoming_terminal() {
        let values = argv(&["-hx"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-hx" })));
        assert_eq!(parser.next_event(), None);

        let values = argv(&["-vhx"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-vhx" })));
        assert_eq!(parser.next_event(), None);

        let values = argv(&["-hv"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Action { action: &HELP_ACTION, long: false })),
        );
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn value_taking_short_flags_absorb_every_remaining_bundle_byte() {
        let values = argv(&["-ovf"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"vf") })),
        );
        assert_eq!(parser.next_event(), None);

        let values = argv(&["-ox"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"x") })),);
        assert_eq!(parser.next_event(), None);

        let values = argv(&["-vo="]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &VERBOSE, value: None })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"") })),);
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn pathological_dash_and_equals_tokens_are_not_normalized() {
        let values = argv(&["---"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"---" })));
        assert_eq!(parser.next_event(), None);

        let values = argv(&["--=value"]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"--=value" })));
        assert_eq!(parser.next_event(), None);

        let values = argv(&["-="]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-=" })));
        assert_eq!(parser.next_event(), None);

        let values = argv(&["", "--path", ""]);
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &INPUT, value: b"" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Flag { flag: &OUTPUT, value: Some(b"") })),);
        assert_eq!(parser.next_event(), None);
    }

    #[test]
    fn negative_number_policy_accepts_only_complete_decimal_spellings() {
        static VALUES: Arg<'static> = Arg {
            key: 90,
            name: "values",
            required: false,
            variadic: true,
            allow_negative_numbers: true,
            ..Arg::REQUIRED
        };
        static NUMBERS: Command<'static> =
            Command { name: "numbers", args: &[&VALUES], ..Command::EMPTY };

        let values = argv(&["-.5", "-1.", "-1e+2", "-0"]);
        let mut parser = ArgvParser::new(&NUMBERS, &values);
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &VALUES, value: b"-.5" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &VALUES, value: b"-1." })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &VALUES, value: b"-1e+2" })));
        assert_eq!(parser.next_event(), Some(Ok(Event::Arg { arg: &VALUES, value: b"-0" })));
        assert_eq!(parser.next_event(), None);

        let values = argv(&["-1e+"]);
        let mut parser = ArgvParser::new(&NUMBERS, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-1e+" })));

        let values = argv(&["-+1"]);
        let mut parser = ArgvParser::new(&NUMBERS, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-+1" })));

        let values = argv(&["--1"]);
        let mut parser = ArgvParser::new(&NUMBERS, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"--1" })));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_and_nul_tokens_are_matched_and_reported_byte_exactly() {
        use std::os::unix::ffi::OsStrExt as _;

        let invalid_long = OsStr::from_bytes(b"--verbose-\xff");
        let values = [invalid_long];
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"--verbose-\xff" })),);
        assert_eq!(parser.next_event(), None);

        let invalid_short = OsStr::from_bytes(b"-\xff");
        let values = [invalid_short];
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(parser.next_event(), Some(Err(Error::UnknownFlag { token: b"-\xff" })));
        assert_eq!(parser.next_event(), None);

        let nul_word = OsStr::from_bytes(b"word\0suffix");
        let values = [nul_word];
        let mut parser = ArgvParser::new(&COMMAND, &values);
        assert_eq!(
            parser.next_event(),
            Some(Ok(Event::Arg { arg: &INPUT, value: b"word\0suffix" })),
        );
        assert_eq!(parser.next_event(), None);
    }
}
