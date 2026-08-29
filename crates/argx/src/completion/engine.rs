//! Shell-independent completion engine over the raw argv parser.

use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    io::{self, Write as _},
};

use super::{
    PROTOCOL_COMMAND, PROTOCOL_ENV, PROTOCOL_LINE_ENV, PROTOCOL_VERSION, PROTOCOL_WORDS_ENV,
};
use crate::{
    cli::{
        argv::{
            ArgvParser, Error as ArgvError, Event, accepts_detached_flag_value,
            routes_negative_number_to_arg,
        },
        command::{
            Action, Arg, Command, ConstraintKind, FIELDS_FLAG, Flag, Key, Named, OUTPUT_FLAG,
            SCHEMA_ACTION, long as resolve_long, short as resolve_short,
        },
        protocol::CommandArgs,
    },
    error::display_bytes,
};

/// Handles one private completion request for `T` from the current process.
///
/// `false` means the current invocation is an ordinary CLI request and should continue through the
/// normal parser. A recognized completion invocation is consumed even when malformed so private
/// protocol arguments never leak into the application's public grammar.
pub(crate) fn handle_process<T>() -> bool
where
    T: CommandArgs,
{
    if env::var_os(PROTOCOL_ENV).as_deref() != Some(OsStr::new(PROTOCOL_VERSION)) {
        return false;
    }

    let mut argv = env::args_os().skip(1);
    if argv.next().as_deref() != Some(OsStr::new(PROTOCOL_COMMAND)) {
        return false;
    }
    if argv.next().is_some() {
        return true;
    }

    let candidates = match env::var(PROTOCOL_WORDS_ENV) {
        Ok(encoded) => {
            let Ok(spans) = serde_json::from_str::<Vec<String>>(&encoded) else {
                return true;
            };
            complete_spans_with_schema(T::COMMAND, &spans, T::SCHEMA_ENABLED)
        }
        Err(env::VarError::NotUnicode(_)) => return true,
        Err(env::VarError::NotPresent) => {
            let Some(line) = env::var_os(PROTOCOL_LINE_ENV) else {
                return true;
            };
            let Some(line) = line.to_str() else {
                return true;
            };
            complete_line_with_schema(T::COMMAND, line, T::SCHEMA_ENABLED)
        }
    };

    let rendered = render_candidates(&candidates);
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(rendered.as_bytes());
    let _ = stdout.flush();
    true
}

/// One shell-independent completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate<'a> {
    /// Exact command-line spelling to insert.
    value: String,
    /// Optional human-facing description shown by shells that support it.
    description: Option<&'a str>,
}

/// Parser state reached by completed words before the cursor.
#[derive(Debug)]
struct Position<'t> {
    /// Selected command after walking completed argv words.
    command: &'t Command<'t>,
    /// Selected command ancestors from root to the parent of `command`.
    ancestors: Vec<&'t Command<'t>>,
    /// Positional argument that would receive the next ordinary word.
    next_arg: Option<&'t Arg<'t>>,
    /// Whether `--` has stopped option and subcommand interpretation.
    flags_stopped: bool,
    /// Value-taking option left waiting for the word under the cursor.
    awaiting_value: Option<&'t Flag<'t>>,
    /// Semantic argument keys supplied before the cursor.
    given: HashSet<Key>,
    /// Whether machine-readable schema discovery is enabled for the parser root.
    schema_enabled: bool,
}

impl<'t> Position<'t> {
    /// Returns the selected command path from root through the current command.
    fn command_path(&self) -> impl DoubleEndedIterator<Item = &'t Command<'t>> + '_ {
        self.ancestors.iter().copied().chain(std::iter::once(self.command))
    }
}

/// Completes one shell line already truncated at the cursor.
#[cfg(test)]
fn complete_line<'t>(root: &'t Command<'t>, line: &str) -> Vec<Candidate<'t>> {
    complete_line_with_schema(root, line, false)
}

/// Completes one shell line with optional schema-discovery candidates.
fn complete_line_with_schema<'t>(
    root: &'t Command<'t>,
    line: &str,
    schema_enabled: bool,
) -> Vec<Candidate<'t>> {
    let split = split_line(line);
    complete_split(root, &split, schema_enabled)
}

/// Completes Nushell's already-tokenized external-completer spans.
#[cfg(test)]
fn complete_spans<'t>(root: &'t Command<'t>, spans: &[String]) -> Vec<Candidate<'t>> {
    complete_spans_with_schema(root, spans, false)
}

