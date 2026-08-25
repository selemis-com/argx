//! Implementation details shared with generated code.
//!
//! This module is public so proc-macro expansions can name these items from downstream crates. It
//! is not part of Argx's stable user-facing API.

mod compose;
mod model;
mod traits;
mod value;

pub use compose::{
    action_flag_spellings_disjoint, command_keys_unique, concat_args, concat_flags,
    flag_spellings_unique, positional_layout_valid, table_len,
};
pub use model::{Action, ActionKind, Arg, Command, Flag, HELP_ACTION, Key, key_base};
pub use traits::{CommandArgs, Subcommands};
pub use value::{
    RawValue, os_value, os_values, parsed_value, parsed_values, text_value, text_values,
};

pub use crate::argv::{ArgvParser, Error, Event};
