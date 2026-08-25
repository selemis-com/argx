//! Shared raw-parser fuzz model and campaign.

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt as _;
use std::{
    env,
    ffi::{OsStr, OsString},
    fmt::{self, Display, Formatter, Write as _},
};

use argx::__private::{ActionKind, Arg, ArgvParser, Command, Error, Event, Flag};
use proptest::{
    collection,
    prelude::*,
    sample,
    test_runner::{Config, FileFailurePersistence},
};

/// Visible short spellings used to build unique generated command tables.
const SHORT_ALPHABET: &[u8] =
    b"abcdefg`ijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+[]{};,.?/|~";

/// Derive-independent shape controls for one generated flag.
#[derive(Debug, Clone, Copy)]
struct FlagPolicy {
    /// Whether the flag consumes one value.
    takes_value: bool,
    /// Whether a detached flag-like value may be consumed.
    allow_hyphen_values: bool,
    /// Whether a detached negative number may be consumed.
    allow_negative_numbers: bool,
    /// Which combination of long names, short names, and aliases is generated.
    spelling_mode: u8,
}

/// Owned representation of one valid parser command table.
#[derive(Debug, Clone)]
struct CommandSpec {
    /// Generated named flags in declaration order.
    flags: Vec<FlagSpec>,
    /// Generated positional arguments in binding order.
    args: Vec<ArgSpec>,
}

/// Owned representation of one generated flag table entry.
#[derive(Debug, Clone)]
struct FlagSpec {
    /// Stable identity used to normalize parser events.
    key: u64,
    /// Diagnostic field name.
    name: String,
    /// Accepted long spellings without `--`.
    longs: Vec<String>,
    /// Accepted short spellings without `-`.
    shorts: Vec<u8>,
    /// Whether the flag consumes one value.
    takes_value: bool,
    /// Whether a detached flag-like value may be consumed.
    allow_hyphen_values: bool,
    /// Whether a detached negative number may be consumed.
    allow_negative_numbers: bool,
}

/// Owned representation of one generated positional table entry.
#[derive(Debug, Clone)]
struct ArgSpec {
    /// Stable identity used to normalize parser events.
    key: u64,
    /// Diagnostic argument name.
    name: String,
    /// Whether the positional is required by the complete command contract.
    required: bool,
    /// Whether this final positional accepts repeated values.
    variadic: bool,
    /// Whether a negative number may bind while flag parsing is enabled.
    allow_negative_numbers: bool,
}

/// Semantic class of one generated argv token.
#[derive(Debug, Clone, Copy)]
enum TokenKind {
    /// Non-flag positional word.
    Word,
    /// Declared long spelling without an attached value.
    KnownLong,
    /// Declared long spelling with an attached value.
    KnownLongAttached,
    /// Declared short spelling without an attached value.
    KnownShort,
    /// Declared short spelling with bytes following it in the same token.
    KnownShortAttached,
    /// Bundle of declared short spellings.
    KnownShortBundle,
    /// Long spelling guaranteed not to be declared.
    UnknownLong,
    /// Short bundle containing an unknown spelling.
    UnknownShortBundle,
    /// End-of-flags separator.
    Separator,
    /// Supported negative-number spelling.
    NegativeNumber,
    /// Conventional lone-dash positional value.
    LoneDash,
    /// Empty positional value.
    Empty,
    /// Unusual but syntactically flag-like raw token.
    RawFlagLike,
    /// Built-in long help switch.
    HelpLong,
    /// Built-in short help switch.
    HelpShort,
}

impl TokenKind {
    /// Number of variants represented in coverage counters.
    const COUNT: usize = 15;

    /// Stable coverage-counter index for this token class.
    const fn index(self) -> usize {
        match self {
            Self::Word => 0,
            Self::KnownLong => 1,
            Self::KnownLongAttached => 2,
            Self::KnownShort => 3,
            Self::KnownShortAttached => 4,
            Self::KnownShortBundle => 5,
            Self::UnknownLong => 6,
            Self::UnknownShortBundle => 7,
            Self::Separator => 8,
            Self::NegativeNumber => 9,
            Self::LoneDash => 10,
            Self::Empty => 11,
            Self::RawFlagLike => 12,
            Self::HelpLong => 13,
            Self::HelpShort => 14,
        }
    }
}

