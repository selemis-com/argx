//! Const-time composition and validation of independently derived command tables.
//!
//! A parent derive cannot inspect the fields of an independently expanded flattened `Args` type.
//! Generated code therefore composes child tables through this const API and immediately validates
//! invariants that span declaration boundaries: semantic key uniqueness, flag spelling collisions,
//! positional layout, built-in action collisions, and relationship target resolution. Invalid
//! composition becomes a compile-time const-evaluation failure rather than a runtime parser state.

use super::model::{Action, Arg, Constraint, ConstraintKind, Flag, HelpGroup, Key};

/// Returns the total number of entries across several static table slices.
pub const fn table_len<T>(groups: &[&[T]]) -> usize {
    let mut total = 0;
    let mut group = 0;
    while group < groups.len() {
        total += groups[group].len();
        group += 1;
    }
    total
}

/// Concatenates table groups into one static array while preserving group order.
///
/// # Panics
///
/// Panics during const evaluation if `N` is not [`table_len`] of `groups`.
const fn concat<T: Copy, const N: usize>(groups: &[&[T]], placeholder: T) -> [T; N] {
    let mut joined = [placeholder; N];
    let mut at = 0;
    let mut group = 0;
    while group < groups.len() {
        let entries = groups[group];
        let mut index = 0;
        while index < entries.len() {
            joined[at] = entries[index];
            at += 1;
            index += 1;
        }
        group += 1;
    }
    assert!(at == N, "concatenated table length must match table_len");
    joined
}

/// Concatenates flag-table groups into one static array while preserving group order.
pub const fn concat_flags<const N: usize>(
    groups: &[&[&'static Flag<'static>]],
) -> [&'static Flag<'static>; N] {
    static PLACEHOLDER: Flag<'static> = Flag::BOOL;
    concat(groups, &PLACEHOLDER)
}

/// Concatenates positional-table groups into one static array while preserving group order.
pub const fn concat_args<const N: usize>(
    groups: &[&[&'static Arg<'static>]],
) -> [&'static Arg<'static>; N] {
    static PLACEHOLDER: Arg<'static> = Arg::REQUIRED;
    concat(groups, &PLACEHOLDER)
}

/// Concatenates constraint-table groups into one static array while preserving group order.
pub const fn concat_constraints<const N: usize>(groups: &[&[Constraint]]) -> [Constraint; N] {
    const PLACEHOLDER: Constraint =
        Constraint { kind: ConstraintKind::Requires, source: 0, target: 0 };
    concat(groups, PLACEHOLDER)
}

/// Concatenates help-group slices into one static array while preserving composition order.
pub const fn concat_help_groups<const N: usize>(
    groups: &[&[&'static HelpGroup<'static>]],
) -> [&'static HelpGroup<'static>; N] {
    static PLACEHOLDER: HelpGroup<'static> = HelpGroup::EMPTY;
    concat(groups, &PLACEHOLDER)
}

/// Resolves exactly one semantic argument key by its Rust declaration field name.
///
/// # Panics
///
/// Panics during const evaluation when no argument or more than one composed argument has `name`.
pub const fn argument_key_by_name(flags: &[&Flag<'_>], args: &[&Arg<'_>], name: &str) -> Key {
    let mut found = 0;
    let mut key = 0;

    let mut flag = 0;
    while flag < flags.len() {
        if str_eq(flags[flag].name, name) {
            found += 1;
            key = flags[flag].key;
        }
        flag += 1;
    }

    let mut arg = 0;
    while arg < args.len() {
        if str_eq(args[arg].name, name) {
            found += 1;
            key = args[arg].key;
        }
        arg += 1;
    }

    assert!(
        found == 1,
        "constraint target must name exactly one argument field in the composed command",
    );
    key
}

/// Reports whether every flag and positional key on one composed command is unique.
pub const fn command_keys_unique(flags: &[&Flag<'_>], args: &[&Arg<'_>]) -> bool {
    let mut flag = 0;
    while flag < flags.len() {
        let mut other = flag + 1;
        while other < flags.len() {
            if flags[flag].key == flags[other].key {
                return false;
            }
            other += 1;
        }
        let mut arg = 0;
        while arg < args.len() {
            if flags[flag].key == args[arg].key {
                return false;
            }
            arg += 1;
        }
        flag += 1;
    }

    let mut arg = 0;
    while arg < args.len() {
        let mut other = arg + 1;
        while other < args.len() {
            if args[arg].key == args[other].key {
                return false;
            }
            other += 1;
        }
        arg += 1;
    }
    true
}

