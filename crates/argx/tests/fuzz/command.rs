//! Property tests for nested raw subcommand traversal.

use std::{cell::RefCell, ffi::OsString};

use argx::__private::{Arg, ArgvParser, Command, Error, Event, Flag};
use proptest::{collection, prelude::*, test_runner::TestRunner};

use super::{os_string, proptest_config};
use crate::tests::{
    ArgvDisplay, Coverage, Trace, TraceDisplay, env_flag, passthrough_parse, production_parse,
    reference_parse, render_argv, scenario_strategy,
};

/// Token classes used by the nested-command raw parser property.
#[derive(Debug, Clone, Copy)]
enum TreeToken {
    /// Root-only switch spelling.
    RootVerbose,
    /// Root `add` command spelling.
    Add,
    /// Root `config` command spelling.
    Config,
    /// `status` spelling shared by root and config scopes.
    Status,
    /// Add-only switch spelling.
    Force,
    /// Config-only switch spelling.
    Local,
    /// Nested config command spelling.
    Get,
    /// Ordinary positional word.
    Word,
    /// End-of-flags separator.
    Separator,
    /// Flag spelling declared in no scope.
    UnknownFlag,
    /// Word matching no declared child command.
    UnknownWord,
}

/// Borrow-free trace for the fixed nested command-tree reference grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TreeTrace {
    /// Matched flag key.
    Flag(u64),
    /// Matched positional key and value bytes.
    Arg(u64, Vec<u8>),
    /// Selected child-command key.
    Command(u64),
    /// Terminal reference or production failure.
    Error(TreeError),
}

/// Terminal errors represented by the nested command-tree property.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TreeError {
    /// Unknown flag-like token.
    UnknownFlag(Vec<u8>),
    /// Unknown word when child selection is required.
    UnknownCommand(Vec<u8>),
    /// Word accepted by neither positional nor child-command tables.
    UnexpectedArg(Vec<u8>),
}

/// Aggregate measurements for nested raw command traversal.
#[derive(Debug, Default)]
struct TreeCoverage {
    /// Total generated argv tokens.
    tokens: usize,
    /// Generated argv tokens grouped by semantic class.
    token_kinds: [usize; TreeToken::COUNT],
    /// Selection counts for each child command in the fixed tree.
    commands: [usize; 5],
    /// Matched flag events.
    flags: usize,
    /// Bound positional events.
    args: usize,
    /// Unknown-flag, unknown-command, and unexpected-argument errors.
    errors: [usize; 3],
}

/// Generates adversarial token sequences over a fixed two-level command tree.
fn tree_tokens_strategy() -> impl Strategy<Value = Vec<TreeToken>> {
    collection::vec(
        prop_oneof![
            3 => Just(TreeToken::RootVerbose),
            4 => Just(TreeToken::Add),
            4 => Just(TreeToken::Config),
            4 => Just(TreeToken::Status),
            3 => Just(TreeToken::Force),
            3 => Just(TreeToken::Local),
            4 => Just(TreeToken::Get),
            6 => Just(TreeToken::Word),
            2 => Just(TreeToken::Separator),
            2 => Just(TreeToken::UnknownFlag),
            2 => Just(TreeToken::UnknownWord),
        ],
        0..=32,
    )
}

impl TreeToken {
    /// Number of generated token classes.
    const COUNT: usize = 11;

    /// Stable coverage bucket for this generated token class.
    const fn index(self) -> usize {
        match self {
            Self::RootVerbose => 0,
            Self::Add => 1,
            Self::Config => 2,
            Self::Status => 3,
            Self::Force => 4,
            Self::Local => 5,
            Self::Get => 6,
            Self::Word => 7,
            Self::Separator => 8,
            Self::UnknownFlag => 9,
            Self::UnknownWord => 10,
        }
    }
}

/// Renders one command-tree token class into argv text.
const fn tree_token_text(token: TreeToken) -> &'static str {
    match token {
        TreeToken::RootVerbose => "--verbose",
        TreeToken::Add => "add",
        TreeToken::Config => "config",
        TreeToken::Status => "status",
        TreeToken::Force => "--force",
        TreeToken::Local => "--local",
        TreeToken::Get => "get",
        TreeToken::Word => "word",
        TreeToken::Separator => "--",
        TreeToken::UnknownFlag => "--unknown",
        TreeToken::UnknownWord => "unknown-command",
    }
}

/// Renders one command-tree token class into argv bytes for the reference grammar.
fn tree_token_bytes(token: TreeToken) -> &'static [u8] {
    tree_token_text(token).as_bytes()
}

