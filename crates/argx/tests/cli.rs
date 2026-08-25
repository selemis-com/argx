//! Real executable facade smoke tests.

#[path = "support/command.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::support;

    #[test]
    fn basic_example_builds_and_runs() {
        support::basic_example_command().assert().success().stdout_eq("").stderr_eq("");
    }
    #[test]
    fn parse_entry_point_reports_errors_and_exits_unsuccessfully() {
        support::basic_example_command()
            .arg("--unknown")
            .assert()
            .failure()
            .stdout_eq("")
            .stderr_eq("error: unknown flag `--unknown`\n\nFor more information, try '--help'.\n");
    }
    #[test]
    fn parse_entry_point_prints_help_and_exits_successfully() {
        support::basic_example_command()
            .arg("--help")
            .assert()
            .success()
            .stdout_eq(concat!(
                "Minimal command with no declared arguments.\n\n",
                "Usage: cli [OPTIONS]\n",
                "\nOptions:\n",
                "  -h, --help  Print help\n",
            ))
            .stderr_eq("");
    }
}
