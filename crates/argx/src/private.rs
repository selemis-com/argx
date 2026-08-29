//! Implementation details shared with generated code.
//!
//! This module is public so proc-macro expansions can name these items from downstream crates. It
//! is not part of Argx's stable user-facing API. The re-exports below are intentionally narrow:
//! they form the generated-code surface shared between the derive crate and the runtime crate.

/// Schemars re-export used by `#[argx(schema)]` expansions.
#[doc(hidden)]
pub use schemars;

pub(crate) use crate::cli::command::{Named, long as resolve_long, short as resolve_short};
/// Configuration derive support named by generated code.
#[doc(hidden)]
pub use crate::config::__private::{
    Config, Environment, EnvironmentContract, EnvironmentError, environment_name,
    parse_environment_field, serde,
};
/// Schema-discovery support named by generated code.
#[doc(hidden)]
pub use crate::schema::{
    Registry as SchemaRegistry, SchemaCommand, SchemaSubcommands,
    register_handler as register_schema_handler,
};
pub use crate::{
    cli::{
        argv::{ArgvParser, Error, Event},
        command::{
            Action, ActionKind, Arg, ArgumentState, Command, Constraint, ConstraintKind, Flag,
            HELP_ACTION, HelpGroup, HelpSection, Key, SCHEMA_ACTION, key_base,
        },
        compose::{
            action_flag_spellings_disjoint, argument_key_by_name, command_keys_unique, concat_args,
            concat_constraints, concat_flags, concat_help_groups, flag_spellings_unique,
            positional_layout_valid, table_len,
        },
        protocol::{
            CommandArgs, HandlerResult, HandlerSchemas, InvocableCommandHandler, Subcommands,
        },
        value::{
            os_value, os_values, parsed_value, parsed_values, text_value, text_values,
            value_enum_value, value_enum_values,
        },
    },
};
