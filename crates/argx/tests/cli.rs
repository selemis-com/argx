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
            .stderr_eq(support::cli_fixture("basic-unknown.stderr"));
    }

    #[test]
    fn parse_entry_point_prints_help_and_exits_successfully() {
        support::example_command("basic")
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(support::cli_fixture("basic-help.stdout"))
            .stderr_eq("");
    }

    #[test]
    fn nested_help_is_scoped_to_the_selected_command() {
        support::example_command("subcommands")
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(support::cli_fixture("subcommands-help.stdout"))
            .stderr_eq("");
        support::example_command("subcommands")
            .args(["add", "--help"])
            .assert()
            .success()
            .stdout_eq(support::cli_fixture("subcommands-add-help.stdout"))
            .stderr_eq("");
    }
}