/// Completes Nushell spans with optional schema-discovery candidates.
fn complete_spans_with_schema<'t>(
    root: &'t Command<'t>,
    spans: &[String],
    schema_enabled: bool,
) -> Vec<Candidate<'t>> {
    let Some((prefix, completed)) = spans.split_last() else {
        return Vec::new();
    };
    let current_index = completed.len();
    let argv = if current_index <= 1 { Vec::new() } else { completed[1..].to_vec() };
    // Nushell uses a whitespace-only final span to represent a fresh word after a separator.
    let prefix =
        if prefix.chars().all(char::is_whitespace) { String::new() } else { prefix.clone() };

    let split = Split { argv, prefix, current_index };
    complete_split(root, &split, schema_enabled)
}

/// Completes one normalized cursor position independent of its shell transport.
fn complete_split<'t>(
    root: &'t Command<'t>,
    split: &Split,
    schema_enabled: bool,
) -> Vec<Candidate<'t>> {
    if split.current_index == 0 {
        return Vec::new();
    }

    if schema_enabled && split.argv.first().is_some_and(|word| word == "schema") {
        return schema_path_candidates(root, &split.argv[1..], &split.prefix);
    }

    let Some(position) = walk(root, &split.argv, schema_enabled) else {
        return Vec::new();
    };

    if let Some(flag) = position.awaiting_value {
        return if flag_available_for_value(&position, flag) {
            detached_value_candidates(flag, &split.prefix)
        } else {
            Vec::new()
        };
    }

    if let Some((flag, value_prefix, insertion_prefix)) =
        attached_long_value(&position, &split.prefix)
            .or_else(|| attached_short_value(&position, &split.prefix))
    {
        return if flag_available_for_value(&position, flag) {
            value_candidates(flag.accepted_values, value_prefix, insertion_prefix)
        } else {
            Vec::new()
        };
    }

    if split.prefix.starts_with('-') && !split.prefix.starts_with("--") && split.prefix.len() > 2 {
        return Vec::new();
    }

    candidates(&position, &split.prefix)
}

/// Completes command-path segments following the built-in `schema` pseudo-command.
fn schema_path_candidates<'t>(
    root: &'t Command<'t>,
    completed: &[String],
    prefix: &str,
) -> Vec<Candidate<'t>> {
    let mut command = root;
    for segment in completed {
        let Some(child) = command.subcommands.iter().copied().find(|child| {
            child.name == segment || child.aliases.iter().any(|alias| *alias == segment)
        }) else {
            return Vec::new();
        };
        command = child;
    }

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for &child in command.subcommands {
        push_candidate(&mut candidates, &mut seen, prefix, child.name, child.about);
    }
    candidates
}

/// Walks complete argv words through the real raw parser.
///
/// A missing detached flag value is usable only when the parser reached the physical end of the
/// completed words. If a later flag-like token caused the error, the completed prefix is already
/// invalid and there is no sound completion position. Any other raw parse error likewise returns
/// `None`.
fn walk<'t>(root: &'t Command<'t>, argv: &[String], schema_enabled: bool) -> Option<Position<'t>> {
    let refs: Vec<&OsStr> = argv.iter().map(|value| OsStr::new(value.as_str())).collect();
    let mut parser = ArgvParser::new_with_schema(root, &refs, schema_enabled);
    let mut given = HashSet::new();
    let mut awaiting_value = None;

    while let Some(event) = parser.next_event() {
        match event {
            Ok(Event::Flag { flag, .. }) => {
                given.insert(flag.key);
            }
            Ok(Event::Arg { arg, .. }) => {
                given.insert(arg.key);
            }
            Ok(Event::Command { .. } | Event::Output { .. } | Event::Fields { .. }) => {}
            Err(ArgvError::MissingFlagValue { flag }) if parser.at_end() => {
                awaiting_value = Some(flag);
                break;
            }
            Ok(Event::Action { .. }) | Err(_) => return None,
        }
    }

    Some(Position {
        command: parser.command(),
        ancestors: parser.ancestors().to_vec(),
        next_arg: parser.next_arg(),
        flags_stopped: parser.flags_stopped(),
        awaiting_value,
        given,
        schema_enabled,
    })
}

