//! Implementation details shared with generated code.
//!
//! This module is public so proc-macro expansions can name these items from downstream crates. It
//! is not part of Argx's stable user-facing API.

mod compose;
mod model;
mod traits;
mod value;

pub use compose::{
    command_keys_unique, concat_args, concat_flags, flag_spellings_unique, positional_layout_valid,
    table_len,
};
pub use model::{Arg, Command, Flag, Key, key_base};
pub use traits::{CommandArgs, FlattenArgs, Subcommands};
pub use value::{os_value, os_values, parsed_value, parsed_values, text_value, text_values};

pub use crate::argv::{ArgvParser, Error, Event};
