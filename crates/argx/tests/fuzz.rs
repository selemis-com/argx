//! Model-based fuzz testing for raw parsing and typed argument binding.
//!
//! Proptest generates valid command tables and arbitrary operating-system arguments. The raw
//! parser is checked against a deliberately separate reference grammar, while typed properties
//! exercise generated binding, conversion, entry-point equivalence, and byte preservation. The
//! fuzzing campaign is isolated from deterministic tests so its size and seed can be controlled
//! without changing the ordinary test suite.

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt as _;
    #[cfg(all(feature = "derive", unix))]
    use std::path::PathBuf;
    use std::{
        cell::RefCell,
        env,
        ffi::{OsStr, OsString},
        fmt::{self, Display, Formatter, Write as _},
    };

    use argx::__private::{Arg, ArgvParser, Command, Error, Event, Flag};
    #[cfg(feature = "derive")]
    use argx::{Error as TypedError, Parser as _};
    use proptest::{
        collection,
        prelude::*,
        sample,
        test_runner::{Config, FileFailurePersistence, TestRunner},
    };

    /// Visible short spellings used to build unique generated command tables.
    const SHORT_ALPHABET: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+[]{};,.?/|~";

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
    }

    impl TokenKind {
        /// Number of variants represented in coverage counters.
        const COUNT: usize = 13;

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
        /// Terminal parser failure.
        Error(ErrorTrace),
    }

    impl Trace {
        /// Reports whether this item is a terminal failure.
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
    }

    impl ErrorTrace {
        /// Stable coverage-counter index for this error class.
        const fn index(&self) -> usize {
            match self {
                Self::UnknownFlag(_) => 0,
                Self::MissingFlagValue(_) => 1,
                Self::UnexpectedFlagValue(_) => 2,
                Self::UnexpectedArg(_) => 3,
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
        /// Counts for each terminal error class.
        errors: [usize; 4],
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
            |(takes_value, allow_hyphen_values, allow_negative_numbers, spelling_mode)| {
                FlagPolicy {
                    takes_value,
                    allow_hyphen_values: takes_value && allow_hyphen_values,
                    allow_negative_numbers: takes_value && allow_negative_numbers,
                    spelling_mode,
                }
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
                            key: 0x2000
                                + u64::try_from(index).expect("generated index fits in u64"),
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
            TokenKind::UnknownLong => {
                unknown_long(token.selectors[0], token.equals, &token.payload)
            }
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
                longs,
                shorts: &flag.shorts,
                takes_value: flag.takes_value,
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
                required: arg.required,
                variadic: arg.variadic,
                allow_negative_numbers: arg.allow_negative_numbers,
            })
            .collect::<Vec<_>>();
        let arg_refs = args.iter().collect::<Vec<_>>();
        let table = Command {
            name: "generated",
            flags: &flag_refs,
            args: &arg_refs,
            subcommands: &[],
            key: 1,
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
            if flags_stopped
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
    fn binds_as_negative_positional(
        command: &CommandSpec,
        arg_position: usize,
        token: &[u8],
    ) -> bool {
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
            match find_short(command, short) {
                None => return vec![Trace::Error(ErrorTrace::UnknownFlag(token.to_vec()))],
                Some(flag) if flag.takes_value => break,
                Some(_) => remaining = tail,
            }
        }

        let mut trace = Vec::new();
        remaining = &token[1..];
        while let Some((&short, tail)) = remaining.split_first() {
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
    fn reference_detached(
        argv: &[Vec<u8>],
        position: &mut usize,
        flag: &FlagSpec,
    ) -> Option<Vec<u8>> {
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
            let digits = exponent
                .strip_prefix(b"+")
                .or_else(|| exponent.strip_prefix(b"-"))
                .unwrap_or(exponent);
            !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
        })
    }

    /// Parses all input after `--` through one variadic positional table entry.
    fn passthrough_parse(argv: &[OsString]) -> ParseRun {
        /// Variadic table entry used by the byte-preservation invariant.
        static VALUE: Arg<'static> = Arg {
            key: 0x3000,
            name: "value",
            required: false,
            variadic: true,
            allow_negative_numbers: false,
        };
        /// Minimal command used by the byte-preservation invariant.
        static COMMAND: Command<'static> =
            Command { name: "passthrough", flags: &[], args: &[&VALUE], subcommands: &[], key: 2 };

        let separator = OsStr::new("--");
        let mut refs = Vec::with_capacity(argv.len() + 1);
        refs.push(separator);
        refs.extend(argv.iter().map(OsString::as_os_str));
        let mut parser = ArgvParser::new(&COMMAND, &refs);
        collect_production_trace(&mut parser)
    }

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
            flags: &[&ROOT_VERBOSE],
            args: &[&ROOT_VALUE],
            subcommands: &[&ADD, &CONFIG, &ROOT_STATUS],
            key: 0x4100,
        };

        let owned =
            tokens.iter().map(|token| OsString::from(tree_token_text(*token))).collect::<Vec<_>>();
        let refs = owned.iter().map(OsString::as_os_str).collect::<Vec<_>>();
        let mut parser = ArgvParser::new(&ROOT, &refs);
        let mut trace = Vec::new();
        while let Some(item) = parser.next_event() {
            match item {
                Ok(Event::Flag { flag, .. }) => trace.push(TreeTrace::Flag(flag.key)),
                Ok(Event::Arg { arg, value }) => {
                    trace.push(TreeTrace::Arg(arg.key, value.to_vec()))
                }
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
            "[parser fuzz] terminal errors: unknown_flag={} | missing_value={} | unexpected_value={} | unexpected_arg={}",
            coverage.errors[0], coverage.errors[1], coverage.errors[2], coverage.errors[3],
        );
        eprintln!(
            "[parser fuzz] generated token classes: word={} | known_long={} | known_long_attached={} | known_short={} | known_short_attached={} | short_bundle={} | unknown_long={} | unknown_short={} | separator={} | negative={} | lone_dash={} | empty={} | raw_flag_like={}",
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
        eprintln!(
            "[command fuzz] events: flags={} | positionals={}",
            coverage.flags, coverage.args,
        );
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

    /// Typed command used to fuzz end-to-end binding and entry-point behavior.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Parser)]
    struct TypedRoundTrip {
        /// Optional switch represented by presence or absence of one flag occurrence.
        #[argx(long)]
        verbose: bool,
        /// Optional scalar converted through `FromStr`.
        #[argx(long)]
        number: Option<i64>,
        /// Repeatable UTF-8 values whose order must be preserved.
        #[argx(long)]
        value: Vec<String>,
        /// Optional repeated UTF-8 values that preserve absence.
        #[argx(long)]
        optional_value: Option<Vec<String>>,
        /// Required positional UTF-8 value.
        input: String,
        /// Remaining positional UTF-8 values.
        rest: Vec<String>,
    }

    /// Deepest reusable group used to fuzz recursive flatten binding.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
    struct FlattenLeaf {
        /// Switch owned by the deepest flattened group.
        #[argx(long)]
        leaf_switch: bool,
        /// Optional scalar owned by the deepest flattened group.
        #[argx(long)]
        leaf_number: Option<i64>,
        /// Repeatable values owned by the deepest flattened group.
        #[argx(long)]
        leaf_value: Vec<String>,
        /// Positional contributed between the root's own positionals.
        middle: String,
    }

    /// Intermediate group used to prove recursive flatten delegation.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
    struct FlattenNested {
        /// Child declaration composed recursively.
        #[argx(flatten)]
        leaf: FlattenLeaf,
        /// Optional collection owned by the intermediate group.
        #[argx(long)]
        nested_value: Option<Vec<String>>,
    }

    /// Sibling flattened group used to exercise independent partial state.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
    struct FlattenSibling {
        /// Optional value owned by a separate flattened declaration.
        #[argx(long)]
        sibling: Option<String>,
    }

    /// Root command used for recursive and sibling flatten round trips.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Parser)]
    struct FlattenRoundTrip {
        /// Root-owned switch.
        #[argx(long)]
        root_switch: bool,
        /// Root positional before flattened positional tables.
        head: String,
        /// Recursively flattened declaration.
        #[argx(flatten)]
        nested: FlattenNested,
        /// Independent sibling flattened declaration.
        #[argx(flatten)]
        sibling: FlattenSibling,
        /// Root positional after flattened positional tables.
        tail: String,
        /// Root-owned trailing values.
        rest: Vec<String>,
    }

    /// Required group used by cross-flatten error-precedence properties.
    #[cfg(feature = "derive")]
    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct FlattenRequired {
        /// Required named value deliberately left absent in precedence cases.
        #[argx(long)]
        required: String,
    }

    /// Scalar group used by cross-flatten error-precedence properties.
    #[cfg(feature = "derive")]
    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct FlattenScalar {
        /// Optional scalar whose repeated occurrences are checked before requiredness.
        #[argx(long)]
        port: Option<u16>,
    }

    /// Root combining independent required and scalar groups for precedence checks.
    #[cfg(feature = "derive")]
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct FlattenErrors {
        /// Group containing a missing required field.
        #[argx(flatten)]
        required: FlattenRequired,
        /// Group containing repeated or invalid scalar values.
        #[argx(flatten)]
        scalar: FlattenScalar,
    }

    /// Shared payload fields used by generated subcommand round trips.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
    struct SubcommandShared {
        /// Switch composed into one selected child.
        #[argx(long)]
        dry_run: bool,
        /// Repeatable child values.
        #[argx(long)]
        tag: Vec<String>,
    }

    /// Payload for the first generated root subcommand.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
    struct SubcommandAdd {
        /// Reusable flattened child fields.
        #[argx(flatten)]
        shared: SubcommandShared,
        /// Required child positional.
        name: String,
    }

    /// Payload for a nested generated subcommand.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
    struct SubcommandGet {
        /// Required nested positional.
        key: String,
    }

    /// Nested command set used by generated subcommand round trips.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Subcommand)]
    enum SubcommandNested {
        /// Nested payload command.
        Get(SubcommandGet),
        /// Nested unit command.
        Status,
    }

    /// Payload selecting a second-level command.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
    struct SubcommandConfig {
        /// Parent-child switch.
        #[argx(long)]
        local: bool,
        /// Required nested command.
        #[argx(subcommand)]
        command: SubcommandNested,
    }

    /// Root generated command set.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Subcommand)]
    enum SubcommandChoice {
        /// Payload with flatten composition.
        Add(SubcommandAdd),
        /// Payload with another command set.
        Config(SubcommandConfig),
        /// Root unit command.
        Status,
    }

    /// Root parser used by subcommand round-trip properties.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, PartialEq, Eq, argx::Parser)]
    struct SubcommandRoundTrip {
        /// Root switch parsed before command selection.
        #[argx(long)]
        verbose: bool,
        /// Root positional parsed before command selection.
        workspace: String,
        /// Selected root command.
        #[argx(subcommand)]
        command: SubcommandChoice,
    }

    /// Scalar payload used by cross-command precedence properties.
    #[cfg(feature = "derive")]
    #[derive(Debug, PartialEq, Eq, argx::Args)]
    struct SubcommandErrorPayload {
        /// Scalar whose duplicate occurrences must beat conversion.
        #[argx(long)]
        port: Option<u16>,
        /// Required value whose absence must beat conversion.
        #[argx(long)]
        required: String,
    }

    /// One command branch used by precedence properties.
    #[cfg(feature = "derive")]
    #[derive(Debug, PartialEq, Eq, argx::Subcommand)]
    enum SubcommandErrorChoice {
        /// Payload under test.
        Child(SubcommandErrorPayload),
    }

    /// Root combining parent cardinality and child semantic errors.
    #[cfg(feature = "derive")]
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct SubcommandErrors {
        /// Parent scalar occurrence state.
        #[argx(long)]
        root: bool,
        /// Required selected command.
        #[argx(subcommand)]
        command: SubcommandErrorChoice,
    }

    /// Generated representable root command value.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone)]
    enum SubcommandGeneratedChoice {
        /// Root add payload.
        Add {
            /// Flattened switch.
            dry_run: bool,
            /// Repeatable flattened values.
            tags: Vec<String>,
            /// Required child positional.
            name: String,
        },
        /// Nested config/get payload.
        ConfigGet {
            /// Config-level switch.
            local: bool,
            /// Nested required positional.
            key: String,
        },
        /// Nested config/status unit command.
        ConfigStatus {
            /// Config-level switch.
            local: bool,
        },
        /// Root unit command.
        Status,
    }

    /// One generated complete subcommand value.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone)]
    struct SubcommandGenerated {
        /// Root switch.
        verbose: bool,
        /// Root positional.
        workspace: String,
        /// Generated branch and payload values.
        command: SubcommandGeneratedChoice,
    }

    /// Typed command used to fuzz deferred scalar cardinality.
    #[cfg(feature = "derive")]
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct TypedScalar {
        /// Optional scalar whose second occurrence must win over conversion as a duplicate error.
        #[argx(long)]
        port: Option<u16>,
    }

    /// Typed positional OS value used by the Unix byte-preservation property.
    #[cfg(all(feature = "derive", unix))]
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct TypedPath {
        /// Operating-system-backed positional value.
        path: PathBuf,
    }

    /// Typed UTF-8 positional used by the invalid-byte rejection property.
    #[cfg(all(feature = "derive", unix))]
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct TypedText {
        /// UTF-8 positional value.
        value: String,
    }

    /// Typed named OS value used by the Unix attached-value preservation property.
    #[cfg(all(feature = "derive", unix))]
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct TypedPathFlag {
        /// Operating-system-backed named value.
        #[argx(long)]
        path: PathBuf,
    }

    /// Typed named UTF-8 value used by the Unix attached-value rejection property.
    #[cfg(all(feature = "derive", unix))]
    #[derive(Debug, PartialEq, Eq, argx::Parser)]
    struct TypedTextFlag {
        /// UTF-8 named value.
        #[argx(long)]
        value: String,
    }

    /// Aggregate measurements for the typed round-trip campaign.
    #[cfg(feature = "derive")]
    #[derive(Debug, Default)]
    struct TypedRoundTripCoverage {
        /// Arguments-only, complete-argv, and non-ASCII argv0 case counts.
        entry_points: [usize; 3],
        /// False/true switches and absent/present optional scalar counts.
        scalars: [usize; 4],
        /// Empty/items counts for `Vec`, `Option<Vec>`, and trailing positionals.
        collections: [usize; 8],
        /// Total, empty, non-ASCII, and Unicode-scalar string counts.
        strings: [usize; 4],
    }

    #[cfg(feature = "derive")]
    impl TypedRoundTripCoverage {
        /// Records one successfully verified round-trip case.
        fn record(&mut self, value: &TypedRoundTrip, argv0: &str) {
            self.entry_points[0] += 1;
            self.entry_points[1] += 1;
            if !argv0.is_ascii() {
                self.entry_points[2] += 1;
            }
            if value.verbose {
                self.scalars[1] += 1;
            } else {
                self.scalars[0] += 1;
            }
            if value.number.is_some() {
                self.scalars[3] += 1;
            } else {
                self.scalars[2] += 1;
            }

            if value.value.is_empty() {
                self.collections[0] += 1;
            }
            self.collections[1] += value.value.len();
            match &value.optional_value {
                None => self.collections[2] += 1,
                Some(items) => {
                    self.collections[3] += 1;
                    self.collections[4] += items.iter().filter(|item| item.is_empty()).count();
                    self.collections[5] += items.len();
                }
            }
            if value.rest.is_empty() {
                self.collections[6] += 1;
            }
            self.collections[7] += value.rest.len();

            self.record_string(&value.input);
            for item in &value.value {
                self.record_string(item);
            }
            if let Some(items) = &value.optional_value {
                for item in items {
                    self.record_string(item);
                }
            }
            for item in &value.rest {
                self.record_string(item);
            }
        }

        /// Records one generated typed UTF-8 value.
        fn record_string(&mut self, value: &str) {
            self.strings[0] += 1;
            if value.is_empty() {
                self.strings[1] += 1;
            }
            if !value.is_ascii() {
                self.strings[2] += 1;
            }
            self.strings[3] += value.chars().count();
        }
    }

    /// Aggregate measurements for recursive flatten round-trip fuzzing.
    #[cfg(feature = "derive")]
    #[derive(Debug, Default)]
    struct FlattenCoverage {
        /// Root and leaf switch false/true counts.
        switches: [usize; 4],
        /// Leaf scalar absent/present and sibling scalar absent/present counts.
        optionals: [usize; 4],
        /// Leaf, nested optional, and trailing collection counts.
        collections: [usize; 7],
        /// Total, empty, non-ASCII, and Unicode-scalar counts across flattened values.
        strings: [usize; 4],
        /// Arguments-only and complete-argv parses verified.
        entry_points: [usize; 2],
    }

    #[cfg(feature = "derive")]
    impl FlattenCoverage {
        /// Records one successfully verified flattened round trip.
        fn record(&mut self, value: &FlattenRoundTrip) {
            self.entry_points[0] += 1;
            self.entry_points[1] += 1;
            let root_switch = if value.root_switch { 1 } else { 0 };
            let leaf_switch = if value.nested.leaf.leaf_switch { 1 } else { 0 };
            let leaf_number = if value.nested.leaf.leaf_number.is_some() { 1 } else { 0 };
            let sibling = if value.sibling.sibling.is_some() { 1 } else { 0 };
            self.switches[root_switch] += 1;
            self.switches[2 + leaf_switch] += 1;
            self.optionals[leaf_number] += 1;
            self.optionals[2 + sibling] += 1;

            if value.nested.leaf.leaf_value.is_empty() {
                self.collections[0] += 1;
            }
            self.collections[1] += value.nested.leaf.leaf_value.len();
            match &value.nested.nested_value {
                None => self.collections[2] += 1,
                Some(items) => {
                    self.collections[3] += 1;
                    self.collections[4] += items.len();
                }
            }
            if value.rest.is_empty() {
                self.collections[5] += 1;
            }
            self.collections[6] += value.rest.len();

            self.record_string(&value.head);
            self.record_string(&value.nested.leaf.middle);
            self.record_string(&value.tail);
            if let Some(value) = &value.sibling.sibling {
                self.record_string(value);
            }
            for value in &value.nested.leaf.leaf_value {
                self.record_string(value);
            }
            if let Some(values) = &value.nested.nested_value {
                for value in values {
                    self.record_string(value);
                }
            }
            for value in &value.rest {
                self.record_string(value);
            }
        }

        /// Records one generated UTF-8 value from a flattened field.
        fn record_string(&mut self, value: &str) {
            self.strings[0] += 1;
            if value.is_empty() {
                self.strings[1] += 1;
            }
            if !value.is_ascii() {
                self.strings[2] += 1;
            }
            self.strings[3] += value.chars().count();
        }
    }

    /// Aggregate measurements for nested subcommand round-trip fuzzing.
    #[cfg(feature = "derive")]
    #[derive(Debug, Default)]
    struct SubcommandCoverage {
        /// Root add, config/get, config/status, and root status selections.
        variants: [usize; 4],
        /// Root and child switch false/true observations.
        switches: [usize; 6],
        /// Empty tag vectors and total tag items.
        collections: [usize; 2],
        /// Arguments-only and complete-argv parses verified.
        entry_points: [usize; 2],
        /// Total, empty, non-ASCII, and Unicode-scalar string counts.
        strings: [usize; 4],
    }

    #[cfg(feature = "derive")]
    impl SubcommandCoverage {
        /// Records one verified generated command tree.
        fn record(&mut self, value: &SubcommandGenerated) {
            self.entry_points[0] += 1;
            self.entry_points[1] += 1;
            self.switches[usize::from(value.verbose)] += 1;
            self.record_string(&value.workspace);
            match &value.command {
                SubcommandGeneratedChoice::Add { dry_run, tags, name } => {
                    self.variants[0] += 1;
                    self.switches[2 + usize::from(*dry_run)] += 1;
                    if tags.is_empty() {
                        self.collections[0] += 1;
                    }
                    self.collections[1] += tags.len();
                    for tag in tags {
                        self.record_string(tag);
                    }
                    self.record_string(name);
                }
                SubcommandGeneratedChoice::ConfigGet { local, key } => {
                    self.variants[1] += 1;
                    self.switches[4 + usize::from(*local)] += 1;
                    self.record_string(key);
                }
                SubcommandGeneratedChoice::ConfigStatus { local } => {
                    self.variants[2] += 1;
                    self.switches[4 + usize::from(*local)] += 1;
                }
                SubcommandGeneratedChoice::Status => self.variants[3] += 1,
            }
        }

        /// Records one generated UTF-8 value.
        fn record_string(&mut self, value: &str) {
            self.strings[0] += 1;
            if value.is_empty() {
                self.strings[1] += 1;
            }
            if !value.is_ascii() {
                self.strings[2] += 1;
            }
            self.strings[3] += value.chars().count();
        }
    }

    /// Aggregate measurements for cross-command error-precedence fuzzing.
    #[cfg(feature = "derive")]
    #[derive(Debug, Default)]
    struct SubcommandErrorCoverage {
        /// Scalar classes generated for the child value.
        classes: [usize; ScalarTextKind::COUNT],
        /// Raw-over-parent, child-duplicate, required-over-conversion, and missing-command checks.
        checks: [usize; 4],
    }

    /// Aggregate measurements for cross-flatten error-precedence fuzzing.
    #[cfg(feature = "derive")]
    #[derive(Debug, Default)]
    struct FlattenErrorCoverage {
        /// Scalar semantic classes used as the first occurrence.
        classes: [usize; ScalarTextKind::COUNT],
        /// Duplicate, syntax, requiredness, and child-conversion checks completed.
        checks: [usize; 4],
    }

    /// Semantic class deliberately generated for scalar conversion fuzzing.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone, Copy)]
    enum ScalarTextKind {
        /// Text that parses successfully as `u16`.
        Valid,
        /// Unsigned decimal text above the `u16` range.
        Overflow,
        /// Negative decimal text, which is invalid for `u16`.
        Negative,
        /// Empty text.
        Empty,
        /// Arbitrary non-numeric Unicode text.
        NonNumeric,
    }

    #[cfg(feature = "derive")]
    impl ScalarTextKind {
        /// Number of semantic classes represented in coverage counters.
        const COUNT: usize = 5;

        /// Stable coverage-counter index for this scalar class.
        const fn index(self) -> usize {
            match self {
                Self::Valid => 0,
                Self::Overflow => 1,
                Self::Negative => 2,
                Self::Empty => 3,
                Self::NonNumeric => 4,
            }
        }
    }

    /// One generated scalar input paired with its intended semantic class.
    #[cfg(feature = "derive")]
    #[derive(Debug, Clone)]
    struct ScalarText {
        /// Generated text supplied to the typed parser.
        value: String,
        /// Semantic class selected by the generator.
        kind: ScalarTextKind,
    }

    /// Aggregate measurements for scalar conversion and precedence fuzzing.
    #[cfg(feature = "derive")]
    #[derive(Debug, Default)]
    struct TypedScalarCoverage {
        /// Counts for each generated first-value semantic class.
        classes: [usize; ScalarTextKind::COUNT],
        /// Parsed and invalid single-value outcomes.
        outcomes: [usize; 2],
        /// Duplicate-over-conversion and syntax-over-duplicate checks.
        precedence: [usize; 2],
        /// First values containing non-ASCII Unicode.
        non_ascii_values: usize,
    }

    #[cfg(feature = "derive")]
    impl TypedScalarCoverage {
        /// Records one successfully verified scalar-precedence case.
        fn record(&mut self, first: &ScalarText, parsed: bool) {
            self.classes[first.kind.index()] += 1;
            if parsed {
                self.outcomes[0] += 1;
            } else {
                self.outcomes[1] += 1;
            }
            self.precedence[0] += 1;
            self.precedence[1] += 1;
            if !first.value.is_ascii() {
                self.non_ascii_values += 1;
            }
        }
    }

    /// Aggregate measurements for Unix non-UTF-8 typed binding fuzzing.
    #[cfg(all(feature = "derive", unix))]
    #[derive(Debug, Default)]
    struct TypedOsCoverage {
        /// Total bytes, high-bit bytes, and values containing NUL.
        generated: [usize; 3],
        /// Positional and attached `PathBuf` round trips.
        path_round_trips: [usize; 2],
        /// Positional and attached `String` rejections.
        text_rejections: [usize; 2],
    }

    #[cfg(all(feature = "derive", unix))]
    impl TypedOsCoverage {
        /// Records one successfully verified non-UTF-8 case.
        fn record(&mut self, bytes: &[u8]) {
            self.generated[0] += bytes.len();
            self.generated[1] += bytes.iter().filter(|byte| **byte >= 0x80).count();
            if bytes.contains(&0) {
                self.generated[2] += 1;
            }
            self.path_round_trips[0] += 1;
            self.path_round_trips[1] += 1;
            self.text_rejections[0] += 1;
            self.text_rejections[1] += 1;
        }
    }

    /// Generates bounded arbitrary Unicode strings for typed-binding campaigns.
    #[cfg(feature = "derive")]
    fn typed_string_strategy() -> impl Strategy<Value = String> {
        collection::vec(any::<char>(), 0..=24)
            .prop_map(|characters| characters.into_iter().collect::<String>())
    }

    /// Generates one complete set of values representable by [`TypedRoundTrip`].
    #[cfg(feature = "derive")]
    fn typed_round_trip_strategy() -> impl Strategy<Value = TypedRoundTrip> {
        (
            any::<bool>(),
            proptest::option::of(any::<i64>()),
            collection::vec(typed_string_strategy(), 0..=8),
            prop_oneof![
                Just(None),
                collection::vec(typed_string_strategy(), 1..=8).prop_map(Some),
            ],
            typed_string_strategy(),
            collection::vec(typed_string_strategy(), 0..=8),
        )
            .prop_map(|(verbose, number, value, optional_value, input, rest)| TypedRoundTrip {
                verbose,
                number,
                value,
                optional_value,
                input,
                rest,
            })
    }

    /// Renders typed values into an unambiguous argv sequence for end-to-end round trips.
    #[cfg(feature = "derive")]
    fn typed_round_trip_argv(value: &TypedRoundTrip) -> Vec<OsString> {
        let mut argv = Vec::new();
        if value.verbose {
            argv.push(OsString::from("--verbose"));
        }
        if let Some(number) = value.number {
            argv.push(OsString::from(format!("--number={number}")));
        }
        argv.extend(value.value.iter().map(|item| OsString::from(format!("--value={item}"))));
        if let Some(optional_values) = &value.optional_value {
            argv.extend(
                optional_values
                    .iter()
                    .map(|item| OsString::from(format!("--optional-value={item}"))),
            );
        }
        argv.push(OsString::from("--"));
        argv.push(OsString::from(value.input.as_str()));
        argv.extend(value.rest.iter().map(|item| OsString::from(item.as_str())));
        argv
    }

    /// Generates values spanning root, recursive, and sibling flattened declarations.
    #[cfg(feature = "derive")]
    fn flatten_round_trip_strategy() -> impl Strategy<Value = FlattenRoundTrip> {
        (
            any::<bool>(),
            typed_string_strategy(),
            any::<bool>(),
            proptest::option::of(any::<i64>()),
            collection::vec(typed_string_strategy(), 0..=8),
            typed_string_strategy(),
            prop_oneof![
                Just(None),
                collection::vec(typed_string_strategy(), 1..=8).prop_map(Some),
            ],
            proptest::option::of(typed_string_strategy()),
            typed_string_strategy(),
            collection::vec(typed_string_strategy(), 0..=8),
        )
            .prop_map(
                |(
                    root_switch,
                    head,
                    leaf_switch,
                    leaf_number,
                    leaf_value,
                    middle,
                    nested_value,
                    sibling,
                    tail,
                    rest,
                )| FlattenRoundTrip {
                    root_switch,
                    head,
                    nested: FlattenNested {
                        leaf: FlattenLeaf { leaf_switch, leaf_number, leaf_value, middle },
                        nested_value,
                    },
                    sibling: FlattenSibling { sibling },
                    tail,
                    rest,
                },
            )
    }

    /// Renders flattened typed values into an unambiguous argv sequence.
    #[cfg(feature = "derive")]
    fn flatten_round_trip_argv(value: &FlattenRoundTrip) -> Vec<OsString> {
        let mut argv = Vec::new();
        if value.root_switch {
            argv.push(OsString::from("--root-switch"));
        }
        if value.nested.leaf.leaf_switch {
            argv.push(OsString::from("--leaf-switch"));
        }
        if let Some(number) = value.nested.leaf.leaf_number {
            argv.push(OsString::from(format!("--leaf-number={number}")));
        }
        argv.extend(
            value
                .nested
                .leaf
                .leaf_value
                .iter()
                .map(|item| OsString::from(format!("--leaf-value={item}"))),
        );
        if let Some(values) = &value.nested.nested_value {
            argv.extend(values.iter().map(|item| OsString::from(format!("--nested-value={item}"))));
        }
        if let Some(sibling) = &value.sibling.sibling {
            argv.push(OsString::from(format!("--sibling={sibling}")));
        }
        argv.push(OsString::from("--"));
        argv.push(OsString::from(value.head.as_str()));
        argv.push(OsString::from(value.nested.leaf.middle.as_str()));
        argv.push(OsString::from(value.tail.as_str()));
        argv.extend(value.rest.iter().map(|item| OsString::from(item.as_str())));
        argv
    }

    /// Generates complete values spanning unit, payload, flatten, and nested command branches.
    #[cfg(feature = "derive")]
    fn subcommand_round_trip_strategy() -> impl Strategy<Value = SubcommandGenerated> {
        let branch = prop_oneof![
            4 => (
                any::<bool>(),
                collection::vec(typed_string_strategy(), 0..=8),
                typed_string_strategy(),
            )
                .prop_map(|(dry_run, tags, name)| SubcommandGeneratedChoice::Add {
                    dry_run,
                    tags,
                    name,
                }),
            3 => (any::<bool>(), typed_string_strategy()).prop_map(|(local, key)| {
                SubcommandGeneratedChoice::ConfigGet { local, key }
            }),
            2 => any::<bool>().prop_map(|local| {
                SubcommandGeneratedChoice::ConfigStatus { local }
            }),
            2 => Just(SubcommandGeneratedChoice::Status),
        ];

        (any::<bool>(), typed_string_strategy(), branch).prop_map(
            |(verbose, workspace, command)| SubcommandGenerated {
                verbose,
                workspace: format!("workspace:{workspace}"),
                command,
            },
        )
    }

    /// Renders one generated command tree into an unambiguous argv sequence.
    #[cfg(feature = "derive")]
    fn subcommand_round_trip_argv(value: &SubcommandGenerated) -> Vec<OsString> {
        let mut argv = Vec::new();
        if value.verbose {
            argv.push(OsString::from("--verbose"));
        }
        argv.push(OsString::from(value.workspace.as_str()));
        match &value.command {
            SubcommandGeneratedChoice::Add { dry_run, tags, name } => {
                argv.push(OsString::from("add"));
                if *dry_run {
                    argv.push(OsString::from("--dry-run"));
                }
                argv.extend(tags.iter().map(|tag| OsString::from(format!("--tag={tag}"))));
                argv.push(OsString::from("--"));
                argv.push(OsString::from(name.as_str()));
            }
            SubcommandGeneratedChoice::ConfigGet { local, key } => {
                argv.push(OsString::from("config"));
                if *local {
                    argv.push(OsString::from("--local"));
                }
                argv.push(OsString::from("get"));
                argv.push(OsString::from("--"));
                argv.push(OsString::from(key.as_str()));
            }
            SubcommandGeneratedChoice::ConfigStatus { local } => {
                argv.push(OsString::from("config"));
                if *local {
                    argv.push(OsString::from("--local"));
                }
                argv.push(OsString::from("status"));
            }
            SubcommandGeneratedChoice::Status => argv.push(OsString::from("status")),
        }
        argv
    }

    /// Converts generated expected data into the derived destination value.
    #[cfg(feature = "derive")]
    fn expected_subcommand(value: &SubcommandGenerated) -> SubcommandRoundTrip {
        let command = match &value.command {
            SubcommandGeneratedChoice::Add { dry_run, tags, name } => {
                SubcommandChoice::Add(SubcommandAdd {
                    shared: SubcommandShared { dry_run: *dry_run, tag: tags.clone() },
                    name: name.clone(),
                })
            }
            SubcommandGeneratedChoice::ConfigGet { local, key } => {
                SubcommandChoice::Config(SubcommandConfig {
                    local: *local,
                    command: SubcommandNested::Get(SubcommandGet { key: key.clone() }),
                })
            }
            SubcommandGeneratedChoice::ConfigStatus { local } => {
                SubcommandChoice::Config(SubcommandConfig {
                    local: *local,
                    command: SubcommandNested::Status,
                })
            }
            SubcommandGeneratedChoice::Status => SubcommandChoice::Status,
        };
        SubcommandRoundTrip { verbose: value.verbose, workspace: value.workspace.clone(), command }
    }

    /// Generates scalar text across meaningful `u16` conversion classes.
    #[cfg(feature = "derive")]
    fn scalar_text_strategy() -> impl Strategy<Value = ScalarText> {
        prop_oneof![
            4 => any::<u16>().prop_map(|value| ScalarText {
                value: value.to_string(),
                kind: ScalarTextKind::Valid,
            }),
            2 => (65_536_u32..=1_065_535_u32).prop_map(|value| ScalarText {
                value: value.to_string(),
                kind: ScalarTextKind::Overflow,
            }),
            2 => any::<u16>().prop_map(|value| {
                let magnitude = u32::from(value) + 1;
                ScalarText {
                    value: format!("-{magnitude}"),
                    kind: ScalarTextKind::Negative,
                }
            }),
            1 => Just(ScalarText { value: String::new(), kind: ScalarTextKind::Empty }),
            4 => (
                any::<char>().prop_filter("first character must be non-numeric", |character| {
                    !character.is_ascii_digit() && !matches!(*character, '-' | '+')
                }),
                collection::vec(any::<char>(), 0..=23),
            )
                .prop_map(|(first, rest)| {
                    let value = std::iter::once(first).chain(rest).collect::<String>();
                    ScalarText { value, kind: ScalarTextKind::NonNumeric }
                }),
        ]
    }

    /// Generates encoded Unix values that are not valid UTF-8.
    #[cfg(all(feature = "derive", unix))]
    fn invalid_utf8_strategy() -> impl Strategy<Value = Vec<u8>> {
        collection::vec(any::<u8>(), 1..=48)
            .prop_filter("generated bytes must be invalid UTF-8", |value| {
                std::str::from_utf8(value).is_err()
            })
    }

    /// Fuzzes typed binding round trips and the argv0-vs-args entry-point contract.
    #[cfg(feature = "derive")]
    #[test]
    fn typed_binding_round_trips_generated_values() {
        let strategy = (typed_round_trip_strategy(), typed_string_strategy());
        let config = proptest_config("typed_binding_round_trips_generated_values");
        let cases = config.cases;
        let coverage = RefCell::new(TypedRoundTripCoverage::default());
        let mut runner = TestRunner::new(config);

        let result = runner.run(&strategy, |(expected, argv0)| {
            let argv = typed_round_trip_argv(&expected);
            let parsed = TypedRoundTrip::try_parse_args(argv.clone());
            prop_assert_eq!(parsed, Ok(expected.clone()));

            let mut complete = Vec::with_capacity(argv.len() + 1);
            complete.push(OsString::from(argv0.as_str()));
            complete.extend(argv);
            let parsed = TypedRoundTrip::try_parse_from(complete);
            prop_assert_eq!(parsed, Ok(expected.clone()));
            coverage.borrow_mut().record(&expected, &argv0);
            Ok(())
        });
        if let Err(error) = result {
            panic!("Argx typed round-trip property failed: {error}");
        }

        let coverage = coverage.into_inner();
        eprintln!("[typed fuzz] PASS: {cases} typed round-trip cases");
        eprintln!(
            "[typed fuzz] entry points: args={} | argv0={} | non_ascii_argv0={}",
            coverage.entry_points[0], coverage.entry_points[1], coverage.entry_points[2],
        );
        eprintln!(
            "[typed fuzz] scalars: bool_true={} | bool_false={} | option_i64_some={} | option_i64_none={}",
            coverage.scalars[1], coverage.scalars[0], coverage.scalars[3], coverage.scalars[2],
        );
        eprintln!(
            "[typed fuzz] collections: vec_empty={} | vec_items={} | option_vec_some={} | option_vec_none={} | option_vec_empty_items={} | option_vec_items={} | rest_empty={} | rest_items={}",
            coverage.collections[0],
            coverage.collections[1],
            coverage.collections[3],
            coverage.collections[2],
            coverage.collections[4],
            coverage.collections[5],
            coverage.collections[6],
            coverage.collections[7],
        );
        eprintln!(
            "[typed fuzz] strings: values={} | empty={} | non_ascii={} | unicode_scalars={}",
            coverage.strings[0], coverage.strings[1], coverage.strings[2], coverage.strings[3],
        );
    }

    /// Fuzzes recursive and sibling flattened binding through both parser entry points.
    #[cfg(feature = "derive")]
    #[test]
    fn flattened_binding_round_trips_generated_values() {
        let strategy = flatten_round_trip_strategy();
        let config = proptest_config("flattened_binding_round_trips_generated_values");
        let cases = config.cases;
        let coverage = RefCell::new(FlattenCoverage::default());
        let mut runner = TestRunner::new(config);

        let result = runner.run(&strategy, |expected| {
            let argv = flatten_round_trip_argv(&expected);
            prop_assert_eq!(FlattenRoundTrip::try_parse_args(argv.clone()), Ok(expected.clone()));

            let mut complete = Vec::with_capacity(argv.len() + 1);
            complete.push(OsString::from("argx-flatten"));
            complete.extend(argv);
            prop_assert_eq!(FlattenRoundTrip::try_parse_from(complete), Ok(expected.clone()));
            coverage.borrow_mut().record(&expected);
            Ok(())
        });
        if let Err(error) = result {
            panic!("Argx flattened typed round-trip property failed: {error}");
        }

        let coverage = coverage.into_inner();
        eprintln!("[flatten fuzz] PASS: {cases} recursive/sibling flatten round-trip cases");
        eprintln!(
            "[flatten fuzz] entry points: args={} | argv0={}",
            coverage.entry_points[0], coverage.entry_points[1],
        );
        eprintln!(
            "[flatten fuzz] switches: root_false={} | root_true={} | leaf_false={} | leaf_true={}",
            coverage.switches[0], coverage.switches[1], coverage.switches[2], coverage.switches[3],
        );
        eprintln!(
            "[flatten fuzz] optionals: leaf_number_none={} | leaf_number_some={} | sibling_none={} | sibling_some={}",
            coverage.optionals[0],
            coverage.optionals[1],
            coverage.optionals[2],
            coverage.optionals[3],
        );
        eprintln!(
            "[flatten fuzz] collections: leaf_empty={} | leaf_items={} | nested_none={} | nested_some={} | nested_items={} | rest_empty={} | rest_items={}",
            coverage.collections[0],
            coverage.collections[1],
            coverage.collections[2],
            coverage.collections[3],
            coverage.collections[4],
            coverage.collections[5],
            coverage.collections[6],
        );
        eprintln!(
            "[flatten fuzz] strings: values={} | empty={} | non_ascii={} | unicode_scalars={}",
            coverage.strings[0], coverage.strings[1], coverage.strings[2], coverage.strings[3],
        );
    }

    /// Fuzzes semantic error precedence across independent flattened groups.
    #[cfg(feature = "derive")]
    #[test]
    fn flattened_binding_preserves_error_precedence() {
        let strategy = (scalar_text_strategy(), scalar_text_strategy());
        let config = proptest_config("flattened_binding_preserves_error_precedence");
        let cases = config.cases;
        let coverage = RefCell::new(FlattenErrorCoverage::default());
        let mut runner = TestRunner::new(config);

        let result = runner.run(&strategy, |(first, second)| {
            let first_arg = OsString::from(format!("--port={}", first.value));
            let second_arg = OsString::from(format!("--port={}", second.value));

            prop_assert_eq!(
                FlattenErrors::try_parse_args([first_arg.clone(), second_arg.clone()]),
                Err(TypedError::DuplicateArgument { name: "port" }),
            );
            prop_assert_eq!(
                FlattenErrors::try_parse_args([
                    first_arg.clone(),
                    second_arg,
                    OsString::from("--unknown"),
                ]),
                Err(TypedError::UnknownFlag { token: b"--unknown".to_vec() }),
            );
            prop_assert_eq!(
                FlattenErrors::try_parse_args([first_arg.clone()]),
                Err(TypedError::MissingRequired { name: "required" }),
            );

            let with_required =
                FlattenErrors::try_parse_args([OsString::from("--required=given"), first_arg]);
            match first.value.parse::<u16>() {
                Ok(port) => prop_assert_eq!(
                    with_required,
                    Ok(FlattenErrors {
                        required: FlattenRequired { required: String::from("given") },
                        scalar: FlattenScalar { port: Some(port) },
                    }),
                ),
                Err(_) => match with_required {
                    Err(TypedError::InvalidValue(error)) => {
                        prop_assert_eq!(error.name, "port");
                        prop_assert_eq!(error.value.as_str(), first.value.as_str());
                    }
                    other => {
                        prop_assert!(false, "unexpected flattened conversion result: {other:?}")
                    }
                },
            }

            let mut coverage = coverage.borrow_mut();
            coverage.classes[first.kind.index()] += 1;
            for check in &mut coverage.checks {
                *check += 1;
            }
            Ok(())
        });
        if let Err(error) = result {
            panic!("Argx flattened error-precedence property failed: {error}");
        }

        let coverage = coverage.into_inner();
        eprintln!("[flatten fuzz] PASS: {cases} flattened error-precedence cases");
        eprintln!(
            "[flatten fuzz] first-value classes: valid_u16={} | overflow={} | negative={} | empty={} | non_numeric={}",
            coverage.classes[0],
            coverage.classes[1],
            coverage.classes[2],
            coverage.classes[3],
            coverage.classes[4],
        );
        eprintln!(
            "[flatten fuzz] precedence checks: duplicate_over_required_and_conversion={} | raw_syntax_over_duplicate={} | required_over_conversion={} | child_conversion={}",
            coverage.checks[0], coverage.checks[1], coverage.checks[2], coverage.checks[3],
        );
    }

    /// Fuzzes typed round trips through payload, unit, flattened, and nested subcommands.
    #[cfg(feature = "derive")]
    #[test]
    fn subcommand_binding_round_trips_generated_trees() {
        let strategy = subcommand_round_trip_strategy();
        let config = proptest_config("subcommand_binding_round_trips_generated_trees");
        let cases = config.cases;
        let coverage = RefCell::new(SubcommandCoverage::default());
        let mut runner = TestRunner::new(config);

        let result = runner.run(&strategy, |value| {
            let argv = subcommand_round_trip_argv(&value);
            let expected = expected_subcommand(&value);
            prop_assert_eq!(
                SubcommandRoundTrip::try_parse_args(argv.clone()),
                Ok(expected.clone())
            );

            let mut complete = vec![OsString::from("argx-subcommand-fuzz")];
            complete.extend(argv);
            prop_assert_eq!(SubcommandRoundTrip::try_parse_from(complete), Ok(expected));

            coverage.borrow_mut().record(&value);
            Ok(())
        });
        if let Err(error) = result {
            panic!("Argx subcommand round-trip property failed: {error}");
        }

        let coverage = coverage.into_inner();
        eprintln!("[subcommand fuzz] PASS: {cases} nested command round-trip cases");
        eprintln!(
            "[subcommand fuzz] selections: add={} | config_get={} | config_status={} | status={}",
            coverage.variants[0], coverage.variants[1], coverage.variants[2], coverage.variants[3],
        );
        eprintln!(
            "[subcommand fuzz] switches: root_false={} | root_true={} | add_false={} | add_true={} | config_false={} | config_true={}",
            coverage.switches[0],
            coverage.switches[1],
            coverage.switches[2],
            coverage.switches[3],
            coverage.switches[4],
            coverage.switches[5],
        );
        eprintln!(
            "[subcommand fuzz] collections: tags_empty={} | tag_items={}",
            coverage.collections[0], coverage.collections[1],
        );
        eprintln!(
            "[subcommand fuzz] entry points: args={} | argv0={}",
            coverage.entry_points[0], coverage.entry_points[1],
        );
        eprintln!(
            "[subcommand fuzz] strings: values={} | empty={} | non_ascii={} | unicode_scalars={}",
            coverage.strings[0], coverage.strings[1], coverage.strings[2], coverage.strings[3],
        );
    }

    /// Fuzzes raw and typed error precedence across a selected command boundary.
    #[cfg(feature = "derive")]
    #[test]
    fn subcommand_binding_preserves_error_precedence() {
        let strategy = (scalar_text_strategy(), scalar_text_strategy());
        let config = proptest_config("subcommand_binding_preserves_error_precedence");
        let cases = config.cases;
        let coverage = RefCell::new(SubcommandErrorCoverage::default());
        let mut runner = TestRunner::new(config);

        let result = runner.run(&strategy, |(first, second)| {
            let first_arg = OsString::from(format!("--port={}", first.value));
            let second_arg = OsString::from(format!("--port={}", second.value));

            prop_assert_eq!(
                SubcommandErrors::try_parse_args([
                    OsString::from("--root"),
                    OsString::from("--root"),
                    OsString::from("child"),
                    OsString::from("--unknown"),
                ]),
                Err(TypedError::UnknownFlag { token: b"--unknown".to_vec() }),
            );
            prop_assert_eq!(
                SubcommandErrors::try_parse_args([
                    OsString::from("child"),
                    first_arg.clone(),
                    second_arg,
                    OsString::from("--required=given"),
                ]),
                Err(TypedError::DuplicateArgument { name: "port" }),
            );
            prop_assert_eq!(
                SubcommandErrors::try_parse_args([OsString::from("child"), first_arg]),
                Err(TypedError::MissingRequired { name: "required" }),
            );
            prop_assert_eq!(
                SubcommandErrors::try_parse_args([
                    OsString::from("--root"),
                    OsString::from("--root"),
                ]),
                Err(TypedError::DuplicateArgument { name: "root" }),
            );

            let mut coverage = coverage.borrow_mut();
            coverage.classes[first.kind.index()] += 1;
            for check in &mut coverage.checks {
                *check += 1;
            }
            Ok(())
        });
        if let Err(error) = result {
            panic!("Argx subcommand error-precedence property failed: {error}");
        }

        let coverage = coverage.into_inner();
        eprintln!("[subcommand fuzz] PASS: {cases} command error-precedence cases");
        eprintln!(
            "[subcommand fuzz] first-value classes: valid_u16={} | overflow={} | negative={} | empty={} | non_numeric={}",
            coverage.classes[0],
            coverage.classes[1],
            coverage.classes[2],
            coverage.classes[3],
            coverage.classes[4],
        );
        eprintln!(
            "[subcommand fuzz] precedence checks: child_raw_over_parent_duplicate={} | child_duplicate_over_conversion={} | child_required_over_conversion={} | parent_duplicate_over_missing_command={}",
            coverage.checks[0], coverage.checks[1], coverage.checks[2], coverage.checks[3],
        );
    }

    /// Fuzzes deferred duplicate checking and raw syntax error precedence.
    #[cfg(feature = "derive")]
    #[test]
    fn typed_scalar_errors_follow_binding_precedence() {
        let strategy = (scalar_text_strategy(), scalar_text_strategy());
        let config = proptest_config("typed_scalar_errors_follow_binding_precedence");
        let cases = config.cases;
        let coverage = RefCell::new(TypedScalarCoverage::default());
        let mut runner = TestRunner::new(config);

        let result = runner.run(&strategy, |(first, second)| {
            let first_value = first.value.as_str();
            let second_value = second.value.as_str();
            let first_arg = OsString::from(format!("--port={first_value}"));
            let second_arg = OsString::from(format!("--port={second_value}"));

            let single = TypedScalar::try_parse_args([first_arg.clone()]);
            let parsed = match first_value.parse::<u16>() {
                Ok(port) => {
                    prop_assert_eq!(single, Ok(TypedScalar { port: Some(port) }));
                    true
                }
                Err(_) => {
                    match single {
                        Err(TypedError::InvalidValue(error)) => {
                            prop_assert_eq!(error.name, "port");
                            prop_assert_eq!(error.value.as_str(), first_value);
                            prop_assert!(!error.reason.is_empty());
                        }
                        other => {
                            prop_assert!(false, "unexpected scalar conversion result: {other:?}")
                        }
                    }
                    false
                }
            };

            prop_assert_eq!(
                TypedScalar::try_parse_args([first_arg.clone(), second_arg.clone()]),
                Err(TypedError::DuplicateArgument { name: "port" }),
            );
            prop_assert_eq!(
                TypedScalar::try_parse_args([first_arg, second_arg, OsString::from("--unknown")]),
                Err(TypedError::UnknownFlag { token: b"--unknown".to_vec() }),
            );
            coverage.borrow_mut().record(&first, parsed);
            Ok(())
        });
        if let Err(error) = result {
            panic!("Argx typed error-precedence property failed: {error}");
        }

        let coverage = coverage.into_inner();
        eprintln!("[typed fuzz] PASS: {cases} typed scalar/error-precedence cases");
        eprintln!(
            "[typed fuzz] first-value classes: valid_u16={} | overflow={} | negative={} | empty={} | non_numeric={} | non_ascii={}",
            coverage.classes[0],
            coverage.classes[1],
            coverage.classes[2],
            coverage.classes[3],
            coverage.classes[4],
            coverage.non_ascii_values,
        );
        eprintln!(
            "[typed fuzz] single outcomes: parsed={} | invalid_value={}",
            coverage.outcomes[0], coverage.outcomes[1],
        );
        eprintln!(
            "[typed fuzz] precedence checks: duplicate_over_conversion={} | raw_syntax_over_duplicate={}",
            coverage.precedence[0], coverage.precedence[1],
        );
    }

    /// Fuzzes lossless OS-backed binding and strict UTF-8 rejection on Unix.
    #[cfg(all(feature = "derive", unix))]
    #[test]
    fn typed_binding_preserves_non_utf8_os_values() {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        let strategy = invalid_utf8_strategy();
        let config = proptest_config("typed_binding_preserves_non_utf8_os_values");
        let cases = config.cases;
        let coverage = RefCell::new(TypedOsCoverage::default());
        let mut runner = TestRunner::new(config);
        let result = runner.run(&strategy, |bytes| {
            let raw = OsString::from_vec(bytes.clone());
            let parsed = TypedPath::try_parse_args([OsString::from("--"), raw.clone()]);
            let parsed = parsed.expect("arbitrary Unix OS bytes must bind to PathBuf");
            prop_assert_eq!(parsed.path.as_os_str().as_bytes(), bytes.as_slice());

            prop_assert_eq!(
                TypedText::try_parse_args([OsString::from("--"), raw]),
                Err(TypedError::InvalidUtf8 { name: "value", value: bytes.clone() }),
            );

            let mut attached_path = b"--path=".to_vec();
            attached_path.extend_from_slice(&bytes);
            let parsed = TypedPathFlag::try_parse_args([OsString::from_vec(attached_path)]);
            let parsed = parsed.expect("attached arbitrary Unix OS bytes must bind to PathBuf");
            prop_assert_eq!(parsed.path.as_os_str().as_bytes(), bytes.as_slice());

            let mut attached_text = b"--value=".to_vec();
            attached_text.extend_from_slice(&bytes);
            prop_assert_eq!(
                TypedTextFlag::try_parse_args([OsString::from_vec(attached_text)]),
                Err(TypedError::InvalidUtf8 { name: "value", value: bytes.clone() }),
            );
            coverage.borrow_mut().record(&bytes);
            Ok(())
        });
        if let Err(error) = result {
            panic!("Argx typed non-UTF-8 property failed: {error}");
        }

        let coverage = coverage.into_inner();
        eprintln!("[typed fuzz] PASS: {cases} non-UTF-8 typed binding cases");
        eprintln!(
            "[typed fuzz] generated bytes: total={} | high_bit={} | values_with_nul={}",
            coverage.generated[0], coverage.generated[1], coverage.generated[2],
        );
        eprintln!(
            "[typed fuzz] PathBuf round-trips: positional={} | attached={}",
            coverage.path_round_trips[0], coverage.path_round_trips[1],
        );
        eprintln!(
            "[typed fuzz] String rejections: positional={} | attached={}",
            coverage.text_rejections[0], coverage.text_rejections[1],
        );
    }
}
