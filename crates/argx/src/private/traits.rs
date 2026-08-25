//! Binding contracts implemented by generated parser, argument, and subcommand declarations.

use super::model::{ArgumentState, Command, Key};
use crate::argv::Event;

/// Generated runtime projections of one normalized command declaration.
pub trait CommandArgs: Sized {
    /// Values collected so far during one parse.
    type Partial;

    /// Private runtime command semantics projected from this declaration.
    const COMMAND: &'static Command<'static>;

    /// Creates empty binding state for a new parse.
    fn start() -> Self::Partial;

    /// Applies one raw parser event when it belongs to this declaration.
    ///
    /// Returns whether this declaration owned the event. Occurrence policy is checked after
    /// raw argv parsing completes so syntax errors take precedence over binding errors.
    fn apply(partial: &mut Self::Partial, event: &Event<'_, '_>) -> bool;

    /// Applies environment fallbacks to arguments left absent by argv.
    fn apply_env(partial: &mut Self::Partial);

    /// Returns presence state for one argument owned by this composed command.
    fn argument_state(partial: &Self::Partial, key: Key) -> Option<ArgumentState>;

    /// Validates normalized argument relationships after source fallback and requiredness.
    ///
    /// # Errors
    ///
    /// Returns the first unmet requirement or conflict in declaration order.
    fn check_constraints(partial: &Self::Partial) -> Result<(), crate::Error>;

    /// Validates completed occurrence cardinality before requiredness or conversion.
    ///
    /// # Errors
    ///
    /// Returns an error when a scalar argument occurred more than once.
    fn check_occurrences(partial: &mut Self::Partial) -> Result<(), crate::Error>;

    /// Validates required fields after every occurrence check has succeeded.
    ///
    /// # Errors
    ///
    /// Returns an error when a required argument was not supplied.
    fn check_required(partial: &mut Self::Partial) -> Result<(), crate::Error>;

    /// Validates occurrences, applies environment fallbacks, and checks requiredness.
    ///
    /// # Errors
    ///
    /// Returns an error when typed cardinality or requiredness is not satisfied.
    fn check(partial: &mut Self::Partial) -> Result<(), crate::Error> {
        Self::check_occurrences(partial)?;
        Self::apply_env(partial);
        Self::check_required(partial)?;
        Self::check_constraints(partial)
    }

    /// Converts completed raw binding state into the destination Rust value.
    ///
    /// # Errors
    ///
    /// Returns an error when a required value is absent or a supplied value cannot be
    /// converted to the destination field type.
    fn finish(partial: Self::Partial) -> Result<Self, crate::Error>;
}

/// Generated command semantics and typed binding for a derived subcommand enum.
pub trait Subcommands: Sized {
    /// Values collected for the selected variant during one parse.
    type Partial;

    /// Private runtime command semantics for the enum's named subcommands.
    const COMMANDS: &'static [&'static Command<'static>];

    /// Creates empty selection and binding state.
    fn start() -> Self::Partial;

    /// Reports whether one sibling command has already been selected.
    fn selected(partial: &Self::Partial) -> bool;

    /// Applies a command-selection event or an event belonging to the selected command tree.
    ///
    /// Returns `false` for events not owned by the selected branch so an ancestor declaration can
    /// bind an inherited global argument.
    fn apply(partial: &mut Self::Partial, event: &Event<'_, '_>) -> bool;

    /// Applies environment fallbacks in the selected command tree.
    fn apply_env(partial: &mut Self::Partial);

    /// Validates scalar occurrence policy in the selected command tree.
    ///
    /// # Errors
    ///
    /// Returns the first duplicate scalar argument in the selected branch.
    fn check_occurrences(partial: &mut Self::Partial) -> Result<(), crate::Error>;

    /// Validates required arguments in the selected command tree.
    ///
    /// # Errors
    ///
    /// Returns the first missing required argument or nested subcommand.
    fn check_required(partial: &mut Self::Partial) -> Result<(), crate::Error>;

    /// Validates argument relationships in the selected command tree.
    ///
    /// # Errors
    ///
    /// Returns the first unmet requirement or conflict in the selected branch.
    fn check_constraints(partial: &Self::Partial) -> Result<(), crate::Error>;

    /// Converts selected raw binding state into the destination enum.
    ///
    /// `None` means no sibling was selected; the containing `CommandArgs` implementation owns
    /// the field name used by the resulting missing-subcommand diagnostic.
    ///
    /// # Errors
    ///
    /// Returns a conversion failure from the selected command payload.
    fn finish(partial: Self::Partial) -> Result<Option<Self>, crate::Error>;
}
