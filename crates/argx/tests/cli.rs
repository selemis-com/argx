//! Executable CLI contract tests against public examples.

#[path = "support/command.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::support;

    #[test]
    fn basic_example_builds_and_runs() {
        support::example_command("basic").assert().success().stdout_eq("").stderr_eq("");
    }

    #[test]
    fn parse_entry_point_reports_errors_and_exits_unsuccessfully() {
        support::example_command("basic")
            .arg("--unknown")
            .assert()
            .failure()
            .stdout_eq("")
            .stderr_eq(snapbox::str![[r#"
error: unknown flag `--unknown`

For more information, try '--help'.

"#]]);
    }

    #[test]
    fn parse_entry_point_prints_help_and_exits_successfully() {
        support::example_command("basic")
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Minimal command with no declared arguments.

Usage: cli [OPTIONS]

Options:
  -h, --help  Print help

"#]])
            .stderr_eq("");
    }

    #[test]
    fn nested_help_is_scoped_to_the_selected_command() {
        support::example_command("subcommands")
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Root command.

Usage: cli [OPTIONS] <COMMAND>

Commands:
  add     Adds one value.
  remove  Removes one value using the same argument shape.
  status  Shows status without additional arguments.

Options:
  -h, --help  Print help

"#]])
            .stderr_eq("");
        support::example_command("subcommands")
            .args(["add", "--help"])
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Adds one value.

Usage: cli add [OPTIONS] <VALUE>

Arguments:
  <VALUE>  Value to add.

Options:
  --force     Forces the operation.
  -h, --help  Print help

"#]])
            .stderr_eq("");
    }
}