/// Resolves one attached long-option value under the cursor.
fn attached_long_value<'a, 't>(
    position: &Position<'t>,
    prefix: &'a str,
) -> Option<(&'t Flag<'t>, &'a str, &'a str)> {
    let body = prefix.strip_prefix("--")?;
    let (name, value_prefix) = body.split_once('=')?;
    let Some(Named::Flag { flag, .. }) =
        resolve_long(position.command, &position.ancestors, name.as_bytes())
    else {
        return None;
    };
    if !flag.takes_value {
        return None;
    }

    let insertion_prefix = &prefix[..prefix.len() - value_prefix.len()];
    Some((flag, value_prefix, insertion_prefix))
}

/// Resolves one attached short-option value under the cursor.
fn attached_short_value<'a, 't>(
    position: &Position<'t>,
    prefix: &'a str,
) -> Option<(&'t Flag<'t>, &'a str, &'a str)> {
    let body = prefix.strip_prefix('-')?;
    if body.starts_with('-') {
        return None;
    }

    let bytes = body.as_bytes();
    let mut index = 0;
    while let Some(&short) = bytes.get(index) {
        match resolve_short(position.command, &position.ancestors, short) {
            Some(Named::Flag { flag, .. }) if flag.takes_value => {
                let tail = index + 1;
                if tail == bytes.len() {
                    return None;
                }
                let value_start = tail + usize::from(bytes[tail] == b'=');
                let value_prefix = &body[value_start..];
                let insertion_prefix = &prefix[..value_start + 1];
                return Some((flag, value_prefix, insertion_prefix));
            }
            Some(Named::Flag { .. }) => index += 1,
            Some(Named::Action(_)) | None => return None,
        }
    }
    None
}

/// Builds available candidates for one parser position and typed prefix.
fn candidates<'t>(position: &Position<'t>, prefix: &str) -> Vec<Candidate<'t>> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if !position.flags_stopped && !prefix.starts_with('-') {
        if position.schema_enabled && position.ancestors.is_empty() {
            push_candidate(
                &mut candidates,
                &mut seen,
                prefix,
                "schema",
                Some("Inspect machine-readable command schema"),
            );
        }
        for &command in position.command.subcommands {
            push_candidate(&mut candidates, &mut seen, prefix, command.name, command.about);
        }
    }

    if let Some(arg) = position.next_arg
        && !conflicts_with_given(position, arg.key)
    {
        push_positional_values(&mut candidates, &mut seen, position, arg, prefix);
    }

    if flags_possible(position, prefix) {
        for flag in [&OUTPUT_FLAG, &FIELDS_FLAG] {
            for &long in flag.longs {
                push_option(&mut candidates, &mut seen, prefix, "--", long, flag.help);
            }
            for &short in flag.shorts {
                let spelling = format!("-{}", char::from(short));
                push_owned_candidate(&mut candidates, &mut seen, prefix, spelling, flag.help);
            }
        }
        if position.schema_enabled {
            for &long in SCHEMA_ACTION.longs {
                push_option(
                    &mut candidates,
                    &mut seen,
                    prefix,
                    "--",
                    long,
                    Some(SCHEMA_ACTION.help),
                );
            }
            for &short in SCHEMA_ACTION.shorts {
                let spelling = format!("-{}", char::from(short));
                push_owned_candidate(
                    &mut candidates,
                    &mut seen,
                    prefix,
                    spelling,
                    Some(SCHEMA_ACTION.help),
                );
            }
        }
        for &action in position.command.actions {
            for &long in action.longs {
                if resolves_action_long(position, action, long) {
                    push_option(&mut candidates, &mut seen, prefix, "--", long, Some(action.help));
                }
            }
            for &short in action.shorts {
                if resolves_action_short(position, action, short) {
                    let spelling = format!("-{}", char::from(short));
                    push_owned_candidate(
                        &mut candidates,
                        &mut seen,
                        prefix,
                        spelling,
                        Some(action.help),
                    );
                }
            }
        }

        for command in position.command_path().rev() {
            for &flag in command.flags {
                if !std::ptr::eq(command, position.command) && !flag.global {
                    continue;
                }
                if (!flag.repeatable && position.given.contains(&flag.key))
                    || conflicts_with_given(position, flag.key)
                {
                    continue;
                }

                for &long in flag.longs {
                    if resolves_flag_long(position, flag, long) {
                        push_option(&mut candidates, &mut seen, prefix, "--", long, flag.help);
                    }
                }
                for &short in flag.shorts {
                    if resolves_flag_short(position, flag, short) {
                        let spelling = format!("-{}", char::from(short));
                        push_owned_candidate(
                            &mut candidates,
                            &mut seen,
                            prefix,
                            spelling,
                            flag.help,
                        );
                    }
                }
            }
        }
    }

    candidates.sort_by(|left, right| left.value.cmp(&right.value));
    candidates
}

