//! Downstream compiler tests for the public derive facade.

#[path = "support/ui.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::support;

    #[test]
    fn supported_derive_shapes_compile_downstream() {
        for (fixture, dependency) in [
            ("basic", "argx"),
            ("renamed_dependency", "cli_args"),
            ("typed", "argx"),
            ("flatten", "argx"),
        ] {
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

    #[test]
    fn invalid_attributes_report_contract_errors() {
        let output = support::ui_output("fail", "invalid_attributes", "argx");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostics");
        for expected in [
            "unsupported Argx command attribute",
            "unsupported Argx field attribute",
            "short flag must be one visible ASCII character other than `-` or `=`",
            "`allow_hyphen_values` is only valid on named flags",
            "value policies are not valid on bool fields",
            "unsupported Argx subcommand attribute",
        ] {
            assert!(stderr.contains(expected), "missing diagnostic: {expected}\n{stderr}");
        }
    }

    #[test]
    fn invalid_command_models_are_rejected_before_codegen() {
        let output = support::ui_output("fail", "invalid_command_model", "argx");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostics");
        for expected in [
            "duplicate long flag `--same`",
            "duplicate short flag `-x`",
            "long flag must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls",
            "required positional arguments cannot follow optional positional arguments",
            "variadic positional argument must be the last positional argument",
        ] {
            assert!(stderr.contains(expected), "missing diagnostic: {expected}\n{stderr}");
        }
    }

    #[test]
    fn invalid_flatten_models_are_rejected_before_codegen() {
        let output = support::ui_output("fail", "invalid_flatten", "argx");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostics");
        assert!(
            stderr
                .matches("flattened command contains duplicate long or short flag spellings")
                .count()
                >= 2,
            "missing long/short flattened collision diagnostics:\n{stderr}"
        );
        for expected in [
            "flattened command contains duplicate argument keys",
            "flattened command has an invalid positional layout",
            "`flatten` cannot depend on the containing struct's generic parameters",
            "`flatten` does not support `Option<T>`",
            "`flatten` does not support collection wrappers",
            "`flatten` cannot be combined with flag or value attributes",
            "`flatten` takes no value",
        ] {
            assert!(stderr.contains(expected), "missing diagnostic: {expected}\n{stderr}");
        }
    }

    #[test]
    fn parser_roots_cannot_be_flattened_as_args_groups() {
        let output = support::ui_output("fail", "parser_as_flatten", "argx");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostics");
        assert!(stderr.contains("FlattenArgs"), "missing Args-only flatten diagnostic:\n{stderr}");
    }

    #[test]
    fn nested_value_wrappers_are_rejected_explicitly() {
        let output = support::ui_output("fail", "nested_value_wrappers", "argx");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostics");
        assert!(
            stderr.matches("nested Option and Vec value wrappers are not supported").count() >= 2,
            "missing nested-wrapper diagnostics:\n{stderr}"
        );
    }

    #[test]
    fn tuple_structs_are_rejected_before_codegen() {
        let output = support::ui_output("fail", "tuple_structs", "argx");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostics");
        assert!(
            stderr
                .matches("Parser and Args do not support tuple structs; use named fields")
                .count()
                >= 2,
            "missing tuple-struct diagnostics:\n{stderr}"
        );
    }
}
