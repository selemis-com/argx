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
            ("subcommands", "argx"),
        ] {
            support::assert_ui_success(fixture, dependency);
        }
    }

    #[test]
    fn invalid_derive_shapes_report_contract_errors() {
        support::assert_ui_failure(
            "invalid_shapes",
            "argx",
            snapbox::str![[r#"
error: Parser can only be derived for structs
error: Args can only be derived for structs
error: Subcommand can only be derived for enums

"#]],
        );
    }

    #[test]
    fn invalid_attributes_report_contract_errors() {
        support::assert_ui_failure(
            "invalid_attributes",
            "argx",
            snapbox::str![[r#"
error: unsupported Argx command attribute
error: unsupported Argx field attribute
error: short flag must be one visible ASCII character other than `-` or `=`
error: short flag must be one visible ASCII character other than `-` or `=`
error: unsupported Argx subcommand attribute
error: `allow_hyphen_values` is only valid on named flags
error: value policies are not valid on bool fields

"#]],
        );
    }

    #[test]
    fn invalid_command_models_are_rejected_before_codegen() {
        support::assert_ui_failure(
            "invalid_command_model",
            "argx",
            snapbox::str![[r#"
error: duplicate long flag `--same`
error: duplicate short flag `-x`
error: long flag must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls
error: required positional arguments cannot follow optional positional arguments
error: variadic positional argument must be the last positional argument
error: `--help` is reserved by Argx
error: `-h` is reserved by Argx

"#]],
        );
    }

    #[test]
    fn invalid_flatten_models_are_rejected_before_codegen() {
        support::assert_ui_failure(
            "invalid_flatten",
            "argx",
            snapbox::str![[r#"
error: `flatten` cannot depend on the containing struct's generic parameters; use a concrete derived type
error: `flatten` does not support `Option<T>`; hold the Args struct directly
error: `flatten` does not support collection wrappers; hold one Args struct directly
error: `flatten` cannot be combined with flag, value, or help attributes
error: `flatten` takes no value
error[..]: [..]flattened command contains duplicate long or short flag spellings
error[..]: [..]flattened command contains duplicate long or short flag spellings
error[..]: [..]flattened command contains duplicate argument keys
error[..]: [..]flattened command has an invalid positional layout

"#]],
        );
    }

    #[test]
    fn parser_roots_cannot_be_flattened_as_args_groups() {
        support::assert_ui_failure(
            "parser_as_flatten",
            "argx",
            snapbox::str![[r#"
error[..]: the trait bound `Child: argx::Args` is not satisfied

"#]],
        );
    }

    #[test]
    fn nested_value_wrappers_are_rejected_explicitly() {
        support::assert_ui_failure(
            "nested_value_wrappers",
            "argx",
            snapbox::str![[r#"
error: nested Option and Vec value wrappers are not supported
error: nested Option and Vec value wrappers are not supported

"#]],
        );
    }

    #[test]
    fn tuple_structs_are_rejected_before_codegen() {
        support::assert_ui_failure(
            "tuple_structs",
            "argx",
            snapbox::str![[r#"
error: Parser and Args do not support tuple structs; use named fields
error: Parser and Args do not support tuple structs; use named fields

"#]],
        );
    }

    #[test]
    fn invalid_subcommand_models_are_rejected_before_codegen() {
        support::assert_ui_failure(
            "invalid_subcommands",
            "argx",
            snapbox::str![[r#"
error: duplicate subcommand `same`
error: subcommand name must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls
error: subcommand variants support only unit variants or one unnamed Args payload
error: subcommand tuple variants must contain exactly one Args payload
error: subcommand payload must be one direct Args type
error: unsupported Argx subcommand payload attribute
error: `subcommand` does not support `Option<T>`; hold the Subcommand enum directly
error: `subcommand` does not support collection wrappers
error: `subcommand` cannot be combined with flag, value, or help attributes
error: `subcommand` takes no value
error: `flatten` and `subcommand` cannot be combined
error: a command can contain only one `subcommand` field
error: `subcommand` cannot depend on the containing struct's generic parameters; use a concrete derived type
error: subcommand payload cannot depend on the enum's generic parameters; use a concrete Args type
error[..]: the trait bound `ParserPayload: argx::Args` is not satisfied

"#]],
        );
    }
}