/// Shrinkable instructions for rendering one generated token against a command.
#[derive(Debug, Clone)]
struct TokenSpec {
    /// Semantic token class.
    kind: TokenKind,
    /// Selectors resolved against the generated command table.
    selectors: [u8; 4],
    /// Arbitrary encoded payload used by words and attached values.
    payload: Vec<u8>,
    /// Whether a short attached value starts with an optional `=` delimiter.
    equals: bool,
}

/// One complete generated parser case.
#[derive(Debug, Clone)]
struct Scenario {
    /// Valid command metadata supplied to both parsers.
    command: CommandSpec,
    /// Shrinkable argv token instructions.
    tokens: Vec<TokenSpec>,
}

/// Parser output normalized away from borrowed table and argv storage.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Trace {
    /// One matched named flag.
    Flag {
        /// Generated flag identity.
        key: u64,
        /// Optional encoded value consumed by the flag.
        value: Option<Vec<u8>>,
    },
    /// One bound positional value.
    Arg {
        /// Generated positional identity.
        key: u64,
        /// Encoded positional value.
        value: Vec<u8>,
    },
    /// Terminal parser control-flow or failure result.
    Error(ErrorTrace),
}

impl Trace {
    /// Reports whether this item terminates parsing.
    const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Borrow-free representation of the public parser error contract.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ErrorTrace {
    /// Whole unknown flag-like token.
    UnknownFlag(Vec<u8>),
    /// Identity of a flag missing its value.
    MissingFlagValue(u64),
    /// Identity of a switch receiving an attached value.
    UnexpectedFlagValue(u64),
    /// Whole positional token that could not be bound.
    UnexpectedArg(Vec<u8>),
    /// Built-in help terminated parsing successfully.
    DisplayHelp,
}

impl ErrorTrace {
    /// Stable coverage-counter index for this error class.
    const fn index(&self) -> usize {
        match self {
            Self::UnknownFlag(_) => 0,
            Self::MissingFlagValue(_) => 1,
            Self::UnexpectedFlagValue(_) => 2,
            Self::UnexpectedArg(_) => 3,
            Self::DisplayHelp => 4,
        }
    }
}

/// One completed production parse plus exhaustion-state observations.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParseRun {
    /// Normalized events and optional terminal error.
    trace: Vec<Trace>,
    /// Whether the first call after completion or failure returned `None`.
    exhausted_once: bool,
    /// Whether exhaustion remained stable for a second call.
    exhausted_twice: bool,
}

/// Display adapter for one encoded operating-system argument.
struct Encoded<'a>(&'a [u8]);

impl Display for Encoded<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_char('"')?;
        for &byte in self.0 {
            match byte {
                b'\\' => formatter.write_str("\\\\")?,
                b'"' => formatter.write_str("\\\"")?,
                b'\n' => formatter.write_str("\\n")?,
                b'\r' => formatter.write_str("\\r")?,
                b'\t' => formatter.write_str("\\t")?,
                b' '..=b'~' => formatter.write_char(char::from(byte))?,
                _ => write!(formatter, "\\x{byte:02x}")?,
            }
        }
        formatter.write_char('"')
    }
}

/// Display adapter for an argv vector.
struct ArgvDisplay<'a>(&'a [Vec<u8>]);

impl Display for ArgvDisplay<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_char('[')?;
        for (index, token) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{}", Encoded(token))?;
        }
        formatter.write_char(']')
    }
}

impl Display for FlagSpec {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "flag(key={}, name={}, longs=[",
            self.key,
            Encoded(self.name.as_bytes())
        )?;
        for (index, long) in self.longs.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{}", Encoded(long.as_bytes()))?;
        }
        write!(
            formatter,
            "], shorts={}, takes_value={}, allow_hyphen_values={}, allow_negative_numbers={})",
            Encoded(&self.shorts),
            self.takes_value,
            self.allow_hyphen_values,
            self.allow_negative_numbers
        )
    }
}

impl Display for ArgSpec {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "arg(key={}, name={}, required={}, variadic={}, allow_negative_numbers={})",
            self.key,
            Encoded(self.name.as_bytes()),
            self.required,
            self.variadic,
            self.allow_negative_numbers
        )
    }
}

impl Display for CommandSpec {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("command(flags=[")?;
        for (index, flag) in self.flags.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{flag}")?;
        }
        formatter.write_str("], args=[")?;
        for (index, arg) in self.args.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{arg}")?;
        }
        formatter.write_str("])")
    }
}