/// Reports whether flag spellings remain meaningful for the current prefix.
fn flags_possible(position: &Position<'_>, prefix: &str) -> bool {
    if position.flags_stopped {
        return false;
    }

    let bytes = prefix.as_bytes();
    if !bytes.starts_with(b"-") {
        return prefix.is_empty();
    }

    !routes_negative_number_to_arg(position.command, &position.ancestors, position.next_arg, bytes)
}

/// Reports whether a value-taking option remains valid enough to offer value candidates.
fn flag_available_for_value(position: &Position<'_>, flag: &Flag<'_>) -> bool {
    (flag.repeatable || !position.given.contains(&flag.key))
        && !conflicts_with_given(position, flag.key)
}

/// Reports whether one candidate key conflicts with any argument already supplied on argv.
fn conflicts_with_given(position: &Position<'_>, candidate: Key) -> bool {
    position.command_path().any(|command| {
        command.constraints.iter().any(|constraint| {
            constraint.kind == ConstraintKind::Conflicts
                && ((constraint.source == candidate && position.given.contains(&constraint.target))
                    || (constraint.target == candidate
                        && position.given.contains(&constraint.source)))
        })
    })
}

/// Checks that a canonical long action spelling still resolves to that action after shadowing.
fn resolves_action_long(position: &Position<'_>, action: &Action<'_>, long: &str) -> bool {
    matches!(
        resolve_long(position.command, &position.ancestors, long.as_bytes()),
        Some(Named::Action(found)) if std::ptr::eq(found, action)
    )
}

/// Checks that a canonical short action spelling still resolves to that action after shadowing.
fn resolves_action_short(position: &Position<'_>, action: &Action<'_>, short: u8) -> bool {
    matches!(
        resolve_short(position.command, &position.ancestors, short),
        Some(Named::Action(found)) if std::ptr::eq(found, action)
    )
}

/// Checks that a canonical long flag spelling still resolves to that flag after shadowing.
fn resolves_flag_long(position: &Position<'_>, flag: &Flag<'_>, long: &str) -> bool {
    matches!(
        resolve_long(position.command, &position.ancestors, long.as_bytes()),
        Some(Named::Flag { flag: found, .. }) if std::ptr::eq(found, flag)
    )
}

/// Checks that a canonical short flag spelling still resolves to that flag after shadowing.
fn resolves_flag_short(position: &Position<'_>, flag: &Flag<'_>, short: u8) -> bool {
    matches!(
        resolve_short(position.command, &position.ancestors, short),
        Some(Named::Flag { flag: found, .. }) if std::ptr::eq(found, flag)
    )
}

/// Builds finite detached values that ordinary flag parsing can actually consume.
fn detached_value_candidates<'t>(flag: &'t Flag<'t>, typed: &str) -> Vec<Candidate<'t>> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for &value in flag.accepted_values {
        if accepts_detached_flag_value(flag, value.as_bytes())
            && value.starts_with(typed)
            && protocol_safe_value(value)
        {
            push_owned_candidate(&mut candidates, &mut seen, "", value.to_owned(), None);
        }
    }
    candidates.sort_by(|left, right| left.value.cmp(&right.value));
    candidates
}

/// Adds finite positional values that still route to the positional under raw argv semantics.
fn push_positional_values<'t>(
    candidates: &mut Vec<Candidate<'t>>,
    seen: &mut HashSet<String>,
    position: &Position<'t>,
    arg: &'t Arg<'t>,
    typed: &str,
) {
    for &value in arg.accepted_values {
        if positional_value_reaches_arg(position, arg, value)
            && value.starts_with(typed)
            && protocol_safe_value(value)
        {
            push_owned_candidate(candidates, seen, "", value.to_owned(), None);
        }
    }
}

/// Reports whether one finite spelling would bind to `arg` instead of being parsed structurally.
fn positional_value_reaches_arg(position: &Position<'_>, arg: &Arg<'_>, value: &str) -> bool {
    if position.flags_stopped {
        return true;
    }

    if position.command.subcommands.iter().any(|command| command.aliases.contains(&value)) {
        return false;
    }

    let bytes = value.as_bytes();
    !matches!(bytes, [b'-', rest @ ..] if !rest.is_empty())
        || routes_negative_number_to_arg(position.command, &position.ancestors, Some(arg), bytes)
}

