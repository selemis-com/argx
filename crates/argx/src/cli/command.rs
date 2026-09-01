//! Private static projection of normalized command semantics.
//!
//! These immutable tables are generated once per derive and shared by parsing, help rendering,
//! typed binding, and machine-readable schema discovery. Typed binding refers back to arguments
//! through stable [`Key`] values, while semantic Rust value types remain in a separate lazy
//! projection so parsing does not require schema support.

/// Stable semantic identity assigned to one command or argument declaration.
pub type Key = u64;

/// One application-defined semantic metadata entry attached to a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataEntry<'a> {
    /// Application-defined metadata key.
    pub key: &'a str,
    /// Structured metadata value preserved for machine-readable projections.
    pub value: MetadataValue<'a>,
}

/// Static JSON-like value supported by command metadata.
#[derive(Debug, Clone, Copy)]
pub enum MetadataValue<'a> {
    /// Explicit null value.
    Null,
    /// Boolean value.
    Bool(bool),
    /// Signed integer value.
    Integer(i64),
    /// Floating-point value.
    Float(f64),
    /// UTF-8 string value.
    String(&'a str),
    /// Ordered collection of metadata values.
    Array(&'a [Self]),
    /// JSON object.
    Object(&'a [MetadataEntry<'a>]),
}

impl PartialEq for MetadataValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left.to_bits() == right.to_bits(),
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Array(left), Self::Array(right)) => left == right,
            (Self::Object(left), Self::Object(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for MetadataValue<'_> {}

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

/// One set of arguments from which exactly one member must be supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneOf<'a> {
    /// Semantic identities of the participating arguments.
    pub members: &'a [Key],
}

/// One set of arguments from which at least one member must be supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnyOf<'a> {
    /// Semantic identities of the participating arguments.
    pub members: &'a [Key],
}

/// Runtime presence state for one semantic argument identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgumentState {
    /// Canonical user-facing label used by diagnostics.
    pub diagnostic: &'static str,
    /// Whether argv supplied this argument.
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
    /// Render machine-readable schema for the selected command scope.
    Schema,
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

/// Schema discovery is injected dynamically only for schema-enabled parser roots.
pub static SCHEMA_ACTION: Action<'static> = Action {
    name: "schema",
    diagnostic: "--schema",
    help: "Print machine-readable schema",
    longs: &["schema"],
    shorts: b"S",
    kind: ActionKind::Schema,
};

/// Static command semantics shared by parsing, help generation, and schema discovery.
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
    /// Application-defined semantic metadata preserved for machine-readable consumers.
    pub metadata: &'a [MetadataEntry<'a>],
    /// Built-in parser actions available in this command scope.
    pub actions: &'a [&'a Action<'a>],
    /// Flags accepted by this command.
    pub flags: &'a [&'a Flag<'a>],
    /// Positional arguments accepted by this command.
    pub args: &'a [&'a Arg<'a>],
    /// Normalized argument relationships in this command scope.
    pub constraints: &'a [Constraint],
    /// Argument sets requiring exactly one explicitly supplied member.
    pub one_of: &'a [OneOf<'a>],
    /// Argument sets requiring at least one explicitly supplied member.
    pub any_of: &'a [AnyOf<'a>],
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
        metadata: &[],
        actions: &[&HELP_ACTION],
        flags: &[],
        args: &[],
        constraints: &[],
        one_of: &[],
        any_of: &[],
        subcommands: &[],
        key: 0,
    };
}

/// Schema-relevant semantic type of one CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueSchema {
    /// Ordinary string value.
    Lexical,
    /// Boolean value.
    Boolean,
    /// Integer value.
    Integer,
    /// Numeric value.
    Number,
    /// Chrono date value recognized when the `chrono` integration is enabled.
    Date,
    /// Chrono date-time value recognized when the `chrono` integration is enabled.
    DateTime,
    /// UUID value recognized when the `uuid` integration is enabled.
    Uuid,
    /// URL value recognized when the `url` integration is enabled.
    Url,
}