impl Display for ErrorTrace {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFlag(token) => write!(formatter, "unknown_flag({})", Encoded(token)),
            Self::MissingFlagValue(key) => write!(formatter, "missing_flag_value(key={key})"),
            Self::UnexpectedFlagValue(key) => {
                write!(formatter, "unexpected_flag_value(key={key})")
            }
            Self::UnexpectedArg(token) => {
                write!(formatter, "unexpected_arg({})", Encoded(token))
            }
            Self::DisplayHelp => formatter.write_str("display_help"),
        }
    }
}

impl Display for Trace {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag { key, value } => {
                write!(formatter, "flag(key={key}, value=")?;
                if let Some(value) = value {
                    write!(formatter, "{}", Encoded(value))?;
                } else {
                    formatter.write_str("none")?;
                }
                formatter.write_char(')')
            }
            Self::Arg { key, value } => {
                write!(formatter, "arg(key={key}, value={})", Encoded(value))
            }
            Self::Error(error) => write!(formatter, "error({error})"),
        }
    }
}

/// Display adapter for a normalized parser trace.
struct TraceDisplay<'a>(&'a [Trace]);

impl Display for TraceDisplay<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_char('[')?;
        for (index, event) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{event}")?;
        }
        formatter.write_char(']')
    }
}

impl Display for ParseRun {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parse(trace={}, exhausted_once={}, exhausted_twice={})",
            TraceDisplay(&self.trace),
            self.exhausted_once,
            self.exhausted_twice
        )
    }
}

/// Aggregate campaign measurements printed after successful execution.
#[derive(Debug, Default)]
struct Coverage {
    /// Rendered tokens across successful cases.
    tokens: usize,
    /// Counts for each generated token class.
    token_kinds: [usize; TokenKind::COUNT],
    /// Encoded tokens that are not valid UTF-8.
    non_utf8_tokens: usize,
    /// Matched flag events.
    flags: usize,
    /// Bound positional events.
    args: usize,
    /// Counts for each terminal control-flow or error class.
    errors: [usize; 5],
}

impl Coverage {
    /// Records one successful generated scenario.
    fn record(&mut self, scenario: &Scenario, argv: &[Vec<u8>], trace: &[Trace]) {
        self.tokens += scenario.tokens.len();
        for token in &scenario.tokens {
            self.token_kinds[token.kind.index()] += 1;
        }
        self.non_utf8_tokens +=
            argv.iter().filter(|token| std::str::from_utf8(token).is_err()).count();
        for item in trace {
            match item {
                Trace::Flag { .. } => self.flags += 1,
                Trace::Arg { .. } => self.args += 1,
                Trace::Error(error) => self.errors[error.index()] += 1,
            }
        }
    }
}

/// Generates independently shrinkable flag behavior.
fn flag_policy_strategy() -> impl Strategy<Value = FlagPolicy> {
    (any::<bool>(), any::<bool>(), any::<bool>(), 0_u8..=3).prop_map(
        |(takes_value, allow_hyphen_values, allow_negative_numbers, spelling_mode)| FlagPolicy {
            takes_value,
            allow_hyphen_values: takes_value && allow_hyphen_values,
            allow_negative_numbers: takes_value && allow_negative_numbers,
            spelling_mode,
        },
    )
}

/// Generates valid command-wide flag and positional metadata.
fn command_strategy() -> impl Strategy<Value = CommandSpec> {
    (
        collection::vec(flag_policy_strategy(), 1..=8),
        collection::vec(any::<bool>(), 1..=6),
        any::<u8>(),
        any::<bool>(),
        0_u8..=69,
    )
        .prop_map(
            |(flag_policies, arg_policies, required_selector, variadic, short_offset)| {
                let flags = flag_policies
                    .into_iter()
                    .enumerate()
                    .map(|(index, policy)| flag_spec(index, short_offset, policy))
                    .collect();
                let required_count = usize::from(required_selector) % (arg_policies.len() + 1);
                let last_arg = arg_policies.len() - 1;
                let args = arg_policies
                    .into_iter()
                    .enumerate()
                    .map(|(index, allow_negative_numbers)| ArgSpec {
                        key: 0x2000 + u64::try_from(index).expect("generated index fits in u64"),
                        name: format!("arg{index}"),
                        required: index < required_count && !(variadic && index == last_arg),
                        variadic: variadic && index == last_arg,
                        allow_negative_numbers,
                    })
                    .collect();
                CommandSpec { flags, args }
            },
        )
}