/// Runs the production parser over the fixed nested command tree.
fn production_tree_parse(tokens: &[TreeToken]) -> Vec<TreeTrace> {
    static ROOT_VERBOSE: Flag<'static> =
        Flag { key: 0x4101, name: "verbose", longs: &["verbose"], ..Flag::BOOL };
    static ADD_FORCE: Flag<'static> =
        Flag { key: 0x4201, name: "force", longs: &["force"], ..Flag::BOOL };
    static CONFIG_LOCAL: Flag<'static> =
        Flag { key: 0x4301, name: "local", longs: &["local"], ..Flag::BOOL };
    static ROOT_VALUE: Arg<'static> = Arg { key: 0x4102, name: "workspace", ..Arg::REQUIRED };
    static ADD_VALUE: Arg<'static> = Arg { key: 0x4202, name: "value", ..Arg::REQUIRED };
    static GET_VALUE: Arg<'static> = Arg { key: 0x4402, name: "key", ..Arg::REQUIRED };
    static GET: Command<'static> =
        Command { name: "get", args: &[&GET_VALUE], key: 0x4400, ..Command::EMPTY };
    static CONFIG_STATUS: Command<'static> =
        Command { name: "status", key: 0x4500, ..Command::EMPTY };
    static ADD: Command<'static> = Command {
        name: "add",
        flags: &[&ADD_FORCE],
        args: &[&ADD_VALUE],
        key: 0x4200,
        ..Command::EMPTY
    };
    static CONFIG: Command<'static> = Command {
        name: "config",
        flags: &[&CONFIG_LOCAL],
        subcommands: &[&GET, &CONFIG_STATUS],
        key: 0x4300,
        ..Command::EMPTY
    };
    static ROOT_STATUS: Command<'static> =
        Command { name: "status", key: 0x4600, ..Command::EMPTY };
    static ROOT: Command<'static> = Command {
        name: "root",
        about: None,
        flags: &[&ROOT_VERBOSE],
        args: &[&ROOT_VALUE],
        subcommands: &[&ADD, &CONFIG, &ROOT_STATUS],
        key: 0x4100,
        ..Command::EMPTY
    };

    let owned =
        tokens.iter().map(|token| OsString::from(tree_token_text(*token))).collect::<Vec<_>>();
    let refs = owned.iter().map(OsString::as_os_str).collect::<Vec<_>>();
    let mut parser = ArgvParser::new(&ROOT, &refs);
    let mut trace = Vec::new();
    while let Some(item) = parser.next_event() {
        match item {
            Ok(Event::Action { action, .. }) => {
                panic!("fixed command-tree grammar selected unexpected action `{}`", action.name)
            }
            Ok(Event::Flag { flag, .. }) => trace.push(TreeTrace::Flag(flag.key)),
            Ok(Event::Arg { arg, value }) => trace.push(TreeTrace::Arg(arg.key, value.to_vec())),
            Ok(Event::Command { command }) => trace.push(TreeTrace::Command(command.key)),
            Err(Error::UnknownFlag { token }) => {
                trace.push(TreeTrace::Error(TreeError::UnknownFlag(token.to_vec())));
                break;
            }
            Err(Error::UnknownCommand { token }) => {
                trace.push(TreeTrace::Error(TreeError::UnknownCommand(token.to_vec())));
                break;
            }
            Err(Error::UnexpectedArg { token }) => {
                trace.push(TreeTrace::Error(TreeError::UnexpectedArg(token.to_vec())));
                break;
            }
            Err(error) => {
                panic!("fixed command-tree grammar produced unexpected error: {error:?}")
            }
        }
    }
    assert!(parser.next_event().is_none(), "command-tree parser must remain exhausted");
    assert!(parser.next_event().is_none(), "command-tree parser exhaustion must be stable");
    trace
}

