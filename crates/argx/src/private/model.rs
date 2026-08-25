//! Private runtime projection of normalized command semantics.

/// Stable semantic identity assigned to one command or argument declaration.
///
/// The raw parser also echoes this identity so generated binding code can dispatch without
/// comparing user-visible spellings.
pub type Key = u64;

/// Static command semantics consumed by parsing and help generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command<'a> {
    /// Command name as exposed on the command line.
    pub name: &'a str,
    /// One-line description shown in generated help.
    pub about: Option<&'a str>,
    /// Flags accepted by this command.
    pub flags: &'a [&'a Flag<'a>],
    /// Positional arguments accepted by this command.
    pub args: &'a [&'a Arg<'a>],
    /// Child commands accepted by this command.
    pub subcommands: &'a [&'a Self],
    /// Derive-assigned semantic command identity.
    pub key: Key,
}

impl Command<'static> {
    /// Empty command metadata for use with struct update syntax.
    pub const EMPTY: Self =
        Self { name: "", about: None, flags: &[], args: &[], subcommands: &[], key: 0 };
}

/// Static semantics for one named argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag<'a> {
    /// Derive-assigned semantic argument identity.
    pub key: Key,
    /// Canonical field name used by diagnostics and generated binding code.
    pub name: &'a str,
    /// One-line description shown in generated help.
    pub help: Option<&'a str>,
    /// Long spellings without the leading `--`.
    pub longs: &'a [&'a str],
    /// ASCII short spellings without the leading `-`.
    pub shorts: &'a [u8],
    /// Whether one occurrence consumes a value.
    pub takes_value: bool,
    /// Whether this flag must occur at least once.
    pub required: bool,
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
        help: None,
        longs: &[],
        shorts: &[],
        takes_value: false,
        required: false,
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
    /// Canonical field name used by diagnostics and generated binding code.
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
        state = (state ^ bytes[index] as u32).rotate_left(5).wrapping_mul(0x9e37_79b1);
        index += 1;
    }
    (state as Key) << 32
}

#[cfg(test)]
mod tests {
    use super::key_base;

    #[test]
    fn key_base_is_stable() {
        assert_eq!(key_base("argx::tests", 0x1234_5678), 0xfb07_66cd_0000_0000);
    }

    #[test]
    fn module_path_contributes_to_key_base() {
        assert_ne!(key_base("argx::add", 42), key_base("argx::remove", 42));
    }
}