/// Builds unique spellings and policies for one generated flag.
fn flag_spec(index: usize, short_offset: u8, policy: FlagPolicy) -> FlagSpec {
    let spelling_mode = policy.spelling_mode % 4;
    let mut longs = Vec::new();
    let mut shorts = Vec::new();
    if spelling_mode != 1 {
        longs.push(format!("flag{index}"));
    }
    if spelling_mode == 3 {
        longs.push(format!("alias{index}"));
    }
    if spelling_mode != 0 {
        let short_index = usize::from(short_offset) + index * 2;
        shorts.push(SHORT_ALPHABET[short_index]);
        if spelling_mode == 3 {
            shorts.push(SHORT_ALPHABET[short_index + 1]);
        }
    }
    FlagSpec {
        key: 0x1000 + u64::try_from(index).expect("generated index fits in u64"),
        name: format!("field{index}"),
        longs,
        shorts,
        takes_value: policy.takes_value,
        allow_hyphen_values: policy.allow_hyphen_values,
        allow_negative_numbers: policy.allow_negative_numbers,
    }
}

/// Generates a weighted mix of structured and adversarial token classes.
fn token_kind_strategy() -> impl Strategy<Value = TokenKind> {
    prop_oneof![
        8 => Just(TokenKind::Word),
        6 => Just(TokenKind::KnownLong),
        5 => Just(TokenKind::KnownLongAttached),
        6 => Just(TokenKind::KnownShort),
        5 => Just(TokenKind::KnownShortAttached),
        5 => Just(TokenKind::KnownShortBundle),
        2 => Just(TokenKind::UnknownLong),
        2 => Just(TokenKind::UnknownShortBundle),
        3 => Just(TokenKind::Separator),
        4 => Just(TokenKind::NegativeNumber),
        2 => Just(TokenKind::LoneDash),
        1 => Just(TokenKind::Empty),
        2 => Just(TokenKind::RawFlagLike),
        1 => Just(TokenKind::HelpLong),
        1 => Just(TokenKind::HelpShort),
    ]
}

/// Generates arbitrary value bytes with extra weight on parser boundary shapes.
fn payload_strategy() -> impl Strategy<Value = Vec<u8>> {
    let alphabet =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_=+eE".to_vec();
    prop_oneof![
        5 => collection::vec(sample::select(alphabet), 0..=16),
        2 => collection::vec(1_u8..=u8::MAX, 0..=16),
        1 => Just(Vec::new()),
        1 => Just(b"--flag0".to_vec()),
        1 => Just(b"-1.5e-2".to_vec()),
        1 => Just(b"a=b=c".to_vec()),
    ]
}

/// Generates one shrinkable token instruction.
fn token_strategy() -> impl Strategy<Value = TokenSpec> {
    (token_kind_strategy(), any::<[u8; 4]>(), payload_strategy(), any::<bool>()).prop_map(
        |(kind, selectors, payload, equals)| TokenSpec { kind, selectors, payload, equals },
    )
}

/// Generates a complete command plus argv campaign input.
fn scenario_strategy() -> impl Strategy<Value = Scenario> {
    let minimum = env_usize("ARGX_FUZZ_MIN_TOKENS", 0);
    let maximum = env_usize("ARGX_FUZZ_TOKENS", 64);
    assert!(minimum <= maximum, "ARGX_FUZZ_MIN_TOKENS must not exceed ARGX_FUZZ_TOKENS");
    (command_strategy(), collection::vec(token_strategy(), minimum..=maximum))
        .prop_map(|(command, tokens)| Scenario { command, tokens })
}

/// Returns the configured Proptest runner behavior with source-adjacent regression persistence.
fn proptest_config(test_name: &'static str) -> Config {
    Config {
        cases: env_u32("ARGX_FUZZ_CASES", 512),
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource(
            "proptest-regressions",
        ))),
        source_file: Some(file!()),
        test_name: Some(test_name),
        ..Config::default()
    }
}

/// Reads one required-positive integer environment override.
fn env_u32(name: &str, default: u32) -> u32 {
    let value = env::var(name).map_or(default, |value| {
        value.parse().unwrap_or_else(|_| panic!("{name} must be a positive integer"))
    });
    assert!(value > 0, "{name} must be a positive integer");
    value
}

