//! Deterministic help rendering from static command metadata.

use std::fmt::Write as _;

use crate::__private::{
    Action, Arg, Command, Flag, HelpGroup, Key, Named, resolve_long, resolve_short,
};

/// Renderable argument rows collected under one help section.
type HelpRows = Vec<(String, String)>;

/// Help sections paired with their rendered argument rows.
type GroupedHelp<'a> = Vec<(&'a str, HelpRows)>;

/// One flag as visible from the selected command scope.
struct VisibleFlag<'a> {
    /// Command-path scope where this visible occurrence is mounted.
    scope: usize,
    /// Original declaration metadata used for help text and value behavior.
    flag: &'a Flag<'a>,
    /// Long spellings that remain visible after lexical shadowing.
    longs: Vec<&'a str>,
    /// Short spellings that remain visible after lexical shadowing.
    shorts: Vec<u8>,
}

/// Renders short help for one selected command path.
pub(crate) fn render(path: &[&Command<'_>]) -> String {
    let Some(&command) = path.last() else {
        return String::new();
    };

    let visible_flags = visible_flags(path);
    let (grouped_keys, grouped_rows) = grouped_rows(path, &visible_flags);

    let mut output = String::new();
    if let Some(description) =
        command.description.or(command.about).filter(|description| !description.is_empty())
    {
        output.push_str(description);
        output.push_str("\n\n");
    }

    output.push_str("Usage:");
    for (index, command) in path.iter().enumerate() {
        output.push(' ');
        output.push_str(command.name);
        if index + 1 == path.len() {
            output.push_str(" [OPTIONS]");
        }
        for flag in command.flags.iter().filter(|flag| flag.required) {
            output.push(' ');
            output.push_str(&required_flag_usage(flag));
        }
        if index + 1 != path.len() {
            for arg in command.args.iter().filter(|arg| arg.required) {
                output.push(' ');
                output.push_str(&arg_usage(arg));
            }
        }
    }
    for arg in command.args {
        output.push(' ');
        output.push_str(&arg_usage(arg));
    }
    if !command.subcommands.is_empty() {
        output.push_str(" <COMMAND>");
    }
    output.push('\n');

    let ungrouped_args = command
        .args
        .iter()
        .copied()
        .filter(|arg| !grouped_keys.contains(&arg.key))
        .collect::<Vec<_>>();
    if !ungrouped_args.is_empty() {
        output.push_str("\nArguments:\n");
        let rows = ungrouped_args
            .iter()
            .map(|arg| (arg_usage(arg), arg.help.unwrap_or("").to_owned()))
            .collect::<Vec<_>>();
        write_rows(&mut output, &rows);
    }

    if !command.subcommands.is_empty() {
        output.push_str("\nCommands:\n");
        let rows = command
            .subcommands
            .iter()
            .map(|command| (command.name.to_owned(), command.about.unwrap_or("").to_owned()))
            .collect::<Vec<_>>();
        write_rows(&mut output, &rows);
    }

    output.push_str("\nOptions:\n");
    let mut rows = visible_flags
        .iter()
        .filter(|flag| !grouped_keys.contains(&flag.flag.key))
        .map(|flag| (flag_label(flag), flag_help(flag.flag)))
        .collect::<Vec<_>>();
    rows.extend(
        command.actions.iter().map(|action| (action_label(action), action.help.to_owned())),
    );
    write_rows(&mut output, &rows);

    for (heading, rows) in grouped_rows {
        output.push('\n');
        output.push_str(heading);
        output.push_str(":\n");
        write_rows(&mut output, &rows);
    }

    for section in command.help_sections {
        output.push('\n');
        output.push_str(section.heading);
        output.push_str(":\n");
        if !section.body.is_empty() {
            output.push_str(section.body);
            output.push('\n');
        }
    }

    output
}

/// Builds documented flattened-group rows and the semantic keys claimed by those groups.
fn grouped_rows<'a>(
    path: &[&'a Command<'a>],
    visible_flags: &[VisibleFlag<'a>],
) -> (Vec<Key>, GroupedHelp<'a>) {
    let Some(&selected) = path.last() else {
        return (Vec::new(), Vec::new());
    };
    let selected_scope = path.len() - 1;

    let mut grouped_keys = Vec::new();
    let mut sections = GroupedHelp::new();
    for (scope, command) in path.iter().enumerate().rev() {
        for group in command.help_groups.iter().copied() {
            if group.heading.is_empty() {
                continue;
            }
            let heading = group.heading;
            let mut rows = Vec::new();
            if scope == selected_scope {
                for arg in selected.args {
                    if group_contains_arg(group, arg) && !grouped_keys.contains(&arg.key) {
                        grouped_keys.push(arg.key);
                        rows.push((arg_usage(arg), arg.help.unwrap_or("").to_owned()));
                    }
                }
            }
            for flag in visible_flags {
                if flag.scope == scope
                    && group_contains_flag(group, flag.flag)
                    && !grouped_keys.contains(&flag.flag.key)
                {
                    grouped_keys.push(flag.flag.key);
                    rows.push((flag_label(flag), flag_help(flag.flag)));
                }
            }
            if rows.is_empty() {
                continue;
            }
            if let Some((_, existing)) =
                sections.iter_mut().find(|(existing, _)| *existing == heading)
            {
                existing.extend(rows);
            } else {
                sections.push((heading, rows));
            }
        }
    }

    (grouped_keys, sections)
}

/// Reports whether one help group contains a named argument.
fn group_contains_flag(group: &HelpGroup<'_>, flag: &Flag<'_>) -> bool {
    group.flags.iter().any(|candidate| std::ptr::eq(*candidate, flag))
}

/// Reports whether one help group contains a positional argument.
fn group_contains_arg(group: &HelpGroup<'_>, arg: &Arg<'_>) -> bool {
    group.args.iter().any(|candidate| std::ptr::eq(*candidate, arg))
}

/// Resolves flags visible from the selected command using the parser's lexical scope rules.
///
/// The resolver retains the command-path scope of each match so repeated mounts of one reusable
/// `Args` declaration remain distinguishable even though they share static metadata pointers.
fn visible_flags<'a>(path: &[&'a Command<'a>]) -> Vec<VisibleFlag<'a>> {
    let Some((&command, ancestors)) = path.split_last() else {
        return Vec::new();
    };
    let current = ancestors.len();

    let candidates = command.flags.iter().copied().map(|flag| (current, flag)).chain(
        ancestors.iter().enumerate().rev().flat_map(|(scope, command)| {
            command.flags.iter().copied().filter(|flag| flag.global).map(move |flag| (scope, flag))
        }),
    );

    candidates
        .filter_map(|(scope, flag)| {
            let longs = flag
                .longs
                .iter()
                .copied()
                .filter(|long| {
                    matches!(
                        resolve_long(command, ancestors, long.as_bytes()),
                        Some(Named::Flag { flag: resolved, scope: resolved_scope })
                            if resolved_scope == scope && std::ptr::eq(resolved, flag)
                    )
                })
                .collect::<Vec<_>>();
            let shorts = flag
                .shorts
                .iter()
                .copied()
                .filter(|short| {
                    matches!(
                        resolve_short(command, ancestors, *short),
                        Some(Named::Flag { flag: resolved, scope: resolved_scope })
                            if resolved_scope == scope && std::ptr::eq(resolved, flag)
                    )
                })
                .collect::<Vec<_>>();

            (!longs.is_empty() || !shorts.is_empty()).then_some(VisibleFlag {
                scope,
                flag,
                longs,
                shorts,
            })
        })
        .collect()
}

/// Writes aligned help rows without terminal-width-dependent wrapping.
fn write_rows(output: &mut String, rows: &[(String, String)]) {
    let width = rows.iter().map(|(label, _)| label.chars().count()).max().unwrap_or(0);
    for (label, help) in rows {
        if help.is_empty() {
            let _ = writeln!(output, "  {label}");
        } else {
            let _ = writeln!(output, "  {label:<width$}  {help}");
        }
    }
}

/// Renders one named flag in an options table.
fn flag_label(flag: &VisibleFlag<'_>) -> String {
    spellings_label(&flag.shorts, &flag.longs, flag.flag.takes_value.then_some(flag.flag.name))
}

/// Renders help text plus the explicit environment fallback, when present.
fn flag_help(flag: &Flag<'_>) -> String {
    let mut help = flag.help.unwrap_or("").to_owned();
    if let Some(env) = flag.env {
        if !help.is_empty() {
            help.push(' ');
        }
        help.push_str("[env: ");
        help.push_str(env);
        if flag.required_if_env_unset {
            help.push_str("; required if unset");
        }
        help.push(']');
    }
    help
}

/// Renders one built-in action in an options table.
fn action_label(action: &Action<'_>) -> String {
    spellings_label(action.shorts, action.longs, None)
}

/// Renders short and long spellings with an optional value placeholder.
fn spellings_label(shorts: &[u8], longs: &[&str], value_name: Option<&str>) -> String {
    let mut label = String::new();
    for (index, short) in shorts.iter().enumerate() {
        if index > 0 {
            label.push_str(", ");
        }
        label.push('-');
        label.push(char::from(*short));
    }
    for long in longs {
        if !label.is_empty() {
            label.push_str(", ");
        }
        label.push_str("--");
        label.push_str(long);
    }
    if let Some(name) = value_name {
        label.push_str(" <");
        label.push_str(&metavar(name));
        label.push('>');
    }
    label
}

/// Renders the canonical spelling of a required named flag for the usage line.
fn required_flag_usage(flag: &Flag<'_>) -> String {
    let mut usage = flag.longs.first().map_or_else(
        || {
            flag.shorts.first().map_or_else(
                || flag.name.to_owned(),
                |short| {
                    let short = char::from(*short);
                    format!("-{short}")
                },
            )
        },
        |long| format!("--{long}"),
    );
    if flag.takes_value {
        usage.push_str(" <");
        usage.push_str(&metavar(flag.name));
        usage.push('>');
    }
    usage
}

/// Renders one positional argument for usage and argument tables.
fn arg_usage(arg: &Arg<'_>) -> String {
    let name = metavar(arg.name);
    let mut usage = if arg.required { format!("<{name}>") } else { format!("[{name}]") };
    if arg.variadic {
        usage.push_str("...");
    }
    usage
}

/// Converts a canonical field name into a conventional value placeholder.
fn metavar(name: &str) -> String {
    name.replace('-', "_").to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::__private::{Action, ActionKind, Arg, Command, Flag};

    static VERBOSE: Flag<'static> = Flag {
        key: 1,
        name: "verbose",
        help: Some("Enable verbose output"),
        longs: &["verbose"],
        shorts: b"v",
        ..Flag::BOOL
    };
    static OUTPUT: Flag<'static> = Flag {
        key: 2,
        name: "output",
        help: Some("Write to this path"),
        longs: &["output"],
        required: true,
        ..Flag::VALUE
    };
    static PROFILE: Flag<'static> = Flag {
        key: 6,
        name: "profile",
        help: Some("Select a profile"),
        longs: &["profile"],
        env: Some("TOOL_PROFILE"),
        ..Flag::VALUE
    };
    static INPUT: Arg<'static> =
        Arg { key: 3, name: "input", help: Some("Input file"), ..Arg::REQUIRED };
    static REST: Arg<'static> = Arg {
        key: 4,
        name: "rest",
        help: None,
        required: false,
        variadic: true,
        allow_negative_numbers: false,
    };
    static GET: Command<'static> =
        Command { name: "get", about: Some("Read one value"), ..Command::EMPTY };
    static CONFIG: Command<'static> = Command {
        name: "config",
        about: Some("Manage configuration"),
        flags: &[&VERBOSE, &OUTPUT, &PROFILE],
        args: &[&INPUT, &REST],
        subcommands: &[&GET],
        key: 5,
        ..Command::EMPTY
    };
    static ROOT: Command<'static> = Command {
        name: "tool",
        about: Some("Example tool"),
        subcommands: &[&CONFIG],
        ..Command::EMPTY
    };

    #[test]
    fn renders_scope_aware_aligned_help() {
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            render(&[&ROOT, &CONFIG]),
            snapbox::str![[r#"
Manage configuration

Usage: tool config [OPTIONS] --output <OUTPUT> <INPUT> [REST]... <COMMAND>

Arguments:
  <INPUT>    Input file
  [REST]...

Commands:
  get  Read one value

Options:
  -v, --verbose        Enable verbose output
  --output <OUTPUT>    Write to this path
  --profile <PROFILE>  Select a profile [env: TOOL_PROFILE]
  -h, --help           Print help

"#]],
        );
    }

    #[test]
    fn descendant_help_includes_visible_globals_with_parser_shadowing() {
        static ROOT_SCOPE: Flag<'static> = Flag {
            key: 10,
            name: "root-scope",
            help: Some("Root scope"),
            longs: &["scope", "root-scope"],
            shorts: b"s",
            global: true,
            ..Flag::BOOL
        };
        static ROOT_PROFILE: Flag<'static> = Flag {
            key: 11,
            name: "profile",
            help: Some("Required profile"),
            longs: &["profile"],
            shorts: b"p",
            global: true,
            required: true,
            ..Flag::VALUE
        };
        static ROOT_VERSION: Flag<'static> = Flag {
            key: 12,
            name: "root-version",
            help: Some("Root version selector"),
            longs: &["version", "root-version"],
            global: true,
            ..Flag::BOOL
        };
        static MID_SCOPE: Flag<'static> = Flag {
            key: 13,
            name: "mid-scope",
            help: Some("Mid scope"),
            longs: &["scope", "mid-scope"],
            shorts: b"m",
            global: true,
            ..Flag::BOOL
        };
        static LOCAL_SCOPE: Flag<'static> = Flag {
            key: 14,
            name: "scope",
            help: Some("Leaf scope"),
            longs: &["scope"],
            shorts: b"l",
            ..Flag::BOOL
        };
        static VERSION: Action<'static> = Action {
            name: "version",
            diagnostic: "--version",
            help: "Print version",
            longs: &["version"],
            shorts: b"V",
            kind: ActionKind::Version { short: "1", long: "1" },
        };
        static LEAF: Command<'static> = Command {
            name: "leaf",
            actions: &[&crate::__private::HELP_ACTION, &VERSION],
            flags: &[&LOCAL_SCOPE],
            ..Command::EMPTY
        };
        static MID: Command<'static> =
            Command { name: "mid", flags: &[&MID_SCOPE], subcommands: &[&LEAF], ..Command::EMPTY };
        static GLOBAL_ROOT: Command<'static> = Command {
            name: "tool",
            flags: &[&ROOT_SCOPE, &ROOT_PROFILE, &ROOT_VERSION],
            subcommands: &[&MID],
            ..Command::EMPTY
        };

        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            render(&[&GLOBAL_ROOT, &MID, &LEAF]),
            snapbox::str![[r#"
Usage: tool --profile <PROFILE> mid leaf [OPTIONS]

Options:
  -l, --scope              Leaf scope
  -m, --mid-scope          Mid scope
  -s, --root-scope         Root scope
  -p, --profile <PROFILE>  Required profile
  --root-version           Root version selector
  -h, --help               Print help
  -V, --version            Print version

"#]],
        );
    }

    #[test]
    fn descendant_usage_keeps_required_ancestor_flags_at_their_declaring_scope() {
        static ROOT_TOKEN: Flag<'static> = Flag {
            key: 20,
            name: "root-token",
            help: Some("Root token"),
            longs: &["token"],
            global: true,
            required: true,
            ..Flag::VALUE
        };
        static ROOT_CONFIG: Flag<'static> = Flag {
            key: 21,
            name: "config",
            help: Some("Root config"),
            longs: &["config"],
            required: true,
            ..Flag::VALUE
        };
        static LOCAL_TOKEN: Flag<'static> = Flag {
            key: 22,
            name: "token",
            help: Some("Leaf token"),
            longs: &["token"],
            ..Flag::VALUE
        };
        static LEAF: Command<'static> =
            Command { name: "leaf", flags: &[&LOCAL_TOKEN], ..Command::EMPTY };
        static ROOT: Command<'static> = Command {
            name: "tool",
            flags: &[&ROOT_TOKEN, &ROOT_CONFIG],
            subcommands: &[&LEAF],
            ..Command::EMPTY
        };

        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            render(&[&ROOT, &LEAF]),
            snapbox::str![[r#"
Usage: tool --token <ROOT_TOKEN> --config <CONFIG> leaf [OPTIONS]

Options:
  --token <TOKEN>  Leaf token
  -h, --help       Print help

"#]],
        );
    }

    #[test]
    fn reused_global_mount_is_listed_only_for_the_nearest_scope() {
        static SHARED: Flag<'static> = Flag {
            key: 30,
            name: "shared",
            help: Some("Shared setting"),
            longs: &["shared"],
            global: true,
            ..Flag::VALUE
        };
        static LEAF: Command<'static> =
            Command { name: "leaf", flags: &[&SHARED], ..Command::EMPTY };
        static ROOT: Command<'static> =
            Command { name: "tool", flags: &[&SHARED], subcommands: &[&LEAF], ..Command::EMPTY };

        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            render(&[&ROOT, &LEAF]),
            snapbox::str![[r#"
Usage: tool leaf [OPTIONS]

Options:
  --shared <SHARED>  Shared setting
  -h, --help         Print help

"#]],
        );
    }
}