/// Reports whether no two flags on one composed command answer to the same spelling.
pub const fn flag_spellings_unique(flags: &[&Flag<'_>]) -> bool {
    let mut left = 0;
    while left < flags.len() {
        let mut right = left + 1;
        while right < flags.len() {
            let mut long = 0;
            while long < flag_long_len(flags[left]) {
                let mut other = 0;
                while other < flag_long_len(flags[right]) {
                    if str_eq(flag_long(flags[left], long), flag_long(flags[right], other)) {
                        return false;
                    }
                    other += 1;
                }
                long += 1;
            }

            let mut short = 0;
            while short < flags[left].shorts.len() {
                let mut other = 0;
                while other < flags[right].shorts.len() {
                    if flags[left].shorts[short] == flags[right].shorts[other] {
                        return false;
                    }
                    other += 1;
                }
                short += 1;
            }
            right += 1;
        }
        left += 1;
    }
    true
}

/// Reports whether built-in actions and declared flags have disjoint spellings in one scope.
pub const fn action_flag_spellings_disjoint(actions: &[&Action<'_>], flags: &[&Flag<'_>]) -> bool {
    let mut action = 0;
    while action < actions.len() {
        let mut flag = 0;
        while flag < flags.len() {
            let mut long = 0;
            while long < actions[action].longs.len() {
                let mut other = 0;
                while other < flag_long_len(flags[flag]) {
                    if str_eq(actions[action].longs[long], flag_long(flags[flag], other)) {
                        return false;
                    }
                    other += 1;
                }
                long += 1;
            }

            let mut short = 0;
            while short < actions[action].shorts.len() {
                let mut other = 0;
                while other < flags[flag].shorts.len() {
                    if actions[action].shorts[short] == flags[flag].shorts[other] {
                        return false;
                    }
                    other += 1;
                }
                short += 1;
            }
            flag += 1;
        }
        action += 1;
    }
    true
}

/// Reports whether a composed positional table has deterministic left-to-right binding.
pub const fn positional_layout_valid(args: &[&Arg<'_>]) -> bool {
    let mut optional_seen = false;
    let mut variadic_seen = false;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index];
        if variadic_seen || (arg.required && optional_seen) {
            return false;
        }
        if !arg.required {
            optional_seen = true;
        }
        if arg.variadic {
            variadic_seen = true;
        }
        index += 1;
    }
    true
}

/// Returns the number of canonical and alias long spellings on one flag.
const fn flag_long_len(flag: &Flag<'_>) -> usize {
    flag.longs.len() + flag.aliases.len()
}

/// Returns one canonical or alias long spelling by combined index.
const fn flag_long<'a>(flag: &'a Flag<'a>, index: usize) -> &'a str {
    if index < flag.longs.len() {
        flag.longs[index]
    } else {
        flag.aliases[index - flag.longs.len()]
    }
}