/// Runs the deliberately separate fixed command-tree reference grammar.
fn reference_tree_parse(tokens: &[TreeToken]) -> Vec<TreeTrace> {
    #[derive(Clone, Copy)]
    enum Scope {
        /// Root command scope.
        Root,
        /// Root `add` payload scope.
        Add,
        /// Root `config` payload scope.
        Config,
        /// Root unit `status` scope.
        RootStatus,
        /// Nested `config get` payload scope.
        Get,
        /// Nested unit `config status` scope.
        ConfigStatus,
    }

    let mut scope = Scope::Root;
    let mut positional = 0_usize;
    let mut flags_stopped = false;
    let mut trace = Vec::new();

    for generated in tokens {
        let token = tree_token_bytes(*generated);
        if !flags_stopped && token == b"--" {
            flags_stopped = true;
            continue;
        }

        if !flags_stopped && token.starts_with(b"-") && token != b"-" {
            let key = match (scope, token) {
                (Scope::Root, b"--verbose") => Some(0x4101),
                (Scope::Add, b"--force") => Some(0x4201),
                (Scope::Config, b"--local") => Some(0x4301),
                _ => None,
            };
            if let Some(key) = key {
                trace.push(TreeTrace::Flag(key));
                continue;
            }
            trace.push(TreeTrace::Error(TreeError::UnknownFlag(token.to_vec())));
            break;
        }

        if !flags_stopped {
            let selected = match (scope, token) {
                (Scope::Root, b"add") => Some((Scope::Add, 0x4200)),
                (Scope::Root, b"config") => Some((Scope::Config, 0x4300)),
                (Scope::Root, b"status") => Some((Scope::RootStatus, 0x4600)),
                (Scope::Config, b"get") => Some((Scope::Get, 0x4400)),
                (Scope::Config, b"status") => Some((Scope::ConfigStatus, 0x4500)),
                _ => None,
            };
            if let Some((next, key)) = selected {
                scope = next;
                positional = 0;
                trace.push(TreeTrace::Command(key));
                continue;
            }
        }

        let arg = match scope {
            Scope::Root if positional == 0 => Some(0x4102),
            Scope::Add if positional == 0 => Some(0x4202),
            Scope::Get if positional == 0 => Some(0x4402),
            _ => None,
        };
        if let Some(key) = arg {
            positional += 1;
            trace.push(TreeTrace::Arg(key, token.to_vec()));
            continue;
        }

        let has_subcommands = matches!(scope, Scope::Root | Scope::Config);
        if !flags_stopped && has_subcommands {
            trace.push(TreeTrace::Error(TreeError::UnknownCommand(token.to_vec())));
        } else {
            trace.push(TreeTrace::Error(TreeError::UnexpectedArg(token.to_vec())));
        }
        break;
    }
    trace
}

