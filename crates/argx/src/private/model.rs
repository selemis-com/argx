//! Private runtime projection of normalized command semantics.
//!
//! These types are immutable tables generated once per derive. The raw parser and help renderer
//! borrow them directly, while typed binding refers back to arguments through stable [`Key`]
//! values. Keeping this projection free of destination Rust types lets the runtime parse without
//! reflection or per-invocation command construction.

/// Stable semantic identity assigned to one command or argument declaration.
pub type Key = u64;

/// One normalized relationship between semantic argument identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Constraint {
    /// Relationship behavior.
    pub kind: ConstraintKind,
    /// Semantic identity of the argument declaring the relationship.
    pub source: Key,
    /// Semantic identity of the referenced argument.
    pub target: Key,
}

/// Supported argument relationship kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    /// Supplying the source requires the target to have a value.
    Requires,
    /// Supplying both source and target is invalid.
    Conflicts,
}

/// Runtime presence state for one semantic argument identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgumentState {
    /// Canonical user-facing label used by diagnostics.
    pub diagnostic: &'static str,
    /// Whether argv or environment fallback supplied this argument.
    pub given: bool,
    /// Whether the argument has a value after considering typed defaults.
    pub satisfied: bool,
}

/// Built-in parser action available in one command scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action<'a> {
    /// Canonical semantic action name.
    pub name: &'a str,
    /// Canonical user-facing spelling used by diagnostics.
    pub diagnostic: &'a str,
    /// One-line description shown in generated help.
    pub help: &'a str,
    /// Long spellings without the leading `--`.
    pub longs: &'a [&'a str],
    /// ASCII short spellings without the leading `-`.
    pub shorts: &'a [u8],
    /// Behavior triggered when the action is selected.
    pub kind: ActionKind<'a>,
}

/// Behavior associated with one built-in parser action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind<'a> {
    /// Render generated help for the selected command scope.
    Help,
    /// Render command version information.
    Version {
        /// Text rendered when the short spelling is used.
        short: &'a str,
        /// Text rendered when the long spelling is used.
        long: &'a str,
    },
}

/// One user-authored help section derived from command documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpSection<'a> {
    /// Section heading without Markdown syntax.
    pub heading: &'a str,
    /// Section body rendered verbatim after generated command sections.
    pub body: &'a str,
}

/// One documented group of arguments contributed through a flattened `Args` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpGroup<'a> {
    /// Group heading supplied by the flatten field's Rust documentation.
    pub heading: &'a str,
    /// Named arguments contributed by the flattened declaration.
    pub flags: &'a [&'a Flag<'a>],
    /// Positional arguments contributed by the flattened declaration.
    pub args: &'a [&'a Arg<'a>],
}

impl HelpGroup<'static> {
    /// Empty group used as a const-composition placeholder.
    pub const EMPTY: Self = Self { heading: "", flags: &[], args: &[] };
}

/// Help is present in every command scope and is modeled as an ordinary built-in action.
pub static HELP_ACTION: Action<'static> = Action {
    name: "help",
    diagnostic: "--help",
    help: "Print help",
    longs: &["help"],
    shorts: b"h",
    kind: ActionKind::Help,
};

/// Static command semantics consumed by parsing and help generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command<'a> {
    /// Command name as exposed on the command line.
    pub name: &'a str,
    /// One-line description shown in generated help.
    pub about: Option<&'a str>,
    /// Full command prose shown before generated help sections.
    pub description: Option<&'a str>,
    /// User-authored help sections shown after generated command sections.
    pub help_sections: &'a [HelpSection<'a>],
    /// Documented flattened argument groups in composition order.
    pub help_groups: &'a [&'a HelpGroup<'a>],
    /// Hidden spellings accepted in addition to the canonical command name.
    pub aliases: &'a [&'a str],
    /// Built-in parser actions available in this command scope.
    pub actions: &'a [&'a Action<'a>],
    /// Flags accepted by this command.
    pub flags: &'a [&'a Flag<'a>],
    /// Positional arguments accepted by this command.
    pub args: &'a [&'a Arg<'a>],
    /// Normalized argument relationships in this command scope.
    pub constraints: &'a [Constraint],
    /// Child commands accepted by this command.
    pub subcommands: &'a [&'a Self],
    /// Derive-assigned semantic command identity.
    pub key: Key,
}

