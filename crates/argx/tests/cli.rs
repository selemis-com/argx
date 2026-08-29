//! Executable CLI process-boundary contract tests against public examples.
//!
//! This layer owns observable process policy: exit status and the exact bytes written to stdout and
//! stderr by ordinary `Parser::parse` entry points. Parser semantics should normally be asserted in
//! `argv.rs` or `typed.rs`; these tests stay intentionally small so subprocess snapshots do not
//! become a second semantic test suite.

#[path = "support/command.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_example_builds_and_runs() {
        support::example_command("basic").assert().success().stdout_eq("").stderr_eq("");
    }

    #[test]
    fn parse_entry_point_answers_generated_completion_protocol() {
        support::example_command("basic")
            .env("ARGX_COMPLETE", "1")
            .env("ARGX_COMPLETE_LINE", "cli --")
            .env_remove("ARGX_COMPLETE_WORDS")
            .arg("__argx_complete__")
            .assert()
            .success()
            .stdout_eq("--help\tPrint help\n")
            .stderr_eq("");
    }

    #[test]
    fn parse_entry_point_answers_nushell_span_protocol() {
        support::example_command("basic")
            .env("ARGX_COMPLETE", "1")
            .env("ARGX_COMPLETE_WORDS", r#"["cli","--"]"#)
            .env_remove("ARGX_COMPLETE_LINE")
            .arg("__argx_complete__")
            .assert()
            .success()
            .stdout_eq("--help\tPrint help\n")
            .stderr_eq("");
    }

    #[test]
    fn private_completion_argv_is_not_reserved_without_the_protocol_marker() {
        support::example_command("basic")
            .env_remove("ARGX_COMPLETE")
            .env_remove("ARGX_COMPLETE_LINE")
            .env_remove("ARGX_COMPLETE_WORDS")
            .args(["__argx_complete__", "--line", "cli --"])
            .assert()
            .failure()
            .stdout_eq("");
    }

    #[test]
    fn malformed_current_completion_requests_are_consumed_silently() {
        support::example_command("basic")
            .env("ARGX_COMPLETE", "1")
            .env("ARGX_COMPLETE_LINE", "cli --")
            .args(["__argx_complete__", "unexpected"])
            .assert()
            .success()
            .stdout_eq("")
            .stderr_eq("");
    }

    #[test]
    fn malformed_nushell_span_requests_are_consumed_silently() {
        support::example_command("basic")
            .env("ARGX_COMPLETE", "1")
            .env("ARGX_COMPLETE_WORDS", "not-json")
            .env_remove("ARGX_COMPLETE_LINE")
            .arg("__argx_complete__")
            .assert()
            .success()
            .stdout_eq("")
            .stderr_eq("");
    }

    #[test]
    fn stale_completion_protocol_versions_do_not_reserve_private_argv() {
        support::example_command("basic")
            .env("ARGX_COMPLETE", "0")
            .env("ARGX_COMPLETE_LINE", "cli --")
            .arg("__argx_complete__")
            .assert()
            .failure()
            .stdout_eq("");
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
    fn diagnostics_use_cli_spellings() {
        support::example_command("defaults")
            .arg("--port")
            .assert()
            .failure()
            .stdout_eq("")
            .stderr_eq(snapbox::str![[r#"
error: missing value for `--port`

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
Minimal command with no application-defined arguments.

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
Root command that requires one child command selection.

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
  <VALUE>  Value to add or remove.

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
Root command with independently configured version metadata.

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
