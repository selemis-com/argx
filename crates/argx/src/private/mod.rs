//! Implementation details shared with generated code.
//!
//! This module is public so proc-macro expansions can name these items from downstream crates. It
//! is not part of Argx's stable user-facing API. The re-exports below are intentionally narrow: they
//! form the generated-code surface shared between the derive crate and the runtime crate.

mod compose;
mod contract;
mod model;
mod scope;
mod traits;
mod value;

pub use compose::{
    action_flag_spellings_disjoint, argument_key_by_name, command_keys_unique, concat_args,
    concat_constraints, concat_contract_args, concat_contract_flags, concat_flags,
    concat_help_groups, flag_spellings_unique, positional_layout_valid, table_len,
};
pub use contract::{ArgSpec, Cardinality, CommandSpec, FlagSpec};
pub use model::{
    Action, ActionKind, Arg, ArgumentState, Command, Constraint, ConstraintKind, Flag, HELP_ACTION,
    HelpGroup, HelpSection, Key, key_base,
};
pub(crate) use scope::{Named, long as resolve_long, short as resolve_short};
pub use traits::{CommandArgs, CommandContract, Subcommands};
pub use value::{
    RawValue, os_value, os_values, parsed_value, parsed_values, text_value, text_values,
};

pub use crate::argv::{ArgvParser, Error, Event};