/// Fuzzes generated valid command schemas and argv against the reference grammar.
#[test]
fn generated_commands_and_argv_match_reference_grammar() {
    let strategy = scenario_strategy();
    let config = proptest_config("generated_commands_and_argv_match_reference_grammar");
    let cases = config.cases;
    let trace_cases = env_flag("ARGX_FUZZ_TRACE");
    let coverage = RefCell::new(Coverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |scenario| {
        let raw_argv = render_argv(&scenario);
        let os_argv = raw_argv.iter().map(|token| os_string(token)).collect::<Vec<_>>();
        let encoded_argv = os_argv
            .iter()
            .map(|token| token.as_encoded_bytes().to_vec())
            .collect::<Vec<_>>();
        let expected = reference_parse(&scenario.command, &encoded_argv);
        let actual = production_parse(&scenario.command, &os_argv);
        let repeated = production_parse(&scenario.command, &os_argv);

        prop_assert!(actual.exhausted_once, "parser emitted an item after completion or error");
        prop_assert!(actual.exhausted_twice, "parser exhaustion was not stable");
        prop_assert!(
            actual == repeated,
            "repeated parsing was not deterministic\nfirst: {actual}\nrepeated: {repeated}"
        );
        prop_assert!(
            actual.trace == expected,
            "generated command and argv diverged from the reference grammar\n{}\nargv: {}\nactual: {}\nexpected: {}",
            scenario.command,
            ArgvDisplay(&encoded_argv),
            TraceDisplay(&actual.trace),
            TraceDisplay(&expected),
         );

        let passthrough = passthrough_parse(&os_argv);
        let passthrough_expected = encoded_argv
            .iter()
            .map(|value| Trace::Arg { key: 0x3000, value: value.clone() })
            .collect::<Vec<_>>();
        prop_assert!(passthrough.exhausted_once && passthrough.exhausted_twice);
        prop_assert!(
            passthrough.trace == passthrough_expected,
            "end-of-flags passthrough did not preserve argv bytes\nargv: {}\nactual: {}\nexpected: {}",
            ArgvDisplay(&encoded_argv),
            TraceDisplay(&passthrough.trace),
            TraceDisplay(&passthrough_expected),
         );

        coverage.borrow_mut().record(&scenario, &encoded_argv, &actual.trace);
        if trace_cases {
            eprintln!(
                "[parser fuzz] {} argv={} outcome={}",
                scenario.command,
                ArgvDisplay(&encoded_argv),
                TraceDisplay(&actual.trace)
            );
        }
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx parser property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!(
        "[parser fuzz] PASS: {cases} cases, {} generated tokens; production parsing matched the reference grammar",
        coverage.tokens,
    );
    eprintln!(
        "[parser fuzz] events: flags={} | positionals={} | non_utf8_tokens={}",
        coverage.flags, coverage.args, coverage.non_utf8_tokens,
    );
    eprintln!(
        "[parser fuzz] terminal outcomes: unknown_flag={} | missing_value={} | unexpected_value={} | unexpected_arg={} | display_help={}",
        coverage.errors[0],
        coverage.errors[1],
        coverage.errors[2],
        coverage.errors[3],
        coverage.errors[4],
    );
    eprintln!(
        "[parser fuzz] generated token classes: word={} | known_long={} | known_long_attached={} | known_short={} | known_short_attached={} | short_bundle={} | unknown_long={} | unknown_short={} | separator={} | negative={} | lone_dash={} | empty={} | raw_flag_like={} | help_long={} | help_short={}",
        coverage.token_kinds[0],
        coverage.token_kinds[1],
        coverage.token_kinds[2],
        coverage.token_kinds[3],
        coverage.token_kinds[4],
        coverage.token_kinds[5],
        coverage.token_kinds[6],
        coverage.token_kinds[7],
        coverage.token_kinds[8],
        coverage.token_kinds[9],
        coverage.token_kinds[10],
        coverage.token_kinds[11],
        coverage.token_kinds[12],
        coverage.token_kinds[13],
        coverage.token_kinds[14],
    );
}

/// Fuzzes command selection and scope changes against a separate nested-tree model.
#[test]
fn nested_command_traversal_matches_reference_grammar() {
    let strategy = tree_tokens_strategy();
    let config = proptest_config("nested_command_traversal_matches_reference_grammar");
    let cases = config.cases;
    let coverage = RefCell::new(TreeCoverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |tokens| {
        let expected = reference_tree_parse(&tokens);
        let actual = production_tree_parse(&tokens);
        let repeated = production_tree_parse(&tokens);
        prop_assert_eq!(actual.as_slice(), expected.as_slice());
        prop_assert_eq!(actual.as_slice(), repeated.as_slice());

        let mut coverage = coverage.borrow_mut();
        coverage.tokens += tokens.len();
        for token in &tokens {
            coverage.token_kinds[token.index()] += 1;
        }
        for item in &actual {
            match item {
                TreeTrace::Flag(_) => coverage.flags += 1,
                TreeTrace::Arg(_, _) => coverage.args += 1,
                TreeTrace::Command(key) => {
                    let index = match *key {
                        0x4200 => 0,
                        0x4300 => 1,
                        0x4600 => 2,
                        0x4400 => 3,
                        0x4500 => 4,
                        other => panic!("unexpected generated command key: {other}"),
                    };
                    coverage.commands[index] += 1;
                }
                TreeTrace::Error(TreeError::UnknownFlag(_)) => coverage.errors[0] += 1,
                TreeTrace::Error(TreeError::UnknownCommand(_)) => coverage.errors[1] += 1,
                TreeTrace::Error(TreeError::UnexpectedArg(_)) => coverage.errors[2] += 1,
            }
        }
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx nested command traversal property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!(
        "[command fuzz] PASS: {cases} cases, {} generated tokens; nested traversal matched the reference grammar",
        coverage.tokens,
    );
    eprintln!(
        "[command fuzz] selections: add={} | config={} | root_status={} | get={} | config_status={}",
        coverage.commands[0],
        coverage.commands[1],
        coverage.commands[2],
        coverage.commands[3],
        coverage.commands[4],
    );
    eprintln!("[command fuzz] events: flags={} | positionals={}", coverage.flags, coverage.args,);
    eprintln!(
        "[command fuzz] terminal errors: unknown_flag={} | unknown_command={} | unexpected_arg={}",
        coverage.errors[0], coverage.errors[1], coverage.errors[2],
    );
    eprintln!(
        "[command fuzz] generated token classes: root_flag={} | add={} | config={} | status={} | child_flag={} | config_flag={} | get={} | word={} | separator={} | unknown_flag={} | unknown_word={}",
        coverage.token_kinds[0],
        coverage.token_kinds[1],
        coverage.token_kinds[2],
        coverage.token_kinds[3],
        coverage.token_kinds[4],
        coverage.token_kinds[5],
        coverage.token_kinds[6],
        coverage.token_kinds[7],
        coverage.token_kinds[8],
        coverage.token_kinds[9],
        coverage.token_kinds[10],
    );
}