impl ValueSchema {
    /// Returns the JSON value type used by the semantic invocation schema.
    #[must_use]
    pub(crate) const fn json_type(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Lexical | Self::Date | Self::DateTime | Self::Uuid | Self::Url => "string",
        }
    }

    /// Returns the JSON Schema string format exposed by the enabled integration.
    #[must_use]
    pub(crate) const fn format(self) -> Option<&'static str> {
        match self {
            Self::Date if cfg!(feature = "chrono") => Some("date"),
            Self::DateTime if cfg!(feature = "chrono") => Some("date-time"),
            Self::Uuid if cfg!(feature = "uuid") => Some("uuid"),
            Self::Url if cfg!(feature = "url") => Some("uri"),
            Self::Lexical
            | Self::Boolean
            | Self::Integer
            | Self::Number
            | Self::Date
            | Self::DateTime
            | Self::Uuid
            | Self::Url => None,
        }
    }
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
    /// One-line description shown in compact generated help.
    pub help: Option<&'a str>,
    /// Full description shown in long generated help.
    pub long_help: Option<&'a str>,
    /// Canonical long spellings without the leading `--`.
    pub longs: &'a [&'a str],
    /// Hidden long aliases without the leading `--`.
    pub aliases: &'a [&'a str],
    /// ASCII short spellings without the leading `-`.
    pub shorts: &'a [u8],
    /// Whether this flag remains in scope for descendant commands.
    pub global: bool,
    /// Whether one occurrence consumes a value.
    pub takes_value: bool,
    /// Canonical finite values accepted by this option, when declared as a `ValueEnum`.
    pub accepted_values: &'a [&'a str],
    /// Lazy schema metadata for the destination value type.
    pub value_schema: ValueSchema,
    /// Whether this named argument may occur more than once.
    pub repeatable: bool,
    /// Whether this flag must occur at least once.
    pub required: bool,
    /// Whether absence is satisfied by a typed Rust default expression.
    pub has_default: bool,
    /// Static user-facing spelling of the declared default, when it can be derived safely.
    pub default_value: Option<&'a str>,
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
        long_help: None,
        longs: &[],
        aliases: &[],
        shorts: &[],
        global: false,
        takes_value: false,
        accepted_values: &[],
        value_schema: ValueSchema::Lexical,
        repeatable: false,
        required: false,
        has_default: false,
        default_value: None,
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
    /// One-line description shown in compact generated help.
    pub help: Option<&'a str>,
    /// Full description shown in long generated help.
    pub long_help: Option<&'a str>,
    /// Whether this positional must receive at least one value.
    pub required: bool,
    /// Whether this positional may receive multiple values.
    pub variadic: bool,
    /// Canonical finite values accepted by this positional, when declared as a `ValueEnum`.
    pub accepted_values: &'a [&'a str],
    /// Lazy schema metadata for the destination value type.
    pub value_schema: ValueSchema,
    /// Whether a negative number may bind here while flag parsing remains enabled.
    pub allow_negative_numbers: bool,
}

impl Arg<'static> {
    /// A required single-value positional for use with struct update syntax.
    pub const REQUIRED: Self = Self {
        key: 0,
        name: "",
        help: None,
        long_help: None,
        required: true,
        variadic: false,
        accepted_values: &[],
        value_schema: ValueSchema::Lexical,
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

// Lexical name resolution is kept with the command metadata it resolves so parsing and help
// share one lookup policy.

/// One named parser entry resolved in the selected command scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Named<'a> {
    /// A built-in action declared on the current command.
    Action(&'a Action<'a>),
    /// A local flag or inherited global flag together with its command-path scope index.
    Flag {
        /// Static metadata for the resolved flag.
        flag: &'a Flag<'a>,
        /// Zero-based command-path index where the flag is mounted.
        scope: usize,
    },
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
                .map(|flag| Named::Flag { flag, scope: ancestors.len() })
        })
        .or_else(|| {
            ancestors.iter().enumerate().rev().find_map(|(scope, command)| {
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
                    .map(|flag| Named::Flag { flag, scope })
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
                .map(|flag| Named::Flag { flag, scope: ancestors.len() })
        })
        .or_else(|| {
            ancestors.iter().enumerate().rev().find_map(|(scope, command)| {
                command
                    .flags
                    .iter()
                    .copied()
                    .find(|flag| flag.global && flag.shorts.contains(&spelling))
                    .map(|flag| Named::Flag { flag, scope })
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_path_contributes_to_key_base() {
        assert_ne!(key_base("argx::add", 42), key_base("argx::remove", 42));
    }
}