/// Reads one non-negative integer environment override.
fn env_usize(name: &str, default: usize) -> usize {
    env::var(name).map_or(default, |value| {
        value.parse().unwrap_or_else(|_| panic!("{name} must be an integer"))
    })
}

/// Reads a conventional boolean environment flag.
fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| {
        matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
    })
}

/// Renders every generated token against its command table.
fn render_argv(scenario: &Scenario) -> Vec<Vec<u8>> {
    scenario.tokens.iter().map(|token| render_token(&scenario.command, token)).collect()
}

/// Resolves one token instruction to encoded argv bytes.
fn render_token(command: &CommandSpec, token: &TokenSpec) -> Vec<u8> {
    match token.kind {
        TokenKind::Word => {
            let mut value = b"word".to_vec();
            value.extend_from_slice(&token.payload);
            value
        }
        TokenKind::KnownLong => selected_long(command, token.selectors[0], token.selectors[1])
            .map_or_else(
                || unknown_long(token.selectors[0], false, &token.payload),
                |long| prefixed_long(long, None),
            ),
        TokenKind::KnownLongAttached => {
            selected_long(command, token.selectors[0], token.selectors[1]).map_or_else(
                || unknown_long(token.selectors[0], true, &token.payload),
                |long| prefixed_long(long, Some(&token.payload)),
            )
        }
        TokenKind::KnownShort => selected_short(command, token.selectors[0])
            .map_or_else(|| vec![b'-', b':'], |short| vec![b'-', short]),
        TokenKind::KnownShortAttached => {
            let Some(short) = selected_short(command, token.selectors[0]) else {
                return vec![b'-', b':'];
            };
            let mut value = vec![b'-', short];
            if token.equals {
                value.push(b'=');
            }
            value.extend_from_slice(&token.payload);
            value
        }
        TokenKind::KnownShortBundle => {
            let mut value = vec![b'-'];
            let count = 1 + usize::from(token.selectors[3] % 4);
            for selector in &token.selectors[..count] {
                if let Some(short) = selected_short(command, *selector) {
                    value.push(short);
                }
            }
            if value.len() == 1 {
                value.push(b':');
            }
            value
        }
        TokenKind::UnknownLong => unknown_long(token.selectors[0], token.equals, &token.payload),
        TokenKind::UnknownShortBundle => {
            let mut value = vec![b'-'];
            if let Some(short) = selected_switch_short(command, token.selectors[0]) {
                value.push(short);
            }
            value.push(b':');
            value
        }
        TokenKind::Separator => b"--".to_vec(),
        TokenKind::NegativeNumber => negative_number(token.selectors[0]).to_vec(),
        TokenKind::LoneDash => b"-".to_vec(),
        TokenKind::Empty => Vec::new(),
        TokenKind::RawFlagLike => raw_flag_like(token.selectors[0]).to_vec(),
        TokenKind::HelpLong => b"--help".to_vec(),
        TokenKind::HelpShort => b"-h".to_vec(),
    }
}

/// Selects one declared long spelling, including aliases.
fn selected_long(command: &CommandSpec, selector: u8, alias: u8) -> Option<&str> {
    let count = command.flags.iter().filter(|flag| !flag.longs.is_empty()).count();
    let flag = command
        .flags
        .iter()
        .filter(|flag| !flag.longs.is_empty())
        .nth(usize::from(selector) % count.max(1))?;
    flag.longs.get(usize::from(alias) % flag.longs.len()).map(String::as_str)
}

/// Selects one declared short spelling, including aliases.
fn selected_short(command: &CommandSpec, selector: u8) -> Option<u8> {
    let total = command.flags.iter().map(|flag| flag.shorts.len()).sum::<usize>();
    let mut selected = usize::from(selector) % total.max(1);
    for flag in &command.flags {
        if selected < flag.shorts.len() {
            return flag.shorts.get(selected).copied();
        }
        selected = selected.saturating_sub(flag.shorts.len());
    }
    None
}

/// Selects a value-less short so a following unknown short remains syntactic.
fn selected_switch_short(command: &CommandSpec, selector: u8) -> Option<u8> {
    let total = command
        .flags
        .iter()
        .filter(|flag| !flag.takes_value)
        .map(|flag| flag.shorts.len())
        .sum::<usize>();
    let mut selected = usize::from(selector) % total.max(1);
    for flag in command.flags.iter().filter(|flag| !flag.takes_value) {
        if selected < flag.shorts.len() {
            return flag.shorts.get(selected).copied();
        }
        selected = selected.saturating_sub(flag.shorts.len());
    }
    None
}