/// Builds finite value candidates using one typed value prefix and insertion prefix.
fn value_candidates<'t>(
    values: &'t [&'t str],
    typed: &str,
    insertion_prefix: &str,
) -> Vec<Candidate<'t>> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    push_values(&mut candidates, &mut seen, values, typed, insertion_prefix);
    candidates.sort_by(|left, right| left.value.cmp(&right.value));
    candidates
}

/// Adds canonical finite values matching the typed prefix.
fn push_values<'t>(
    candidates: &mut Vec<Candidate<'t>>,
    seen: &mut HashSet<String>,
    values: &'t [&'t str],
    typed: &str,
    insertion_prefix: &str,
) {
    for &value in values {
        if value.starts_with(typed) && protocol_safe_value(value) {
            push_owned_candidate(candidates, seen, "", format!("{insertion_prefix}{value}"), None);
        }
    }
}

/// Reports whether one value can be represented without escaping in the line-oriented protocol.
fn protocol_safe_value(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

/// Adds one option spelling built from a fixed prefix and static name.
fn push_option<'t>(
    candidates: &mut Vec<Candidate<'t>>,
    seen: &mut HashSet<String>,
    typed: &str,
    prefix: &str,
    name: &str,
    description: Option<&'t str>,
) {
    let spelling = format!("{prefix}{name}");
    push_owned_candidate(candidates, seen, typed, spelling, description);
}

/// Adds one borrowed candidate when it matches the typed prefix and was not already offered.
fn push_candidate<'t>(
    candidates: &mut Vec<Candidate<'t>>,
    seen: &mut HashSet<String>,
    prefix: &str,
    value: &str,
    description: Option<&'t str>,
) {
    if value.starts_with(prefix) && seen.insert(value.to_owned()) {
        candidates.push(Candidate { value: value.to_owned(), description });
    }
}

/// Adds one owned candidate when it matches the typed prefix and was not already offered.
fn push_owned_candidate<'t>(
    candidates: &mut Vec<Candidate<'t>>,
    seen: &mut HashSet<String>,
    prefix: &str,
    value: String,
    description: Option<&'t str>,
) {
    if value.starts_with(prefix) && seen.insert(value.clone()) {
        candidates.push(Candidate { value, description });
    }
}

/// Parsed shell line through the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Split {
    /// Completed argv words after the executable and before the word being completed.
    argv: Vec<String>,
    /// Unquoted portion of the current word before the cursor.
    prefix: String,
    /// Zero-based word index occupied by the cursor, including the executable word.
    current_index: usize,
}

/// Splits a Bash/Fish/Zsh command line that has already been truncated at the cursor.
///
/// Only shell word boundaries needed to reconstruct argv are interpreted: whitespace, single and
/// double quotes, and backslash escapes. Shell expansions are deliberately left as literal text;
/// Argx candidates never depend on evaluating shell code.
fn split_line(line: &str) -> Split {
    let mut completed = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut chars = line.chars().peekable();

    while let Some(character) = chars.next() {
        match quote {
            Some('\'') => {
                if character == '\'' {
                    quote = None;
                } else {
                    word.push(character);
                }
            }
            Some('"') => {
                if character == '"' {
                    quote = None;
                } else if character == '\\' {
                    match chars.peek().copied() {
                        Some(next) if matches!(next, '"' | '\\' | '$' | '`') => {
                            word.push(next);
                            chars.next();
                        }
                        _ => word.push(character),
                    }
                } else {
                    word.push(character);
                }
            }
            Some(_) => unreachable!("completion splitter only stores supported quote characters"),
            None => {
                if matches!(character, '\'' | '"') {
                    quote = Some(character);
                    started = true;
                } else if character == '\\' {
                    started = true;
                    if let Some(next) = chars.next() {
                        word.push(next);
                    }
                } else if character.is_whitespace() {
                    if started {
                        completed.push(std::mem::take(&mut word));
                        started = false;
                    }
                } else {
                    word.push(character);
                    started = true;
                }
            }
        }
    }

    let prefix = if started { word } else { String::new() };
    let current_index = completed.len();
    let argv = if current_index <= 1 { Vec::new() } else { completed[1..].to_vec() };

    Split { argv, prefix, current_index }
}

