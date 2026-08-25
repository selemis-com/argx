//! Deterministic help rendering from static command metadata.

use std::fmt::Write as _;

use crate::__private::{Action, Arg, Command, Flag};

/// Renders short help for one selected command path.
pub(crate) fn render(path: &[&Command<'_>]) -> String {
    let Some(&command) = path.last() else {
        return String::new();
    };

    let mut output = String::new();
    if let Some(about) = command.about.filter(|about| !about.is_empty()) {
        output.push_str(about);
        output.push_str("\n\n");
    }

    output.push_str("Usage:");
    for command in path {
        output.push(' ');
        output.push_str(command.name);
    }
    output.push_str(" [OPTIONS]");
    for flag in command.flags.iter().copied().filter(|flag| flag.required) {
        output.push(' ');
        output.push_str(&required_flag_usage(flag));
    }
    for arg in command.args {
        output.push(' ');
        output.push_str(&arg_usage(arg));
    }
    if !command.subcommands.is_empty() {
        output.push_str(" <COMMAND>");
    }
    output.push('\n');

    if !command.args.is_empty() {
        output.push_str("\nArguments:\n");
        let rows = command
            .args
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
    let mut rows =
        command.flags.iter().map(|flag| (flag_label(flag), flag_help(flag))).collect::<Vec<_>>();
    rows.extend(
        command.actions.iter().map(|action| (action_label(action), action.help.to_owned())),
    );
    write_rows(&mut output, &rows);

    output
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
fn flag_label(flag: &Flag<'_>) -> String {
    spellings_label(flag.shorts, flag.longs, flag.takes_value.then_some(flag.name))
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
    use crate::__private::{Arg, Command, Flag};

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
}