/// Adds a long prefix and optional attached-value delimiter.
fn prefixed_long(long: &str, attached: Option<&[u8]>) -> Vec<u8> {
    let mut value = b"--".to_vec();
    value.extend_from_slice(long.as_bytes());
    if let Some(attached) = attached {
        value.push(b'=');
        value.extend_from_slice(attached);
    }
    value
}

/// Builds a long spelling outside the generated command namespace.
fn unknown_long(selector: u8, attached: bool, payload: &[u8]) -> Vec<u8> {
    let mut value = format!("--unknown{selector}").into_bytes();
    if attached {
        value.push(b'=');
        value.extend_from_slice(payload);
    }
    value
}

/// Selects one supported negative-number grammar boundary.
const fn negative_number(selector: u8) -> &'static [u8] {
    match selector % 7 {
        0 => b"-0",
        1 => b"-1",
        2 => b"-.5",
        3 => b"-1.",
        4 => b"-1e5",
        5 => b"-1E-5",
        _ => b"-0.0e+0",
    }
}

/// Selects one malformed or unusual flag-like token.
const fn raw_flag_like(selector: u8) -> &'static [u8] {
    match selector % 5 {
        0 => b"---",
        1 => b"--=value",
        2 => b"-=",
        3 => b"-:",
        _ => b"---=value",
    }
}

/// Creates one platform-native argument from generated encoded bytes.
#[cfg(unix)]
fn os_string(bytes: &[u8]) -> OsString {
    OsString::from_vec(bytes.to_vec())
}

/// Creates a portable argument when arbitrary byte strings are not native argv values.
#[cfg(not(unix))]
fn os_string(bytes: &[u8]) -> OsString {
    String::from_utf8_lossy(bytes).into_owned().into()
}

