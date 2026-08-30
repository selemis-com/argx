//! Executable CLI process-boundary contract tests against public examples.
//!
//! This layer owns observable process policy: exit status and the exact bytes written to stdout and
//! stderr by ordinary `Parser::parse` entry points. Parser semantics should normally be asserted in
//! `argv.rs` or `parser.rs`; these tests stay intentionally small so subprocess snapshots do not
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
        support::example_command("arguments")
            .arg("--format")
            .assert()
            .failure()
            .stdout_eq("")
            .stderr_eq(snapbox::str![[r#"
error: missing value for `--format`

Possible values:
  - human
  - json
  - json-lines

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
  -h, --help
          Print help (see a summary with '-h')

"#]])
            .stderr_eq("");
    }

    #[test]
    fn nested_help_is_scoped_to_the_selected_command() {
        support::example_command("commands")
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Manage values in the example workspace.

The longer command description is authored in the Rust documentation beside its type.

Usage: cli [OPTIONS] <COMMAND>

Commands:
  add     Adds one value.
  remove  Removes one value.
  status  Shows status without additional arguments.

Options:
  -v, --verbose
          Enables verbose output.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Examples:
    commands add hello
    commands --verbose rm hello

"#]])
            .stderr_eq("");
        support::example_command("commands")
            .args(["add", "--help"])
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Adds one value.

Usage: cli add [OPTIONS] <VALUE>

Arguments:
  <VALUE>
          Value to add or remove.

Options:
      --force
          Forces the operation.

  -v, --verbose
          Enables verbose output.

  -h, --help
          Print help (see a summary with '-h')

"#]])
            .stderr_eq("");
    }

    #[test]
    fn missing_required_arguments_render_cli_spelling_and_usage() {
        support::example_command("commands").arg("add").assert().failure().stdout_eq("").stderr_eq(
            snapbox::str![[r#"
error: the following required arguments were not provided:
  <VALUE>

Usage: cli add <VALUE>

For more information, try '--help'.

"#]],
        );
    }

    #[test]
    fn version_actions_use_stdout_and_success_status() {
        support::example_command("commands")
            .arg("-V")
            .assert()
            .success()
            .stdout_eq("cli 1.2.3\n")
            .stderr_eq("");
        support::example_command("commands")
            .arg("--version")
            .assert()
            .success()
            .stdout_eq("cli 1.2.3 (build abc123)\n")
            .stderr_eq("");
        support::example_command("commands")
            .args(["status", "-V"])
            .assert()
            .success()
            .stdout_eq("status 1.2.3\n")
            .stderr_eq("");
        support::example_command("commands")
            .args(["status", "--version"])
            .assert()
            .success()
            .stdout_eq("status 1.2.3 (build abc123)\n")
            .stderr_eq("");
        support::example_command("commands")
            .args(["add", "--version", "value"])
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
        support::example_command("commands")
            .args(["status", "--help"])
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Shows status without additional arguments.

Usage: cli status [OPTIONS]

Options:
  -v, --verbose
          Enables verbose output.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

"#]])
            .stderr_eq("");
        support::example_command("commands")
            .args(["add", "--help"])
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Adds one value.

Usage: cli add [OPTIONS] <VALUE>

Arguments:
  <VALUE>
          Value to add or remove.

Options:
      --force
          Forces the operation.

  -v, --verbose
          Enables verbose output.

  -h, --help
          Print help (see a summary with '-h')

"#]])
            .stderr_eq("");
    }
}
