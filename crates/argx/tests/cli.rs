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
    fn diagnostics_use_cli_spellings_and_environment_context() {
        support::example_command("defaults")
            .arg("--port")
            .assert()
            .failure()
            .stdout_eq("")
            .stderr_eq(snapbox::str![[r#"
error: missing value for `--port`

For more information, try '--help'.

"#]]);

        support::example_command("environment")
            .env("ARGX_PORT", "not-a-port")
            .assert()
            .failure()
            .stdout_eq("")
            .stderr_eq(snapbox::str![[r#"
error: invalid value `not-a-port` from environment variable `ARGX_PORT` for `--port`: invalid digit found in string

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

    #[test]
    fn structured_help_example_renders_documented_sections_and_groups() {
        support::example_command("structured_help")
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Inspect objects in a workspace.

The command keeps its longer explanation alongside the Rust type that defines it.

Usage: structured-help [OPTIONS]

Options:
  -h, --help  Print help

Output:
  --json           Emit structured JSON output.
  --field <FIELD>  Include one output field; repeat the option to select more than one.

Examples:
    structured-help --field id --field title
    structured-help --json

Machine-readable usage:
Use the application's schema command when a structured command contract is needed.

"#]])
            .stderr_eq("");
    }

    #[test]
    fn version_actions_use_stdout_and_success_status() {
        support::example_command("version")
            .arg("-V")
            .assert()
            .success()
            .stdout_eq("cli 1.2.3\n")
            .stderr_eq("");
        support::example_command("version")
            .arg("--version")
            .assert()
            .success()
            .stdout_eq("cli 1.2.3 (build abc123)\n")
            .stderr_eq("");
        support::example_command("version")
            .args(["run", "-V"])
            .assert()
            .success()
            .stdout_eq("run 1.2.3\n")
            .stderr_eq("");
        support::example_command("version")
            .args(["run", "--version"])
            .assert()
            .success()
            .stdout_eq("run 1.2.3 (build abc123)\n")
            .stderr_eq("");
        support::example_command("version")
            .arg("internal")
            .assert()
            .success()
            .stdout_eq("")
            .stderr_eq("");
        support::example_command("version")
            .args(["internal", "--version"])
            .assert()
            .failure()
            .stdout_eq("")
            .stderr_eq(snapbox::str![[r#"
error: unknown flag `--version`

For more information, try '--help'.

"#]]);
    }

    #[test]
    fn version_actions_are_scoped_in_generated_help() {
        support::example_command("version")
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Version metadata example.

Usage: cli [OPTIONS] <COMMAND>

Commands:
  run       Runs the versioned command.
  internal  Runs an unversioned command.

Options:
  -h, --help     Print help
  -V, --version  Print version

"#]])
            .stderr_eq("");
        support::example_command("version")
            .args(["run", "--help"])
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Runs the versioned command.

Usage: cli run [OPTIONS]

Options:
  -h, --help     Print help
  -V, --version  Print version

"#]])
            .stderr_eq("");
        support::example_command("version")
            .args(["internal", "--help"])
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Runs an unversioned command.

Usage: cli internal [OPTIONS]

Options:
  -h, --help  Print help

"#]])
            .stderr_eq("");
    }
}
