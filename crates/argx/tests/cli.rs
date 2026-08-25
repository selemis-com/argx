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
}
