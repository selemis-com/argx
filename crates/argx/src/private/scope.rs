//! Shared lexical name resolution for one selected command scope.

use super::model::{Action, Command, Flag};

/// One named parser entry resolved in the selected command scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Named<'a> {
    /// A built-in action declared on the current command.
    Action(&'a Action<'a>),
    /// A local flag or inherited global flag.
    Flag(&'a Flag<'a>),
}

/// Resolves one long spelling using the parser's lexical scope rules.
pub(crate) fn long<'a>(
    command: &'a Command<'a>,
    ancestors: &[&'a Command<'a>],
    name: &[u8],
) -> Option<Named<'a>> {
    command
        .actions
        .iter()
        .copied()
        .find(|action| action.longs.iter().any(|long| long.as_bytes() == name))
        .map(Named::Action)
        .or_else(|| {
            command
                .flags
                .iter()
                .copied()
                .find(|flag| {
                    flag.longs.iter().chain(flag.aliases).any(|long| long.as_bytes() == name)
                })
                .map(Named::Flag)
        })
        .or_else(|| {
            ancestors.iter().rev().find_map(|command| {
                command
                    .flags
                    .iter()
                    .copied()
                    .find(|flag| {
                        flag.global
                            && flag
                                .longs
                                .iter()
                                .chain(flag.aliases)
                                .any(|long| long.as_bytes() == name)
                    })
                    .map(Named::Flag)
            })
        })
}

/// Resolves one short spelling using the parser's lexical scope rules.
pub(crate) fn short<'a>(
    command: &'a Command<'a>,
    ancestors: &[&'a Command<'a>],
    spelling: u8,
) -> Option<Named<'a>> {
    command
        .actions
        .iter()
        .copied()
        .find(|action| action.shorts.contains(&spelling))
        .map(Named::Action)
        .or_else(|| {
            command
                .flags
                .iter()
                .copied()
                .find(|flag| flag.shorts.contains(&spelling))
                .map(Named::Flag)
        })
        .or_else(|| {
            ancestors.iter().rev().find_map(|command| {
                command
                    .flags
                    .iter()
                    .copied()
                    .find(|flag| flag.global && flag.shorts.contains(&spelling))
                    .map(Named::Flag)
            })
        })
}
