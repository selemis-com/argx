//! Downstream compiler tests for the public derive facade.

#[path = "support/ui.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::support;

    #[test]
    fn supported_derive_shapes_compile_downstream() {
        for (fixture, dependency) in [("basic", "argx"), ("renamed_dependency", "cli_args")] {
            let output = support::ui_output("pass", fixture, dependency);
            assert!(
                output.status.success(),
                "downstream fixture `{fixture}` failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stderr.is_empty(),
                "downstream fixture `{fixture}` emitted diagnostics:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn invalid_derive_shapes_report_contract_errors() {
        let output = support::ui_output("fail", "invalid_shapes", "argx");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostics");
        for expected in [
            "Parser can only be derived for structs",
            "Args can only be derived for structs",
            "Subcommand can only be derived for enums",
        ] {
            assert!(stderr.contains(expected), "missing diagnostic: {expected}\n{stderr}");
        }
    }
}
