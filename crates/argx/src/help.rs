//! Deterministic help rendering from static command metadata.

use std::fmt::Write as _;

use crate::__private::{Arg, Command, Flag};

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
            .map(|arg| (arg_usage(arg), arg.help.unwrap_or("")))
            .collect::<Vec<_>>();
        write_rows(&mut output, &rows);
    }

    if !command.subcommands.is_empty() {
        output.push_str("\nCommands:\n");
        let rows = command
            .subcommands
            .iter()
            .map(|command| (command.name.to_owned(), command.about.unwrap_or("")))
            .collect::<Vec<_>>();
        write_rows(&mut output, &rows);
    }

    output.push_str("\nOptions:\n");
    let mut rows = command
        .flags
        .iter()
        .map(|flag| (flag_label(flag), flag.help.unwrap_or("")))
        .collect::<Vec<_>>();
    rows.push(("-h, --help".to_owned(), "Print help"));
    write_rows(&mut output, &rows);

    output
}

/// Writes aligned help rows without terminal-width-dependent wrapping.
fn write_rows(output: &mut String, rows: &[(String, &str)]) {
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
    let mut label = String::new();
    for (index, short) in flag.shorts.iter().enumerate() {
        if index > 0 {
            label.push_str(", ");
        }
        label.push('-');
        label.push(char::from(*short));
    }
    for long in flag.longs {
        if !label.is_empty() {
            label.push_str(", ");
        }
        label.push_str("--");
        label.push_str(long);
    }
    if flag.takes_value {
        label.push_str(" <");
        label.push_str(&metavar(flag.name));
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
        flags: &[&VERBOSE, &OUTPUT],
        args: &[&INPUT, &REST],
        subcommands: &[&GET],
        key: 5,
    };
    static ROOT: Command<'static> = Command {
        name: "tool",
        about: Some("Example tool"),
        subcommands: &[&CONFIG],
        ..Command::EMPTY
    };

    #[test]
    fn renders_scope_aware_aligned_help() {
        assert_eq!(
            render(&[&ROOT, &CONFIG]),
            concat!(
                "Manage configuration\n\n",
                "Usage: tool config [OPTIONS] --output <OUTPUT> <INPUT> [REST]... <COMMAND>\n",
                "\nArguments:\n",
                "  <INPUT>    Input file\n",
                "  [REST]...\n",
                "\nCommands:\n",
                "  get  Read one value\n",
                "\nOptions:\n",
                "  -v, --verbose      Enable verbose output\n",
                "  --output <OUTPUT>  Write to this path\n",
                "  -h, --help         Print help\n",
            )
        );
    }
}
