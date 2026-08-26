//! Private static projection used to build public machine contracts.
//!
//! This projection intentionally does not reuse the runtime `Flag` and `Arg` tables. Runtime tables
//! contain parser-oriented details, while this table is
//! limited to stable invocation semantics that may be exposed publicly. Both projections are
//! generated from the same derive-time semantic model so they cannot drift through independent
//! attribute interpretation.

use super::model::{Constraint, Key};

/// Static machine-contract semantics for one command declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec<'a> {
    /// Command name exposed on the command line.
    pub name: &'a str,
    /// One-line command description.
    pub about: Option<&'a str>,
    /// Hidden command aliases accepted in addition to `name`.
    pub aliases: &'a [&'a str],
    /// Named arguments declared in this command context.
    pub flags: &'a [&'a FlagSpec<'a>],
    /// Positional arguments declared in this command context.
    pub args: &'a [&'a ArgSpec<'a>],
    /// Argument relationships declared in this command context.
    pub constraints: &'a [Constraint],
    /// Selectable child command contracts.
    pub subcommands: &'a [&'a Self],
}

/// Static machine-contract semantics for one named argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagSpec<'a> {
    /// Semantic identity used to resolve normalized relationships.
    pub key: Key,
    /// Rust-facing semantic name used as the human-readable value label.
    pub name: &'a str,
    /// One-line argument description.
    pub help: Option<&'a str>,
    /// Canonical long spellings without the leading `--`.
    pub longs: &'a [&'a str],
    /// Hidden long aliases without the leading `--`.
    pub aliases: &'a [&'a str],
    /// ASCII short spellings without the leading `-`.
    pub shorts: &'a [u8],
    /// Whether this argument remains in scope after entering descendants.
    pub global: bool,
    /// Environment variable consulted after argv when configured.
    pub env: Option<&'a str>,
    /// Rust value cardinality represented by this argument.
    pub cardinality: Cardinality,
    /// Whether the resolved argument value is required when no typed default exists.
    pub required: bool,
    /// Whether absence is satisfied by a typed Rust default expression.
    pub has_default: bool,
    /// Whether detached values may themselves be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether detached negative numbers may be consumed while other flag-like values are refused.
    pub allow_negative_numbers: bool,
}

/// Static machine-contract semantics for one positional argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgSpec<'a> {
    /// Semantic identity used to resolve normalized relationships.
    pub key: Key,
    /// Human-readable positional value name.
    pub name: &'a str,
    /// One-line argument description.
    pub help: Option<&'a str>,
    /// Rust value cardinality represented by this argument.
    pub cardinality: Cardinality,
    /// Whether the resolved argument value is required when no typed default exists.
    pub required: bool,
    /// Whether absence is satisfied by a typed Rust default expression.
    pub has_default: bool,
    /// Whether negative numbers may bind while flag parsing remains enabled.
    pub allow_negative_numbers: bool,
}

/// Rust value cardinality projected into the machine contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// A value-less named boolean switch.
    Switch,
    /// Exactly one resolved value.
    One,
    /// Zero or one value.
    Optional,
    /// Zero or more values.
    Many,
}
