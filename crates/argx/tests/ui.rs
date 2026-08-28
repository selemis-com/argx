//! Downstream compiler UI contract tests for derive expansions.
//!
//! These fixtures own declaration-time acceptance and rejection: invalid attributes, unsupported
//! shapes, invalid relationships/defaults, flatten/subcommand misuse, and renamed dependencies.
//! Runtime parser behavior belongs to the integration suites; keeping invalid declarations here
//! lets failures be checked at the same compilation boundary users encounter.

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
            ("defaults", "argx"),
            ("environment", "argx"),
            ("flatten", "argx"),
            ("subcommands", "argx"),
            ("handler", "argx"),
            ("value_enum", "argx"),
        ] {
            support::assert_ui_success(fixture, dependency);
        }
    }

    #[test]
    fn invalid_value_enum_declarations_are_rejected_deterministically() {
        support::assert_ui_failure(
            "invalid_value_enum",
            "argx",
            snapbox::str![[r#"
error: ValueEnum can only be derived for enums
error: ValueEnum does not support generic enums
error: ValueEnum requires at least one variant
error: ValueEnum variants cannot contain fields
error: duplicate ValueEnum spelling `foo`
error: `value_enum` is only valid on value-taking arguments
error: `value_enum` cannot depend on the containing struct's generic parameters; use a concrete ValueEnum type
error: `value_enum` takes no value

"#]],
        );
    }

    #[test]
    fn duplicate_handlers_are_rejected_by_coherence() {
        support::assert_ui_failure(
            "duplicate_handler",
            "argx",
            snapbox::str![[r#"
error[E0119]: conflicting implementations of trait `HandlerSchemaSource` for type `Command`
error[E0119]: conflicting implementations of trait `argx::__private::SchemaCommand` for type `Command`

"#]],
        );
    }

    #[test]
    fn handlers_cannot_attach_to_non_invocable_command_groups() {
        support::assert_ui_failure(
            "non_invocable_handler",
            "argx",
            snapbox::str![[r#"
error[E0277]: the trait bound `GroupArgs: argx::InvocableHandlerCommand` is not satisfied

"#]],
        );
    }

    #[test]
    fn handler_result_types_must_implement_schema() {
        support::assert_ui_failure(
            "non_schema_handler_result",
            "argx",
            snapbox::str![[r#"
error[E0277]: the trait bound `Output: argx::__private::schemars::JsonSchema` is not satisfied

"#]],
        );
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
error: `global` is only valid on named flags
error: `flatten` cannot be combined with flag, value, or help attributes
error: `subcommand` cannot be combined with flag, value, or help attributes
error: `global` takes no value

"#]],
        );
    }

    #[test]
    fn invalid_alias_declarations_are_rejected_deterministically() {
        support::assert_ui_failure(
            "invalid_aliases",
            "argx",
            snapbox::str![[r#"
error: command aliases are only valid on Subcommand variants
error: `alias` and `aliases` are only valid on named flags
error: plural alias attributes require at least one value
error: long flag must be non-empty, must not start with `-`, and cannot contain `=`, whitespace, or controls
error: `--help` is reserved by Argx
error: duplicate subcommand `second`
error: duplicate subcommand spelling `run`

"#]],
        );
    }

    #[test]
    fn invalid_constraint_declarations_are_rejected_deterministically() {
        support::assert_ui_failure(
            "invalid_constraints",
            "argx",
            snapbox::str![[r#"
error: `requires` names no argument field `token` in this command
error: `requires` cannot reference its own field `value`
error: duplicate `requires` reference `token`
error: `requires` array must contain at least one target
error: `conflicts` targets must be string literals
error: argument `endpoint` cannot both require and conflict with `token`
error: `requires` target `command` is not an argument field
error: `requires` and `conflicts` are only valid on argument fields
error: `requires` and `conflicts` are only valid on argument fields
error[E0080]: evaluation panicked: constraint target must name exactly one argument field in the composed command

"#]],
        );
    }

    #[test]
    fn invalid_environment_declarations_are_rejected_deterministically() {
        support::assert_ui_failure(
            "invalid_env",
            "argx",
            snapbox::str![[r#"
error: `env` is only supported on scalar value-taking flags
error: `env` is only supported on scalar value-taking flags
error: `env` is only supported on scalar value-taking flags
error: duplicate `env` attribute
error: environment variable name must be non-empty and cannot contain `=` or NUL
error: environment variable name must be non-empty and cannot contain `=` or NUL
error: `env` requires a string value

"#]],
        );
    }

    #[test]
    fn invalid_default_declarations_are_rejected_deterministically() {
        support::assert_ui_failure(
            "invalid_defaults",
            "argx",
            snapbox::str![[r#"
error: `default` is only supported on scalar value-taking flags
error: `default` is only supported on scalar value-taking flags
error: `default` is only supported on scalar value-taking flags
error: duplicate `default` attribute

"#]],
        );
    }

    #[test]
    fn default_expression_types_are_checked_by_rust() {
        // Keep these as direct type mismatches. Diagnostics about generated `match` arms would
        // leak macro implementation details instead of pointing users at their default expression.
        support::assert_ui_failure(
            "invalid_default_types",
            "argx",
            snapbox::str![[r#"
error[E0308]: mismatched types
error[E0308]: mismatched types

"#]],
        );
    }

    #[test]
    fn selected_ui_diagnostics_remain_anchored_in_user_source() {
        support::assert_ui_failure_spans(
            "invalid_default_types",
            "argx",
            &[
                ("mismatched types", r#"default = "3000""#),
                ("mismatched types", "default = Some(3000_u16)"),
            ],
        );
        support::assert_ui_failure_spans(
            "invalid_handler",
            "argx",
            &[
                (
                    "#[argx::handler] requires a command type or inherent handler method",
                    "#[argx::handler]",
                ),
                ("Argx handlers must be non-generic", "fn generic_handler<T>()"),
                (
                    "Argx handlers require a concrete Result<Success, Error> return type",
                    "fn missing_result()",
                ),
                (
                    "Argx handlers do not support opaque `impl Trait` return types",
                    "fn opaque_result() -> impl Sized",
                ),
                (
                    "unsupported Argx handler arguments; expected one command type",
                    "#[argx::handler(UnsupportedArguments, error = ())]",
                ),
                (
                    "#[argx::handler(...)] can only be applied to a free function or inherent impl",
                    "#[argx::handler(NotAFunction)]",
                ),
            ],
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
    fn invalid_version_metadata_is_rejected_deterministically() {
        support::assert_ui_failure(
            "invalid_version",
            "argx",
            snapbox::str![[r#"
error: `--version` is reserved when command version metadata is present
error: `-V` is reserved when command version metadata is present
error: version metadata is only valid on Parser declarations and Subcommand variants
error[..]: [..]command contains a flag spelling reserved by a built-in action
error[..]: [..]subcommand contains a flag spelling reserved by a built-in action

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
error[..]: [..]flattened command contains duplicate long or short flag spellings

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
error: Subcommand requires at least one variant
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
