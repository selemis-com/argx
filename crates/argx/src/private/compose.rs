//! Const-time composition and validation of independently derived command tables.

use super::model::{Action, Arg, Flag};

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

/// Concatenates flag-table groups into one static array while preserving group order.
///
/// # Panics
///
/// Panics during const evaluation if `N` is not [`table_len`] of `groups`.
pub const fn concat_flags<const N: usize>(
    groups: &[&[&'static Flag<'static>]],
) -> [&'static Flag<'static>; N] {
    static PLACEHOLDER: Flag<'static> = Flag::BOOL;
    let mut joined = [&PLACEHOLDER; N];
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
    assert!(at == N, "concatenated flag-table length must match table_len");
    joined
}

/// Concatenates positional-table groups into one static array while preserving group order.
///
/// # Panics
///
/// Panics during const evaluation if `N` is not [`table_len`] of `groups`.
pub const fn concat_args<const N: usize>(
    groups: &[&[&'static Arg<'static>]],
) -> [&'static Arg<'static>; N] {
    static PLACEHOLDER: Arg<'static> = Arg::REQUIRED;
    let mut joined = [&PLACEHOLDER; N];
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
    assert!(at == N, "concatenated positional-table length must match table_len");
    joined
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