/// Lowers owned test metadata and runs the production parser to exhaustion.
fn production_parse(command: &CommandSpec, argv: &[OsString]) -> ParseRun {
    let long_tables = command
        .flags
        .iter()
        .map(|flag| flag.longs.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let flags = command
        .flags
        .iter()
        .zip(&long_tables)
        .map(|(flag, longs)| Flag {
            key: flag.key,
            name: &flag.name,
            diagnostic: &flag.name,
            help: None,
            longs,
            aliases: &[],
            shorts: &flag.shorts,
            global: false,
            env: None,
            takes_value: flag.takes_value,
            required: false,
            required_if_env_unset: false,
            allow_hyphen_values: flag.allow_hyphen_values,
            allow_negative_numbers: flag.allow_negative_numbers,
        })
        .collect::<Vec<_>>();
    let flag_refs = flags.iter().collect::<Vec<_>>();
    let args = command
        .args
        .iter()
        .map(|arg| Arg {
            key: arg.key,
            name: &arg.name,
            help: None,
            required: arg.required,
            variadic: arg.variadic,
            allow_negative_numbers: arg.allow_negative_numbers,
        })
        .collect::<Vec<_>>();
    let arg_refs = args.iter().collect::<Vec<_>>();
    let table = Command {
        name: "generated",
        about: None,
        flags: &flag_refs,
        args: &arg_refs,
        subcommands: &[],
        key: 1,
        ..Command::EMPTY
    };
    let argv_refs = argv.iter().map(OsString::as_os_str).collect::<Vec<_>>();
    let mut parser = ArgvParser::new(&table, &argv_refs);
    collect_production_trace(&mut parser)
}

/// Collects normalized production output and observes stable terminality.
fn collect_production_trace(parser: &mut ArgvParser<'_, '_, '_>) -> ParseRun {
    let mut trace = Vec::new();
    while let Some(result) = parser.next_event() {
        match result {
            Ok(Event::Action { action, .. }) => {
                assert_eq!(action.kind, ActionKind::Help);
                trace.push(Trace::Error(ErrorTrace::DisplayHelp));
                break;
            }
            Ok(Event::Flag { flag, value }) => {
                trace.push(Trace::Flag { key: flag.key, value: value.map(<[u8]>::to_vec) })
            }
            Ok(Event::Arg { arg, value }) => {
                trace.push(Trace::Arg { key: arg.key, value: value.to_vec() });
            }
            Ok(Event::Command { command }) => {
                panic!("flat-command property unexpectedly selected `{}`", command.name);
            }
            Err(error) => {
                trace.push(Trace::Error(normalize_error(error)));
                break;
            }
        }
    }
    ParseRun {
        trace,
        exhausted_once: parser.next_event().is_none(),
        exhausted_twice: parser.next_event().is_none(),
    }
}

/// Normalizes one public parser error without relying on borrowed storage.
fn normalize_error(error: Error<'_, '_>) -> ErrorTrace {
    match error {
        Error::UnexpectedActionValue { action: _ } => ErrorTrace::UnexpectedFlagValue(0_u64),
        Error::UnknownFlag { token } => ErrorTrace::UnknownFlag(token.to_vec()),
        Error::MissingFlagValue { flag } => ErrorTrace::MissingFlagValue(flag.key),
        Error::UnexpectedFlagValue { flag } => ErrorTrace::UnexpectedFlagValue(flag.key),
        Error::UnexpectedArg { token } => ErrorTrace::UnexpectedArg(token.to_vec()),
        _ => panic!("property harness does not recognize this parser error variant"),
    }
}

/// Runs a deliberately straightforward reference implementation of the argv grammar.
fn reference_parse(command: &CommandSpec, argv: &[Vec<u8>]) -> Vec<Trace> {
    let mut trace = Vec::new();
    let mut position = 0;
    let mut arg_position = 0;
    let mut flags_stopped = false;

    while let Some(token) = argv.get(position) {
        position += 1;
        if !flags_stopped && token.as_slice() == b"--" {
            flags_stopped = true;
            continue;
        }
        if !flags_stopped && matches!(token.as_slice(), b"--help" | b"-h") {
            trace.push(Trace::Error(ErrorTrace::DisplayHelp));
        } else if flags_stopped
            || binds_as_negative_positional(command, arg_position, token)
            || !is_flag_like(token)
        {
            trace.push(reference_word(command, &mut arg_position, token));
        } else if token.starts_with(b"--") {
            trace.push(reference_long(command, argv, &mut position, token));
        } else {
            trace.extend(reference_short(command, argv, &mut position, token));
        }

        if trace.last().is_some_and(Trace::is_error) {
            break;
        }
    }
    trace
}

/// Reports whether a supported negative number belongs to the next positional.
fn binds_as_negative_positional(command: &CommandSpec, arg_position: usize, token: &[u8]) -> bool {
    let declared_numeric_short = matches!(token, [b'-', short]
        if short.is_ascii_digit() && find_short(command, *short).is_some());
    !declared_numeric_short
        && is_negative_number(token)
        && command.args.get(arg_position).is_some_and(|arg| arg.allow_negative_numbers)
}

/// Binds one positional word or produces the terminal overflow error.
fn reference_word(command: &CommandSpec, arg_position: &mut usize, token: &[u8]) -> Trace {
    let Some(arg) = command.args.get(*arg_position) else {
        return Trace::Error(ErrorTrace::UnexpectedArg(token.to_vec()));
    };
    if !arg.variadic {
        *arg_position += 1;
    }
    Trace::Arg { key: arg.key, value: token.to_vec() }
}

/// Parses one long token and applies detached-value policy.
fn reference_long(
    command: &CommandSpec,
    argv: &[Vec<u8>],
    position: &mut usize,
    token: &[u8],
) -> Trace {
    let body = &token[2..];
    let (name, attached) = body
        .iter()
        .position(|byte| *byte == b'=')
        .map_or((body, None), |index| (&body[..index], Some(&body[index + 1..])));
    let Some(flag) = find_long(command, name) else {
        return Trace::Error(ErrorTrace::UnknownFlag(token.to_vec()));
    };
    if !flag.takes_value {
        return attached.map_or(Trace::Flag { key: flag.key, value: None }, |_| {
            Trace::Error(ErrorTrace::UnexpectedFlagValue(flag.key))
        });
    }
    attached.map_or_else(
        || {
            reference_detached(argv, position, flag).map_or_else(
                || Trace::Error(ErrorTrace::MissingFlagValue(flag.key)),
                |value| Trace::Flag { key: flag.key, value: Some(value) },
            )
        },
        |value| Trace::Flag { key: flag.key, value: Some(value.to_vec()) },
    )
}

/// Parses an atomically preflighted short bundle.
fn reference_short(
    command: &CommandSpec,
    argv: &[Vec<u8>],
    position: &mut usize,
    token: &[u8],
) -> Vec<Trace> {
    let mut remaining = &token[1..];
    while let Some((&short, tail)) = remaining.split_first() {
        if short == b'h' {
            remaining = tail;
            continue;
        }
        match find_short(command, short) {
            None => return vec![Trace::Error(ErrorTrace::UnknownFlag(token.to_vec()))],
            Some(flag) if flag.takes_value => break,
            Some(_) => remaining = tail,
        }
    }

    let mut trace = Vec::new();
    remaining = &token[1..];
    while let Some((&short, tail)) = remaining.split_first() {
        if short == b'h' {
            trace.push(Trace::Error(ErrorTrace::DisplayHelp));
            break;
        }
        let flag = find_short(command, short).expect("short bundle was preflighted");
        if !flag.takes_value {
            trace.push(Trace::Flag { key: flag.key, value: None });
            remaining = tail;
            continue;
        }

        if tail.is_empty() {
            trace.push(reference_detached(argv, position, flag).map_or_else(
                || Trace::Error(ErrorTrace::MissingFlagValue(flag.key)),
                |value| Trace::Flag { key: flag.key, value: Some(value) },
            ));
        } else {
            let value = tail.strip_prefix(b"=").unwrap_or(tail);
            trace.push(Trace::Flag { key: flag.key, value: Some(value.to_vec()) });
        }
        break;
    }
    trace
}

/// Applies detached-value acceptance without consuming a refused token.
fn reference_detached(argv: &[Vec<u8>], position: &mut usize, flag: &FlagSpec) -> Option<Vec<u8>> {
    let value = argv.get(*position)?;
    if !flag.allow_hyphen_values
        && is_flag_like(value)
        && !(flag.allow_negative_numbers && is_negative_number(value))
    {
        return None;
    }
    *position += 1;
    Some(value.clone())
}

/// Looks up one exact long spelling.
fn find_long<'a>(command: &'a CommandSpec, name: &[u8]) -> Option<&'a FlagSpec> {
    command.flags.iter().find(|flag| flag.longs.iter().any(|long| long.as_bytes() == name))
}