/// Serializes candidates as one line per insertion with an optional tab-separated description.
fn render_candidates(candidates: &[Candidate<'_>]) -> String {
    let mut output = String::new();
    for candidate in candidates {
        output.push_str(&candidate.value);
        if let Some(description) = candidate.description {
            let description = single_line(description);
            if !description.is_empty() {
                output.push('\t');
                output.push_str(&description);
            }
        }
        output.push('\n');
    }
    output
}

/// Collapses arbitrary help prose into one protocol-safe display line.
fn single_line(text: &str) -> String {
    let mut output = String::new();
    for word in text.split_whitespace() {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(&display_bytes(word.as_bytes()));
    }
    output
}

#[cfg(test)]
mod tests {
    #![expect(dead_code, reason = "fixtures are exercised through generated command metadata")]

    use super::*;

    #[derive(argx::ValueEnum)]
    enum Mode {
        Fast,
        Slow,
    }

    #[derive(argx::Args)]
    struct GetArgs {
        /// Output file.
        #[argx(long = "path", short = 'p')]
        path: Option<String>,
        /// Repeatable field selection.
        #[argx(long)]
        field: Vec<String>,
        /// Finite output mode.
        #[argx(long, short = 'm', value_enum, conflicts = "raw")]
        mode: Option<Mode>,
        /// Local spelling that shadows the inherited global.
        #[argx(long = "format")]
        local_format: bool,
        /// Value that conflicts with `json`.
        #[argx(long)]
        raw: bool,
        /// JSON output.
        #[argx(long, conflicts = "raw")]
        json: bool,
        /// Negative numeric positional.
        #[argx(allow_negative_numbers)]
        amount: Option<f64>,
    }

    #[derive(argx::Subcommand)]
    enum Command {
        /// Retrieve one object.
        #[argx(alias = "show")]
        Get(GetArgs),
        /// Print service status.
        Status,
    }

    #[derive(argx::Parser)]
    #[argx(name = "tool", version = "1.0.0")]
    struct Cli {
        /// Root-local flag that must not leak into descendants.
        #[argx(long)]
        root_only: bool,
        /// Global verbosity.
        #[argx(long, short, global)]
        verbose: bool,
        /// Numeric short used to protect negative-number precedence.
        #[argx(short = '1', global)]
        numeric: bool,
        /// Global format whose canonical spelling is shadowed below `get`.
        #[argx(long = "format", alias = "fmt", global)]
        global_format: Option<String>,
        #[argx(subcommand)]
        command: Command,
    }

    #[derive(argx::Parser)]
    #[argx(name = "value-tool")]
    struct ValueCli {
        /// Finite positional mode.
        #[argx(value_enum)]
        mode: Option<Mode>,
    }

    fn values(line: &str) -> Vec<String> {
        complete_line(<Cli as CommandArgs>::COMMAND, line)
            .into_iter()
            .map(|candidate| candidate.value)
            .collect()
    }

    fn positional_values(line: &str) -> Vec<String> {
        complete_line(<ValueCli as CommandArgs>::COMMAND, line)
            .into_iter()
            .map(|candidate| candidate.value)
            .collect()
    }

    fn schema_values(line: &str) -> Vec<String> {
        complete_line_with_schema(<Cli as CommandArgs>::COMMAND, line, true)
            .into_iter()
            .map(|candidate| candidate.value)
            .collect()
    }

    #[test]
    fn split_reconstructs_quoted_completed_words_and_current_prefix() {
        assert_eq!(
            split_line(r#"tool get --output "two words" --j"#),
            Split {
                argv: vec!["get".into(), "--output".into(), "two words".into()],
                prefix: "--j".into(),
                current_index: 4,
            },
        );
        assert_eq!(
            split_line("tool get "),
            Split { argv: vec!["get".into()], prefix: String::new(), current_index: 2 },
        );
        assert_eq!(
            split_line(r"tool get two\ words --"),
            Split {
                argv: vec!["get".into(), "two words".into()],
                prefix: "--".into(),
                current_index: 3,
            },
        );
    }

    #[test]
    fn split_handles_single_quotes_escaped_double_quotes_and_empty_words() {
        assert_eq!(
            split_line(r#"tool get 'two words' --j"#),
            Split {
                argv: vec!["get".into(), "two words".into()],
                prefix: "--j".into(),
                current_index: 3,
            },
        );
        assert_eq!(
            split_line(r#"tool get "two \"words\"" --j"#),
            Split {
                argv: vec!["get".into(), "two \"words\"".into()],
                prefix: "--j".into(),
                current_index: 3,
            },
        );
        assert_eq!(
            split_line(r#"tool get "" --j"#),
            Split {
                argv: vec!["get".into(), String::new()],
                prefix: "--j".into(),
                current_index: 3,
            },
        );
        assert_eq!(
            split_line(r#"tool get "--j"#),
            Split { argv: vec!["get".into()], prefix: "--j".into(), current_index: 2 },
        );
    }

    #[test]
    fn nushell_spans_map_directly_to_completed_argv_and_prefix() {
        let root = <Cli as CommandArgs>::COMMAND;
        let spans = vec!["tool".into(), "get".into(), "--j".into()];
        let candidates = complete_spans(root, &spans)
            .into_iter()
            .map(|candidate| candidate.value)
            .collect::<Vec<_>>();
        assert_eq!(candidates, vec!["--json"]);

        let fresh_word = vec!["tool".into(), "get".into(), " ".into()];
        let candidates = complete_spans(root, &fresh_word)
            .into_iter()
            .map(|candidate| candidate.value)
            .collect::<Vec<_>>();
        assert!(candidates.contains(&"--json".into()));
        assert!(candidates.contains(&"--help".into()));
    }

    #[test]
    fn schema_completion_offers_short_and_long_actions() {
        let candidates = schema_values("tool -");
        assert!(candidates.contains(&"-S".into()));
        assert!(candidates.contains(&"--schema".into()));
    }

    #[test]
    fn root_completion_offers_canonical_commands_and_options_only() {
        let candidates = values("tool ");
        assert!(candidates.contains(&"get".into()));
        assert!(candidates.contains(&"status".into()));
        assert!(candidates.contains(&"--verbose".into()));
        assert!(candidates.contains(&"-v".into()));
        assert!(candidates.contains(&"--help".into()));
        assert!(candidates.contains(&"--version".into()));
        assert!(!candidates.contains(&"show".into()));
        assert!(!candidates.contains(&"--fmt".into()));
    }

    #[test]
    fn aliases_are_accepted_while_reconstructing_scope() {
        let candidates = values("tool show --j");
        assert_eq!(candidates, vec!["--json"]);
    }

    #[test]
    fn descendants_offer_globals_but_not_ancestor_local_flags() {
        let candidates = values("tool get --");
        assert!(candidates.contains(&"--verbose".into()));
        assert!(!candidates.contains(&"--root-only".into()));
    }

    #[test]
    fn used_scalar_options_are_suppressed_but_repeatable_options_remain() {
        let candidates = values("tool get --path out --field id --");
        assert!(!candidates.contains(&"--path".into()));
        assert!(candidates.contains(&"--field".into()));
    }

    #[test]
    fn conflicting_options_are_suppressed() {
        let candidates = values("tool get --raw --");
        assert!(!candidates.contains(&"--json".into()));
        assert!(!candidates.contains(&"--raw".into()));
        assert!(candidates.contains(&"--field".into()));
    }

    #[test]
    fn conflict_suppression_is_symmetric() {
        let candidates = values("tool get --json --");
        assert!(!candidates.contains(&"--raw".into()));
    }

    #[test]
    fn local_spellings_shadow_inherited_globals() {
        let candidates = values("tool get --form");
        assert_eq!(candidates, vec!["--format"]);
    }

    #[test]
    fn arbitrary_values_do_not_offer_structural_candidates() {
        assert!(values("tool get --path ").is_empty());
        assert!(values("tool get --path=pa").is_empty());
        assert!(values("tool get -ppa").is_empty());
    }

    #[test]
    fn finite_flag_values_complete_detached_and_attached_forms() {
        assert_eq!(values("tool get --mode "), vec!["fast", "slow"]);
        assert_eq!(values("tool get --mode f"), vec!["fast"]);
        assert_eq!(values("tool get -m s"), vec!["slow"]);
        assert_eq!(values("tool get --mode=f"), vec!["--mode=fast"]);
        assert_eq!(values("tool get -mf"), vec!["-mfast"]);
        assert_eq!(values("tool get -m="), vec!["-m=fast", "-m=slow"]);
        assert_eq!(values("tool get -vmf"), vec!["-vmfast"]);
        assert!(values("tool get -hmf").is_empty());
    }

    #[test]
    fn finite_values_respect_nonrepeatable_and_conflict_state() {
        assert!(values("tool get --mode fast --mode=").is_empty());
        assert!(values("tool get --raw --mode=").is_empty());
    }

    #[test]
    fn detached_finite_values_respect_flag_hyphen_policy() {
        let strict = Flag { accepted_values: &["-fast", "slow"], ..Flag::VALUE };
        assert_eq!(
            detached_value_candidates(&strict, "")
                .into_iter()
                .map(|candidate| candidate.value)
                .collect::<Vec<_>>(),
            vec!["slow"],
        );
        assert_eq!(
            value_candidates(strict.accepted_values, "-", "--mode="),
            vec![Candidate { value: "--mode=-fast".into(), description: None }]
        );

        let loose = Flag { allow_hyphen_values: true, ..strict };
        assert_eq!(
            detached_value_candidates(&loose, "")
                .into_iter()
                .map(|candidate| candidate.value)
                .collect::<Vec<_>>(),
            vec!["-fast", "slow"],
        );
    }

    #[test]
    fn positional_finite_values_follow_raw_flag_and_alias_routing() {
        use crate::cli::command::Command as RuntimeCommand;

        static ARG: Arg<'static> = Arg {
            name: "mode",
            required: false,
            accepted_values: &["-fast", "show", "slow"],
            ..Arg::REQUIRED
        };
        static CHILD: RuntimeCommand<'static> =
            RuntimeCommand { name: "get", aliases: &["show"], ..RuntimeCommand::EMPTY };
        static ROOT: RuntimeCommand<'static> = RuntimeCommand {
            name: "tool",
            args: &[&ARG],
            subcommands: &[&CHILD],
            ..RuntimeCommand::EMPTY
        };
        let position = Position {
            command: &ROOT,
            ancestors: Vec::new(),
            next_arg: Some(&ARG),
            flags_stopped: false,
            awaiting_value: None,
            given: HashSet::new(),
            schema_enabled: false,
        };
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        push_positional_values(&mut candidates, &mut seen, &position, &ARG, "");
        assert_eq!(
            candidates.into_iter().map(|candidate| candidate.value).collect::<Vec<_>>(),
            vec!["slow"],
        );

        let stopped = Position { flags_stopped: true, ..position };
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        push_positional_values(&mut candidates, &mut seen, &stopped, &ARG, "-");
        assert_eq!(
            candidates.into_iter().map(|candidate| candidate.value).collect::<Vec<_>>(),
            vec!["-fast"],
        );
    }

    #[test]
    fn finite_positional_values_complete_before_and_after_separator() {
        let candidates = positional_values("value-tool ");
        assert!(candidates.contains(&"fast".into()));
        assert!(candidates.contains(&"slow".into()));
        assert!(candidates.contains(&"--help".into()));
        assert!(candidates.contains(&"-h".into()));

        assert_eq!(positional_values("value-tool s"), vec!["slow"]);
        assert_eq!(positional_values("value-tool -- s"), vec!["slow"]);
    }

    #[test]
    fn separator_stops_option_and_subcommand_completion() {
        assert!(values("tool get -- ").is_empty());
    }

    #[test]
    fn negative_numbers_route_to_the_positional_instead_of_flags() {
        assert!(values("tool get -2").is_empty());
    }

    #[test]
    fn declared_numeric_shorts_take_precedence_over_negative_number_routing() {
        assert_eq!(values("tool get -1"), vec!["-1"]);
    }

    #[test]
    fn prefixes_filter_structural_candidates() {
        assert_eq!(values("tool st"), vec!["status"]);
        assert_eq!(values("tool --ver"), vec!["--verbose", "--version"]);
    }

    #[test]
    fn protocol_rendering_flattens_and_escapes_descriptions() {
        let rendered = render_candidates(&[Candidate {
            value: "--json".into(),
            description: Some("JSON\noutput\tmode\u{1b}[31m"),
        }]);
        assert_eq!(rendered, "--json\tJSON output mode\\u{1b}[31m\n");
        assert!(!rendered.contains('\u{1b}'));
    }

    #[test]
    fn protocol_unsafe_finite_values_are_not_suggested() {
        let candidates = value_candidates(&["safe", "bad\nvalue", "bad\tvalue"], "", "");
        assert_eq!(
            candidates.into_iter().map(|candidate| candidate.value).collect::<Vec<_>>(),
            vec!["safe"],
        );
    }
}