impl Command<'static> {
    /// Empty command metadata for use with struct update syntax.
    pub const EMPTY: Self = Self {
        name: "",
        about: None,
        description: None,
        help_sections: &[],
        help_groups: &[],
        aliases: &[],
        actions: &[&HELP_ACTION],
        flags: &[],
        args: &[],
        constraints: &[],
        subcommands: &[],
        key: 0,
    };
}

/// Static semantics for one named argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag<'a> {
    /// Derive-assigned semantic argument identity.
    pub key: Key,
    /// Canonical field name used for value placeholders and semantic identity.
    pub name: &'a str,
    /// Canonical user-facing spelling used by diagnostics.
    pub diagnostic: &'a str,
    /// One-line description shown in generated help.
    pub help: Option<&'a str>,
    /// Canonical long spellings without the leading `--`.
    pub longs: &'a [&'a str],
    /// Hidden long aliases without the leading `--`.
    pub aliases: &'a [&'a str],
    /// ASCII short spellings without the leading `-`.
    pub shorts: &'a [u8],
    /// Whether this flag remains in scope for descendant commands.
    pub global: bool,
    /// Environment variable consulted when argv does not supply this flag.
    pub env: Option<&'a str>,
    /// Whether one occurrence consumes a value.
    pub takes_value: bool,
    /// Whether this flag must occur at least once.
    pub required: bool,
    /// Whether the final value is required when the configured environment variable is unset.
    pub required_if_env_unset: bool,
    /// Whether a detached value may itself be flag-like.
    pub allow_hyphen_values: bool,
    /// Whether a detached negative number may be consumed while other flag-like values are
    /// refused.
    pub allow_negative_numbers: bool,
}

impl Flag<'static> {
    /// A value-less flag for use with struct update syntax.
    pub const BOOL: Self = Self {
        key: 0,
        name: "",
        diagnostic: "",
        help: None,
        longs: &[],
        aliases: &[],
        shorts: &[],
        global: false,
        env: None,
        takes_value: false,
        required: false,
        required_if_env_unset: false,
        allow_hyphen_values: false,
        allow_negative_numbers: false,
    };

    /// A flag that consumes one value for use with struct update syntax.
    pub const VALUE: Self = Self { takes_value: true, ..Self::BOOL };
}

/// Static semantics for one positional argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arg<'a> {
    /// Derive-assigned semantic argument identity.
    pub key: Key,
    /// Canonical field name used by generated binding and help.
    pub name: &'a str,
    /// One-line description shown in generated help.
    pub help: Option<&'a str>,
    /// Whether this positional must receive at least one value.
    pub required: bool,
    /// Whether this positional may receive multiple values.
    pub variadic: bool,
    /// Whether a negative number may bind here while flag parsing remains enabled.
    pub allow_negative_numbers: bool,
}

impl Arg<'static> {
    /// A required single-value positional for use with struct update syntax.
    pub const REQUIRED: Self = Self {
        key: 0,
        name: "",
        help: None,
        required: true,
        variadic: false,
        allow_negative_numbers: false,
    };
}

/// Computes the high 32 bits shared by keys from one derived declaration.
///
/// Generated code supplies the containing module so declarations expanded independently can
/// still be distinguished when their source tokens are otherwise identical.
pub const fn key_base(module: &str, declaration: u32) -> Key {
    let bytes = module.as_bytes();
    let mut state = declaration;
    let mut index = 0;
    while index < bytes.len() {
        state = (state ^ bytes[index] as u32).wrapping_mul(0x0100_0193);
        index += 1;
    }
    (state as Key) << 32
}

#[cfg(test)]
mod tests {
    use super::key_base;

    #[test]
    fn key_base_is_stable() {
        assert_eq!(key_base("argx::tests", 0x1234_5678), 0x6570_cc45_0000_0000);
    }

    #[test]
    fn module_path_contributes_to_key_base() {
        assert_ne!(key_base("argx::add", 42), key_base("argx::remove", 42));
    }
}