/// Looks up one exact short spelling.
fn find_short(command: &CommandSpec, short: u8) -> Option<&FlagSpec> {
    command.flags.iter().find(|flag| flag.shorts.contains(&short))
}

/// Reports whether a token is flag-like rather than the conventional lone dash.
fn is_flag_like(token: &[u8]) -> bool {
    token.starts_with(b"-") && token.len() > 1
}

/// Reports whether a token has the supported negative-number grammar.
fn is_negative_number(token: &[u8]) -> bool {
    token.strip_prefix(b"-").is_some_and(is_number)
}

/// Recognizes the parser contract's decimal and scientific-notation grammar.
fn is_number(token: &[u8]) -> bool {
    let (mantissa, exponent) = token
        .iter()
        .position(|byte| matches!(byte, b'e' | b'E'))
        .map_or((token, None), |index| (&token[..index], Some(&token[index + 1..])));
    let mut digit = false;
    let mut dot = false;
    for &byte in mantissa {
        match byte {
            b'0'..=b'9' => digit = true,
            b'.' if !dot => dot = true,
            _ => return false,
        }
    }
    if !digit {
        return false;
    }
    exponent.is_none_or(|exponent| {
        let digits =
            exponent.strip_prefix(b"+").or_else(|| exponent.strip_prefix(b"-")).unwrap_or(exponent);
        !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
    })
}

/// Parses all input after `--` through one variadic positional table entry.
fn passthrough_parse(argv: &[OsString]) -> ParseRun {
    /// Variadic table entry used by the byte-preservation invariant.
    static VALUE: Arg<'static> = Arg {
        key: 0x3000,
        name: "value",
        help: None,
        required: false,
        variadic: true,
        allow_negative_numbers: false,
    };
    /// Minimal command used by the byte-preservation invariant.
    static COMMAND: Command<'static> = Command {
        name: "passthrough",
        about: None,
        flags: &[],
        args: &[&VALUE],
        subcommands: &[],
        key: 2,
        ..Command::EMPTY
    };

    let separator = OsStr::new("--");
    let mut refs = Vec::with_capacity(argv.len() + 1);
    refs.push(separator);
    refs.extend(argv.iter().map(OsString::as_os_str));
    let mut parser = ArgvParser::new(&COMMAND, &refs);
    collect_production_trace(&mut parser)
}

mod command;

#[cfg(feature = "derive")]
mod typed;