/// Const-compatible string equality used by composed table validation.
const fn str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::__private::HELP_ACTION;

    static ALPHA: Flag<'static> = Flag {
        key: 1,
        name: "alpha",
        longs: &["alpha"],
        aliases: &["first"],
        shorts: b"a",
        ..Flag::BOOL
    };
    static BETA: Flag<'static> =
        Flag { key: 2, name: "beta", longs: &["beta"], shorts: b"b", ..Flag::BOOL };
    static INPUT: Arg<'static> = Arg { key: 3, name: "input", ..Arg::REQUIRED };

    #[test]
    fn table_composition_preserves_group_order() {
        let flag_groups: &[&[&Flag<'static>]] = &[&[&ALPHA], &[], &[&BETA]];
        assert_eq!(table_len(flag_groups), 2);
        assert_eq!(concat_flags::<2>(flag_groups), [&ALPHA, &BETA]);

        let arg_groups: &[&[&Arg<'static>]] = &[&[], &[&INPUT]];
        assert_eq!(concat_args::<1>(arg_groups), [&INPUT]);

        const REQUIRES: Constraint =
            Constraint { kind: ConstraintKind::Requires, source: 1, target: 3 };
        const CONFLICTS: Constraint =
            Constraint { kind: ConstraintKind::Conflicts, source: 2, target: 3 };
        assert_eq!(concat_constraints::<2>(&[&[REQUIRES], &[CONFLICTS]]), [REQUIRES, CONFLICTS],);

        static PRIMARY_HELP: HelpGroup<'static> =
            HelpGroup { heading: "Primary", flags: &[&ALPHA], args: &[] };
        static SECONDARY_HELP: HelpGroup<'static> =
            HelpGroup { heading: "Secondary", flags: &[&BETA], args: &[&INPUT] };
        let help_groups: &[&[&HelpGroup<'static>]] = &[&[&PRIMARY_HELP], &[], &[&SECONDARY_HELP]];
        assert_eq!(table_len(help_groups), 2);
        assert_eq!(concat_help_groups::<2>(help_groups), [&PRIMARY_HELP, &SECONDARY_HELP],);
    }

    #[test]
    fn argument_lookup_resolves_flags_and_positionals() {
        assert_eq!(argument_key_by_name(&[&ALPHA], &[&INPUT], "alpha"), ALPHA.key);
        assert_eq!(argument_key_by_name(&[&ALPHA], &[&INPUT], "input"), INPUT.key);
    }

    #[test]
    #[should_panic(expected = "constraint target must name exactly one argument field")]
    fn argument_lookup_rejects_an_unknown_name() {
        let _ = argument_key_by_name(&[&ALPHA], &[&INPUT], "missing");
    }

    #[test]
    #[should_panic(expected = "constraint target must name exactly one argument field")]
    fn argument_lookup_rejects_an_ambiguous_name() {
        static SAME: Arg<'static> = Arg { key: 4, name: "alpha", ..Arg::REQUIRED };
        let _ = argument_key_by_name(&[&ALPHA], &[&SAME], "alpha");
    }

    #[test]
    fn composed_keys_must_be_unique_across_every_argument_kind() {
        assert!(command_keys_unique(&[&ALPHA, &BETA], &[&INPUT]));

        static FLAG_DUPLICATE: Flag<'static> = Flag { key: 1, ..Flag::BOOL };
        static ARG_DUPLICATE: Arg<'static> = Arg { key: 1, ..Arg::REQUIRED };
        static OTHER_ARG_DUPLICATE: Arg<'static> = Arg { key: 3, ..Arg::REQUIRED };
        assert!(!command_keys_unique(&[&ALPHA, &FLAG_DUPLICATE], &[]));
        assert!(!command_keys_unique(&[&ALPHA], &[&ARG_DUPLICATE]));
        assert!(!command_keys_unique(&[], &[&INPUT, &OTHER_ARG_DUPLICATE]));
    }

    #[test]
    fn composed_flag_spellings_include_aliases_and_shorts() {
        assert!(flag_spellings_unique(&[&ALPHA, &BETA]));

        static LONG_COLLISION: Flag<'static> = Flag { longs: &["alpha"], ..Flag::BOOL };
        static ALIAS_COLLISION: Flag<'static> = Flag { aliases: &["first"], ..Flag::BOOL };
        static SHORT_COLLISION: Flag<'static> = Flag { shorts: b"a", ..Flag::BOOL };
        assert!(!flag_spellings_unique(&[&ALPHA, &LONG_COLLISION]));
        assert!(!flag_spellings_unique(&[&ALPHA, &ALIAS_COLLISION]));
        assert!(!flag_spellings_unique(&[&ALPHA, &SHORT_COLLISION]));
    }

    #[test]
    fn built_in_actions_must_not_collide_with_canonical_alias_or_short_spellings() {
        assert!(action_flag_spellings_disjoint(&[&HELP_ACTION], &[&ALPHA]));

        static LONG_COLLISION: Flag<'static> = Flag { longs: &["help"], ..Flag::BOOL };
        static ALIAS_COLLISION: Flag<'static> = Flag { aliases: &["help"], ..Flag::BOOL };
        static SHORT_COLLISION: Flag<'static> = Flag { shorts: b"h", ..Flag::BOOL };
        assert!(!action_flag_spellings_disjoint(&[&HELP_ACTION], &[&LONG_COLLISION]));
        assert!(!action_flag_spellings_disjoint(&[&HELP_ACTION], &[&ALIAS_COLLISION]));
        assert!(!action_flag_spellings_disjoint(&[&HELP_ACTION], &[&SHORT_COLLISION]));
    }

    #[test]
    fn positional_layout_rejects_required_or_anything_after_open_ended_values() {
        static OPTIONAL: Arg<'static> =
            Arg { key: 4, name: "optional", required: false, ..Arg::REQUIRED };
        static REQUIRED: Arg<'static> = Arg { key: 5, name: "required", ..Arg::REQUIRED };
        static VARIADIC: Arg<'static> =
            Arg { key: 6, name: "variadic", required: false, variadic: true, ..Arg::REQUIRED };

        assert!(positional_layout_valid(&[&REQUIRED, &OPTIONAL]));
        assert!(!positional_layout_valid(&[&OPTIONAL, &REQUIRED]));
        assert!(!positional_layout_valid(&[&VARIADIC, &OPTIONAL]));
    }
}
