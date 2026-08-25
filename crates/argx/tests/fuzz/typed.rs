//! Property tests for generated typed binding.

#[cfg(unix)]
use std::path::PathBuf;
use std::{cell::RefCell, ffi::OsString};

use argx::{Error as TypedError, Parser as _};
use proptest::{collection, prelude::*, test_runner::TestRunner};

use super::proptest_config;

/// Typed command used to fuzz end-to-end binding and entry-point behavior.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Parser)]
struct TypedRoundTrip {
    /// Optional switch represented by presence or absence of one flag occurrence.
    #[argx(long)]
    verbose: bool,
    /// Optional scalar converted through `FromStr`.
    #[argx(long)]
    number: Option<i64>,
    /// Repeatable UTF-8 values whose order must be preserved.
    #[argx(long)]
    value: Vec<String>,
    /// Optional repeated UTF-8 values that preserve absence.
    #[argx(long)]
    optional_value: Option<Vec<String>>,
    /// Required positional UTF-8 value.
    input: String,
    /// Remaining positional UTF-8 values.
    rest: Vec<String>,
}

/// Deepest reusable group used to fuzz recursive flatten binding.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
struct FlattenLeaf {
    /// Switch owned by the deepest flattened group.
    #[argx(long)]
    leaf_switch: bool,
    /// Optional scalar owned by the deepest flattened group.
    #[argx(long)]
    leaf_number: Option<i64>,
    /// Repeatable values owned by the deepest flattened group.
    #[argx(long)]
    leaf_value: Vec<String>,
    /// Positional contributed between the root's own positionals.
    middle: String,
}

/// Intermediate group used to prove recursive flatten delegation.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
struct FlattenNested {
    /// Child declaration composed recursively.
    #[argx(flatten)]
    leaf: FlattenLeaf,
    /// Optional collection owned by the intermediate group.
    #[argx(long)]
    nested_value: Option<Vec<String>>,
}

/// Sibling flattened group used to exercise independent partial state.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
struct FlattenSibling {
    /// Optional value owned by a separate flattened declaration.
    #[argx(long)]
    sibling: Option<String>,
}

/// Root command used for recursive and sibling flatten round trips.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Parser)]
struct FlattenRoundTrip {
    /// Root-owned switch.
    #[argx(long)]
    root_switch: bool,
    /// Root positional before flattened positional tables.
    head: String,
    /// Recursively flattened declaration.
    #[argx(flatten)]
    nested: FlattenNested,
    /// Independent sibling flattened declaration.
    #[argx(flatten)]
    sibling: FlattenSibling,
    /// Root positional after flattened positional tables.
    tail: String,
    /// Root-owned trailing values.
    rest: Vec<String>,
}

/// Required group used by cross-flatten error-precedence properties.
#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, Eq, argx::Args)]
struct FlattenRequired {
    /// Required named value deliberately left absent in precedence cases.
    #[argx(long)]
    required: String,
}

/// Scalar group used by cross-flatten error-precedence properties.
#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, Eq, argx::Args)]
struct FlattenScalar {
    /// Optional scalar whose repeated occurrences are checked before requiredness.
    #[argx(long)]
    port: Option<u16>,
}

/// Root combining independent required and scalar groups for precedence checks.
#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct FlattenErrors {
    /// Group containing a missing required field.
    #[argx(flatten)]
    required: FlattenRequired,
    /// Group containing repeated or invalid scalar values.
    #[argx(flatten)]
    scalar: FlattenScalar,
}

/// Shared payload fields used by generated subcommand round trips.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
struct SubcommandShared {
    /// Switch composed into one selected child.
    #[argx(long)]
    dry_run: bool,
    /// Repeatable child values.
    #[argx(long)]
    tag: Vec<String>,
}

/// Payload for the first generated root subcommand.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
struct SubcommandAdd {
    /// Reusable flattened child fields.
    #[argx(flatten)]
    shared: SubcommandShared,
    /// Required child positional.
    name: String,
}

/// Payload for a nested generated subcommand.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
struct SubcommandGet {
    /// Required nested positional.
    key: String,
}

/// Nested command set used by generated subcommand round trips.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Subcommand)]
enum SubcommandNested {
    /// Nested payload command.
    Get(SubcommandGet),
    /// Nested unit command.
    Status,
}

/// Payload selecting a second-level command.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Args)]
struct SubcommandConfig {
    /// Parent-child switch.
    #[argx(long)]
    local: bool,
    /// Required nested command.
    #[argx(subcommand)]
    command: SubcommandNested,
}

/// Root generated command set.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Subcommand)]
enum SubcommandChoice {
    /// Payload with flatten composition.
    Add(SubcommandAdd),
    /// Payload with another command set.
    Config(SubcommandConfig),
    /// Root unit command.
    Status,
}

/// Root parser used by subcommand round-trip properties.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, PartialEq, Eq, argx::Parser)]
struct SubcommandRoundTrip {
    /// Root switch parsed before command selection.
    #[argx(long)]
    verbose: bool,
    /// Root positional parsed before command selection.
    workspace: String,
    /// Selected root command.
    #[argx(subcommand)]
    command: SubcommandChoice,
}

/// Scalar payload used by cross-command precedence properties.
#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, Eq, argx::Args)]
struct SubcommandErrorPayload {
    /// Scalar whose duplicate occurrences must beat conversion.
    #[argx(long)]
    port: Option<u16>,
    /// Required value whose absence must beat conversion.
    #[argx(long)]
    required: String,
}

/// One command branch used by precedence properties.
#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, Eq, argx::Subcommand)]
enum SubcommandErrorChoice {
    /// Payload under test.
    Child(SubcommandErrorPayload),
}

/// Root combining parent cardinality and child semantic errors.
#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct SubcommandErrors {
    /// Parent scalar occurrence state.
    #[argx(long)]
    root: bool,
    /// Required selected command.
    #[argx(subcommand)]
    command: SubcommandErrorChoice,
}

/// Generated representable root command value.
#[cfg(feature = "derive")]
#[derive(Debug, Clone)]
enum SubcommandGeneratedChoice {
    /// Root add payload.
    Add {
        /// Flattened switch.
        dry_run: bool,
        /// Repeatable flattened values.
        tags: Vec<String>,
        /// Required child positional.
        name: String,
    },
    /// Nested config/get payload.
    ConfigGet {
        /// Config-level switch.
        local: bool,
        /// Nested required positional.
        key: String,
    },
    /// Nested config/status unit command.
    ConfigStatus {
        /// Config-level switch.
        local: bool,
    },
    /// Root unit command.
    Status,
}

/// One generated complete subcommand value.
#[cfg(feature = "derive")]
#[derive(Debug, Clone)]
struct SubcommandGenerated {
    /// Root switch.
    verbose: bool,
    /// Root positional.
    workspace: String,
    /// Generated branch and payload values.
    command: SubcommandGeneratedChoice,
}

/// Typed command used to fuzz deferred scalar cardinality.
#[cfg(feature = "derive")]
#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct TypedScalar {
    /// Optional scalar whose second occurrence must win over conversion as a duplicate error.
    #[argx(long)]
    port: Option<u16>,
}

/// Typed positional OS value used by the Unix byte-preservation property.
#[cfg(all(feature = "derive", unix))]
#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct TypedPath {
    /// Operating-system-backed positional value.
    path: PathBuf,
}

/// Typed UTF-8 positional used by the invalid-byte rejection property.
#[cfg(all(feature = "derive", unix))]
#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct TypedText {
    /// UTF-8 positional value.
    value: String,
}

/// Typed named OS value used by the Unix attached-value preservation property.
#[cfg(all(feature = "derive", unix))]
#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct TypedPathFlag {
    /// Operating-system-backed named value.
    #[argx(long)]
    path: PathBuf,
}

/// Typed named UTF-8 value used by the Unix attached-value rejection property.
#[cfg(all(feature = "derive", unix))]
#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct TypedTextFlag {
    /// UTF-8 named value.
    #[argx(long)]
    value: String,
}

/// Aggregate measurements for the typed round-trip campaign.
#[cfg(feature = "derive")]
#[derive(Debug, Default)]
struct TypedRoundTripCoverage {
    /// Arguments-only, complete-argv, and non-ASCII argv0 case counts.
    entry_points: [usize; 3],
    /// False/true switches and absent/present optional scalar counts.
    scalars: [usize; 4],
    /// Empty/items counts for `Vec`, `Option<Vec>`, and trailing positionals.
    collections: [usize; 8],
    /// Total, empty, non-ASCII, and Unicode-scalar string counts.
    strings: [usize; 4],
}

#[cfg(feature = "derive")]
impl TypedRoundTripCoverage {
    /// Records one successfully verified round-trip case.
    fn record(&mut self, value: &TypedRoundTrip, argv0: &str) {
        self.entry_points[0] += 1;
        self.entry_points[1] += 1;
        if !argv0.is_ascii() {
            self.entry_points[2] += 1;
        }
        if value.verbose {
            self.scalars[1] += 1;
        } else {
            self.scalars[0] += 1;
        }
        if value.number.is_some() {
            self.scalars[3] += 1;
        } else {
            self.scalars[2] += 1;
        }

        if value.value.is_empty() {
            self.collections[0] += 1;
        }
        self.collections[1] += value.value.len();
        match &value.optional_value {
            None => self.collections[2] += 1,
            Some(items) => {
                self.collections[3] += 1;
                self.collections[4] += items.iter().filter(|item| item.is_empty()).count();
                self.collections[5] += items.len();
            }
        }
        if value.rest.is_empty() {
            self.collections[6] += 1;
        }
        self.collections[7] += value.rest.len();

        self.record_string(&value.input);
        for item in &value.value {
            self.record_string(item);
        }
        if let Some(items) = &value.optional_value {
            for item in items {
                self.record_string(item);
            }
        }
        for item in &value.rest {
            self.record_string(item);
        }
    }

    /// Records one generated typed UTF-8 value.
    fn record_string(&mut self, value: &str) {
        self.strings[0] += 1;
        if value.is_empty() {
            self.strings[1] += 1;
        }
        if !value.is_ascii() {
            self.strings[2] += 1;
        }
        self.strings[3] += value.chars().count();
    }
}

/// Aggregate measurements for recursive flatten round-trip fuzzing.
#[cfg(feature = "derive")]
#[derive(Debug, Default)]
struct FlattenCoverage {
    /// Root and leaf switch false/true counts.
    switches: [usize; 4],
    /// Leaf scalar absent/present and sibling scalar absent/present counts.
    optionals: [usize; 4],
    /// Leaf, nested optional, and trailing collection counts.
    collections: [usize; 7],
    /// Total, empty, non-ASCII, and Unicode-scalar counts across flattened values.
    strings: [usize; 4],
    /// Arguments-only and complete-argv parses verified.
    entry_points: [usize; 2],
}

#[cfg(feature = "derive")]
impl FlattenCoverage {
    /// Records one successfully verified flattened round trip.
    fn record(&mut self, value: &FlattenRoundTrip) {
        self.entry_points[0] += 1;
        self.entry_points[1] += 1;
        let root_switch = if value.root_switch { 1 } else { 0 };
        let leaf_switch = if value.nested.leaf.leaf_switch { 1 } else { 0 };
        let leaf_number = if value.nested.leaf.leaf_number.is_some() { 1 } else { 0 };
        let sibling = if value.sibling.sibling.is_some() { 1 } else { 0 };
        self.switches[root_switch] += 1;
        self.switches[2 + leaf_switch] += 1;
        self.optionals[leaf_number] += 1;
        self.optionals[2 + sibling] += 1;

        if value.nested.leaf.leaf_value.is_empty() {
            self.collections[0] += 1;
        }
        self.collections[1] += value.nested.leaf.leaf_value.len();
        match &value.nested.nested_value {
            None => self.collections[2] += 1,
            Some(items) => {
                self.collections[3] += 1;
                self.collections[4] += items.len();
            }
        }
        if value.rest.is_empty() {
            self.collections[5] += 1;
        }
        self.collections[6] += value.rest.len();

        self.record_string(&value.head);
        self.record_string(&value.nested.leaf.middle);
        self.record_string(&value.tail);
        if let Some(value) = &value.sibling.sibling {
            self.record_string(value);
        }
        for value in &value.nested.leaf.leaf_value {
            self.record_string(value);
        }
        if let Some(values) = &value.nested.nested_value {
            for value in values {
                self.record_string(value);
            }
        }
        for value in &value.rest {
            self.record_string(value);
        }
    }

    /// Records one generated UTF-8 value from a flattened field.
    fn record_string(&mut self, value: &str) {
        self.strings[0] += 1;
        if value.is_empty() {
            self.strings[1] += 1;
        }
        if !value.is_ascii() {
            self.strings[2] += 1;
        }
        self.strings[3] += value.chars().count();
    }
}

/// Aggregate measurements for nested subcommand round-trip fuzzing.
#[cfg(feature = "derive")]
#[derive(Debug, Default)]
struct SubcommandCoverage {
    /// Root add, config/get, config/status, and root status selections.
    variants: [usize; 4],
    /// Root and child switch false/true observations.
    switches: [usize; 6],
    /// Empty tag vectors and total tag items.
    collections: [usize; 2],
    /// Arguments-only and complete-argv parses verified.
    entry_points: [usize; 2],
    /// Total, empty, non-ASCII, and Unicode-scalar string counts.
    strings: [usize; 4],
}

#[cfg(feature = "derive")]
impl SubcommandCoverage {
    /// Records one verified generated command tree.
    fn record(&mut self, value: &SubcommandGenerated) {
        self.entry_points[0] += 1;
        self.entry_points[1] += 1;
        self.switches[usize::from(value.verbose)] += 1;
        self.record_string(&value.workspace);
        match &value.command {
            SubcommandGeneratedChoice::Add { dry_run, tags, name } => {
                self.variants[0] += 1;
                self.switches[2 + usize::from(*dry_run)] += 1;
                if tags.is_empty() {
                    self.collections[0] += 1;
                }
                self.collections[1] += tags.len();
                for tag in tags {
                    self.record_string(tag);
                }
                self.record_string(name);
            }
            SubcommandGeneratedChoice::ConfigGet { local, key } => {
                self.variants[1] += 1;
                self.switches[4 + usize::from(*local)] += 1;
                self.record_string(key);
            }
            SubcommandGeneratedChoice::ConfigStatus { local } => {
                self.variants[2] += 1;
                self.switches[4 + usize::from(*local)] += 1;
            }
            SubcommandGeneratedChoice::Status => self.variants[3] += 1,
        }
    }

    /// Records one generated UTF-8 value.
    fn record_string(&mut self, value: &str) {
        self.strings[0] += 1;
        if value.is_empty() {
            self.strings[1] += 1;
        }
        if !value.is_ascii() {
            self.strings[2] += 1;
        }
        self.strings[3] += value.chars().count();
    }
}

/// Aggregate measurements for cross-command error-precedence fuzzing.
#[cfg(feature = "derive")]
#[derive(Debug, Default)]
struct SubcommandErrorCoverage {
    /// Scalar classes generated for the child value.
    classes: [usize; ScalarTextKind::COUNT],
    /// Raw-over-parent, child-duplicate, required-over-conversion, and missing-command checks.
    checks: [usize; 4],
}

/// Aggregate measurements for cross-flatten error-precedence fuzzing.
#[cfg(feature = "derive")]
#[derive(Debug, Default)]
struct FlattenErrorCoverage {
    /// Scalar semantic classes used as the first occurrence.
    classes: [usize; ScalarTextKind::COUNT],
    /// Duplicate, syntax, requiredness, and child-conversion checks completed.
    checks: [usize; 4],
}

/// Semantic class deliberately generated for scalar conversion fuzzing.
#[cfg(feature = "derive")]
#[derive(Debug, Clone, Copy)]
enum ScalarTextKind {
    /// Text that parses successfully as `u16`.
    Valid,
    /// Unsigned decimal text above the `u16` range.
    Overflow,
    /// Negative decimal text, which is invalid for `u16`.
    Negative,
    /// Empty text.
    Empty,
    /// Arbitrary non-numeric Unicode text.
    NonNumeric,
}

#[cfg(feature = "derive")]
impl ScalarTextKind {
    /// Number of semantic classes represented in coverage counters.
    const COUNT: usize = 5;

    /// Stable coverage-counter index for this scalar class.
    const fn index(self) -> usize {
        match self {
            Self::Valid => 0,
            Self::Overflow => 1,
            Self::Negative => 2,
            Self::Empty => 3,
            Self::NonNumeric => 4,
        }
    }
}

/// One generated scalar input paired with its intended semantic class.
#[cfg(feature = "derive")]
#[derive(Debug, Clone)]
struct ScalarText {
    /// Generated text supplied to the typed parser.
    value: String,
    /// Semantic class selected by the generator.
    kind: ScalarTextKind,
}

/// Aggregate measurements for scalar conversion and precedence fuzzing.
#[cfg(feature = "derive")]
#[derive(Debug, Default)]
struct TypedScalarCoverage {
    /// Counts for each generated first-value semantic class.
    classes: [usize; ScalarTextKind::COUNT],
    /// Parsed and invalid single-value outcomes.
    outcomes: [usize; 2],
    /// Duplicate-over-conversion and syntax-over-duplicate checks.
    precedence: [usize; 2],
    /// First values containing non-ASCII Unicode.
    non_ascii_values: usize,
}

#[cfg(feature = "derive")]
impl TypedScalarCoverage {
    /// Records one successfully verified scalar-precedence case.
    fn record(&mut self, first: &ScalarText, parsed: bool) {
        self.classes[first.kind.index()] += 1;
        if parsed {
            self.outcomes[0] += 1;
        } else {
            self.outcomes[1] += 1;
        }
        self.precedence[0] += 1;
        self.precedence[1] += 1;
        if !first.value.is_ascii() {
            self.non_ascii_values += 1;
        }
    }
}

/// Aggregate measurements for Unix non-UTF-8 typed binding fuzzing.
#[cfg(all(feature = "derive", unix))]
#[derive(Debug, Default)]
struct TypedOsCoverage {
    /// Total bytes, high-bit bytes, and values containing NUL.
    generated: [usize; 3],
    /// Positional and attached `PathBuf` round trips.
    path_round_trips: [usize; 2],
    /// Positional and attached `String` rejections.
    text_rejections: [usize; 2],
}

#[cfg(all(feature = "derive", unix))]
impl TypedOsCoverage {
    /// Records one successfully verified non-UTF-8 case.
    fn record(&mut self, bytes: &[u8]) {
        self.generated[0] += bytes.len();
        self.generated[1] += bytes.iter().filter(|byte| **byte >= 0x80).count();
        if bytes.contains(&0) {
            self.generated[2] += 1;
        }
        self.path_round_trips[0] += 1;
        self.path_round_trips[1] += 1;
        self.text_rejections[0] += 1;
        self.text_rejections[1] += 1;
    }
}

/// Generates bounded arbitrary Unicode strings for typed-binding campaigns.
#[cfg(feature = "derive")]
fn typed_string_strategy() -> impl Strategy<Value = String> {
    collection::vec(any::<char>(), 0..=24)
        .prop_map(|characters| characters.into_iter().collect::<String>())
}

/// Generates one complete set of values representable by [`TypedRoundTrip`].
#[cfg(feature = "derive")]
fn typed_round_trip_strategy() -> impl Strategy<Value = TypedRoundTrip> {
    (
        any::<bool>(),
        proptest::option::of(any::<i64>()),
        collection::vec(typed_string_strategy(), 0..=8),
        prop_oneof![Just(None), collection::vec(typed_string_strategy(), 1..=8).prop_map(Some),],
        typed_string_strategy(),
        collection::vec(typed_string_strategy(), 0..=8),
    )
        .prop_map(|(verbose, number, value, optional_value, input, rest)| TypedRoundTrip {
            verbose,
            number,
            value,
            optional_value,
            input,
            rest,
        })
}

/// Renders typed values into an unambiguous argv sequence for end-to-end round trips.
#[cfg(feature = "derive")]
fn typed_round_trip_argv(value: &TypedRoundTrip) -> Vec<OsString> {
    let mut argv = Vec::new();
    if value.verbose {
        argv.push(OsString::from("--verbose"));
    }
    if let Some(number) = value.number {
        argv.push(OsString::from(format!("--number={number}")));
    }
    argv.extend(value.value.iter().map(|item| OsString::from(format!("--value={item}"))));
    if let Some(optional_values) = &value.optional_value {
        argv.extend(
            optional_values.iter().map(|item| OsString::from(format!("--optional-value={item}"))),
        );
    }
    argv.push(OsString::from("--"));
    argv.push(OsString::from(value.input.as_str()));
    argv.extend(value.rest.iter().map(|item| OsString::from(item.as_str())));
    argv
}

/// Generates values spanning root, recursive, and sibling flattened declarations.
#[cfg(feature = "derive")]
fn flatten_round_trip_strategy() -> impl Strategy<Value = FlattenRoundTrip> {
    (
        any::<bool>(),
        typed_string_strategy(),
        any::<bool>(),
        proptest::option::of(any::<i64>()),
        collection::vec(typed_string_strategy(), 0..=8),
        typed_string_strategy(),
        prop_oneof![Just(None), collection::vec(typed_string_strategy(), 1..=8).prop_map(Some),],
        proptest::option::of(typed_string_strategy()),
        typed_string_strategy(),
        collection::vec(typed_string_strategy(), 0..=8),
    )
        .prop_map(
            |(
                root_switch,
                head,
                leaf_switch,
                leaf_number,
                leaf_value,
                middle,
                nested_value,
                sibling,
                tail,
                rest,
            )| FlattenRoundTrip {
                root_switch,
                head,
                nested: FlattenNested {
                    leaf: FlattenLeaf { leaf_switch, leaf_number, leaf_value, middle },
                    nested_value,
                },
                sibling: FlattenSibling { sibling },
                tail,
                rest,
            },
        )
}

/// Renders flattened typed values into an unambiguous argv sequence.
#[cfg(feature = "derive")]
fn flatten_round_trip_argv(value: &FlattenRoundTrip) -> Vec<OsString> {
    let mut argv = Vec::new();
    if value.root_switch {
        argv.push(OsString::from("--root-switch"));
    }
    if value.nested.leaf.leaf_switch {
        argv.push(OsString::from("--leaf-switch"));
    }
    if let Some(number) = value.nested.leaf.leaf_number {
        argv.push(OsString::from(format!("--leaf-number={number}")));
    }
    argv.extend(
        value
            .nested
            .leaf
            .leaf_value
            .iter()
            .map(|item| OsString::from(format!("--leaf-value={item}"))),
    );
    if let Some(values) = &value.nested.nested_value {
        argv.extend(values.iter().map(|item| OsString::from(format!("--nested-value={item}"))));
    }
    if let Some(sibling) = &value.sibling.sibling {
        argv.push(OsString::from(format!("--sibling={sibling}")));
    }
    argv.push(OsString::from("--"));
    argv.push(OsString::from(value.head.as_str()));
    argv.push(OsString::from(value.nested.leaf.middle.as_str()));
    argv.push(OsString::from(value.tail.as_str()));
    argv.extend(value.rest.iter().map(|item| OsString::from(item.as_str())));
    argv
}

/// Generates complete values spanning unit, payload, flatten, and nested command branches.
#[cfg(feature = "derive")]
fn subcommand_round_trip_strategy() -> impl Strategy<Value = SubcommandGenerated> {
    let branch = prop_oneof![
        4 => (
            any::<bool>(),
            collection::vec(typed_string_strategy(), 0..=8),
            typed_string_strategy(),
        )
            .prop_map(|(dry_run, tags, name)| SubcommandGeneratedChoice::Add {
                dry_run,
                tags,
                name,
            }),
        3 => (any::<bool>(), typed_string_strategy()).prop_map(|(local, key)| {
            SubcommandGeneratedChoice::ConfigGet { local, key }
        }),
        2 => any::<bool>().prop_map(|local| {
            SubcommandGeneratedChoice::ConfigStatus { local }
        }),
        2 => Just(SubcommandGeneratedChoice::Status),
    ];

    (any::<bool>(), typed_string_strategy(), branch).prop_map(|(verbose, workspace, command)| {
        SubcommandGenerated { verbose, workspace: format!("workspace:{workspace}"), command }
    })
}

/// Renders one generated command tree into an unambiguous argv sequence.
#[cfg(feature = "derive")]
fn subcommand_round_trip_argv(value: &SubcommandGenerated) -> Vec<OsString> {
    let mut argv = Vec::new();
    if value.verbose {
        argv.push(OsString::from("--verbose"));
    }
    argv.push(OsString::from(value.workspace.as_str()));
    match &value.command {
        SubcommandGeneratedChoice::Add { dry_run, tags, name } => {
            argv.push(OsString::from("add"));
            if *dry_run {
                argv.push(OsString::from("--dry-run"));
            }
            argv.extend(tags.iter().map(|tag| OsString::from(format!("--tag={tag}"))));
            argv.push(OsString::from("--"));
            argv.push(OsString::from(name.as_str()));
        }
        SubcommandGeneratedChoice::ConfigGet { local, key } => {
            argv.push(OsString::from("config"));
            if *local {
                argv.push(OsString::from("--local"));
            }
            argv.push(OsString::from("get"));
            argv.push(OsString::from("--"));
            argv.push(OsString::from(key.as_str()));
        }
        SubcommandGeneratedChoice::ConfigStatus { local } => {
            argv.push(OsString::from("config"));
            if *local {
                argv.push(OsString::from("--local"));
            }
            argv.push(OsString::from("status"));
        }
        SubcommandGeneratedChoice::Status => argv.push(OsString::from("status")),
    }
    argv
}

/// Converts generated expected data into the derived destination value.
#[cfg(feature = "derive")]
fn expected_subcommand(value: &SubcommandGenerated) -> SubcommandRoundTrip {
    let command = match &value.command {
        SubcommandGeneratedChoice::Add { dry_run, tags, name } => {
            SubcommandChoice::Add(SubcommandAdd {
                shared: SubcommandShared { dry_run: *dry_run, tag: tags.clone() },
                name: name.clone(),
            })
        }
        SubcommandGeneratedChoice::ConfigGet { local, key } => {
            SubcommandChoice::Config(SubcommandConfig {
                local: *local,
                command: SubcommandNested::Get(SubcommandGet { key: key.clone() }),
            })
        }
        SubcommandGeneratedChoice::ConfigStatus { local } => {
            SubcommandChoice::Config(SubcommandConfig {
                local: *local,
                command: SubcommandNested::Status,
            })
        }
        SubcommandGeneratedChoice::Status => SubcommandChoice::Status,
    };
    SubcommandRoundTrip { verbose: value.verbose, workspace: value.workspace.clone(), command }
}

/// Generates scalar text across meaningful `u16` conversion classes.
#[cfg(feature = "derive")]
fn scalar_text_strategy() -> impl Strategy<Value = ScalarText> {
    prop_oneof![
        4 => any::<u16>().prop_map(|value| ScalarText {
            value: value.to_string(),
            kind: ScalarTextKind::Valid,
        }),
        2 => (65_536_u32..=1_065_535_u32).prop_map(|value| ScalarText {
            value: value.to_string(),
            kind: ScalarTextKind::Overflow,
        }),
        2 => any::<u16>().prop_map(|value| {
            let magnitude = u32::from(value) + 1;
            ScalarText {
                value: format!("-{magnitude}"),
                kind: ScalarTextKind::Negative,
            }
        }),
        1 => Just(ScalarText { value: String::new(), kind: ScalarTextKind::Empty }),
        4 => (
            any::<char>().prop_filter("first character must be non-numeric", |character| {
                !character.is_ascii_digit() && !matches!(*character, '-' | '+')
            }),
            collection::vec(any::<char>(), 0..=23),
        )
            .prop_map(|(first, rest)| {
                let value = std::iter::once(first).chain(rest).collect::<String>();
                ScalarText { value, kind: ScalarTextKind::NonNumeric }
            }),
    ]
}

/// Generates encoded Unix values that are not valid UTF-8.
#[cfg(all(feature = "derive", unix))]
fn invalid_utf8_strategy() -> impl Strategy<Value = Vec<u8>> {
    collection::vec(any::<u8>(), 1..=48)
        .prop_filter("generated bytes must be invalid UTF-8", |value| {
            std::str::from_utf8(value).is_err()
        })
}

/// Fuzzes typed binding round trips and the argv0-vs-args entry-point contract.
#[cfg(feature = "derive")]
#[test]
fn typed_binding_round_trips_generated_values() {
    let strategy = (typed_round_trip_strategy(), typed_string_strategy());
    let config = proptest_config("typed_binding_round_trips_generated_values");
    let cases = config.cases;
    let coverage = RefCell::new(TypedRoundTripCoverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |(expected, argv0)| {
        let argv = typed_round_trip_argv(&expected);
        let parsed = TypedRoundTrip::try_parse_args(argv.clone());
        prop_assert_eq!(parsed, Ok(expected.clone()));

        let mut complete = Vec::with_capacity(argv.len() + 1);
        complete.push(OsString::from(argv0.as_str()));
        complete.extend(argv);
        let parsed = TypedRoundTrip::try_parse_from(complete);
        prop_assert_eq!(parsed, Ok(expected.clone()));
        coverage.borrow_mut().record(&expected, &argv0);
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx typed round-trip property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!("[typed fuzz] PASS: {cases} typed round-trip cases");
    eprintln!(
        "[typed fuzz] entry points: args={} | argv0={} | non_ascii_argv0={}",
        coverage.entry_points[0], coverage.entry_points[1], coverage.entry_points[2],
    );
    eprintln!(
        "[typed fuzz] scalars: bool_true={} | bool_false={} | option_i64_some={} | option_i64_none={}",
        coverage.scalars[1], coverage.scalars[0], coverage.scalars[3], coverage.scalars[2],
    );
    eprintln!(
        "[typed fuzz] collections: vec_empty={} | vec_items={} | option_vec_some={} | option_vec_none={} | option_vec_empty_items={} | option_vec_items={} | rest_empty={} | rest_items={}",
        coverage.collections[0],
        coverage.collections[1],
        coverage.collections[3],
        coverage.collections[2],
        coverage.collections[4],
        coverage.collections[5],
        coverage.collections[6],
        coverage.collections[7],
    );
    eprintln!(
        "[typed fuzz] strings: values={} | empty={} | non_ascii={} | unicode_scalars={}",
        coverage.strings[0], coverage.strings[1], coverage.strings[2], coverage.strings[3],
    );
}

/// Fuzzes recursive and sibling flattened binding through both parser entry points.
#[cfg(feature = "derive")]
#[test]
fn flattened_binding_round_trips_generated_values() {
    let strategy = flatten_round_trip_strategy();
    let config = proptest_config("flattened_binding_round_trips_generated_values");
    let cases = config.cases;
    let coverage = RefCell::new(FlattenCoverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |expected| {
        let argv = flatten_round_trip_argv(&expected);
        prop_assert_eq!(FlattenRoundTrip::try_parse_args(argv.clone()), Ok(expected.clone()));

        let mut complete = Vec::with_capacity(argv.len() + 1);
        complete.push(OsString::from("argx-flatten"));
        complete.extend(argv);
        prop_assert_eq!(FlattenRoundTrip::try_parse_from(complete), Ok(expected.clone()));
        coverage.borrow_mut().record(&expected);
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx flattened typed round-trip property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!("[flatten fuzz] PASS: {cases} recursive/sibling flatten round-trip cases");
    eprintln!(
        "[flatten fuzz] entry points: args={} | argv0={}",
        coverage.entry_points[0], coverage.entry_points[1],
    );
    eprintln!(
        "[flatten fuzz] switches: root_false={} | root_true={} | leaf_false={} | leaf_true={}",
        coverage.switches[0], coverage.switches[1], coverage.switches[2], coverage.switches[3],
    );
    eprintln!(
        "[flatten fuzz] optionals: leaf_number_none={} | leaf_number_some={} | sibling_none={} | sibling_some={}",
        coverage.optionals[0], coverage.optionals[1], coverage.optionals[2], coverage.optionals[3],
    );
    eprintln!(
        "[flatten fuzz] collections: leaf_empty={} | leaf_items={} | nested_none={} | nested_some={} | nested_items={} | rest_empty={} | rest_items={}",
        coverage.collections[0],
        coverage.collections[1],
        coverage.collections[2],
        coverage.collections[3],
        coverage.collections[4],
        coverage.collections[5],
        coverage.collections[6],
    );
    eprintln!(
        "[flatten fuzz] strings: values={} | empty={} | non_ascii={} | unicode_scalars={}",
        coverage.strings[0], coverage.strings[1], coverage.strings[2], coverage.strings[3],
    );
}

/// Fuzzes semantic error precedence across independent flattened groups.
#[cfg(feature = "derive")]
#[test]
fn flattened_binding_preserves_error_precedence() {
    let strategy = (scalar_text_strategy(), scalar_text_strategy());
    let config = proptest_config("flattened_binding_preserves_error_precedence");
    let cases = config.cases;
    let coverage = RefCell::new(FlattenErrorCoverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |(first, second)| {
        let first_arg = OsString::from(format!("--port={}", first.value));
        let second_arg = OsString::from(format!("--port={}", second.value));

        prop_assert_eq!(
            FlattenErrors::try_parse_args([first_arg.clone(), second_arg.clone()]),
            Err(TypedError::DuplicateArgument { name: "port" }),
        );
        prop_assert_eq!(
            FlattenErrors::try_parse_args([
                first_arg.clone(),
                second_arg,
                OsString::from("--unknown"),
            ]),
            Err(TypedError::UnknownFlag { token: b"--unknown".to_vec() }),
        );
        prop_assert_eq!(
            FlattenErrors::try_parse_args([first_arg.clone()]),
            Err(TypedError::MissingRequired { name: "required" }),
        );

        let with_required =
            FlattenErrors::try_parse_args([OsString::from("--required=given"), first_arg]);
        match first.value.parse::<u16>() {
            Ok(port) => prop_assert_eq!(
                with_required,
                Ok(FlattenErrors {
                    required: FlattenRequired { required: String::from("given") },
                    scalar: FlattenScalar { port: Some(port) },
                }),
            ),
            Err(_) => match with_required {
                Err(TypedError::InvalidValue(error)) => {
                    prop_assert_eq!(error.name, "port");
                    prop_assert_eq!(error.value.as_str(), first.value.as_str());
                }
                other => {
                    prop_assert!(false, "unexpected flattened conversion result: {other:?}")
                }
            },
        }

        let mut coverage = coverage.borrow_mut();
        coverage.classes[first.kind.index()] += 1;
        for check in &mut coverage.checks {
            *check += 1;
        }
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx flattened error-precedence property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!("[flatten fuzz] PASS: {cases} flattened error-precedence cases");
    eprintln!(
        "[flatten fuzz] first-value classes: valid_u16={} | overflow={} | negative={} | empty={} | non_numeric={}",
        coverage.classes[0],
        coverage.classes[1],
        coverage.classes[2],
        coverage.classes[3],
        coverage.classes[4],
    );
    eprintln!(
        "[flatten fuzz] precedence checks: duplicate_over_required_and_conversion={} | raw_syntax_over_duplicate={} | required_over_conversion={} | child_conversion={}",
        coverage.checks[0], coverage.checks[1], coverage.checks[2], coverage.checks[3],
    );
}

/// Fuzzes typed round trips through payload, unit, flattened, and nested subcommands.
#[cfg(feature = "derive")]
#[test]
fn subcommand_binding_round_trips_generated_trees() {
    let strategy = subcommand_round_trip_strategy();
    let config = proptest_config("subcommand_binding_round_trips_generated_trees");
    let cases = config.cases;
    let coverage = RefCell::new(SubcommandCoverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |value| {
        let argv = subcommand_round_trip_argv(&value);
        let expected = expected_subcommand(&value);
        prop_assert_eq!(SubcommandRoundTrip::try_parse_args(argv.clone()), Ok(expected.clone()));

        let mut complete = vec![OsString::from("argx-subcommand-fuzz")];
        complete.extend(argv);
        prop_assert_eq!(SubcommandRoundTrip::try_parse_from(complete), Ok(expected));

        coverage.borrow_mut().record(&value);
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx subcommand round-trip property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!("[subcommand fuzz] PASS: {cases} nested command round-trip cases");
    eprintln!(
        "[subcommand fuzz] selections: add={} | config_get={} | config_status={} | status={}",
        coverage.variants[0], coverage.variants[1], coverage.variants[2], coverage.variants[3],
    );
    eprintln!(
        "[subcommand fuzz] switches: root_false={} | root_true={} | add_false={} | add_true={} | config_false={} | config_true={}",
        coverage.switches[0],
        coverage.switches[1],
        coverage.switches[2],
        coverage.switches[3],
        coverage.switches[4],
        coverage.switches[5],
    );
    eprintln!(
        "[subcommand fuzz] collections: tags_empty={} | tag_items={}",
        coverage.collections[0], coverage.collections[1],
    );
    eprintln!(
        "[subcommand fuzz] entry points: args={} | argv0={}",
        coverage.entry_points[0], coverage.entry_points[1],
    );
    eprintln!(
        "[subcommand fuzz] strings: values={} | empty={} | non_ascii={} | unicode_scalars={}",
        coverage.strings[0], coverage.strings[1], coverage.strings[2], coverage.strings[3],
    );
}

/// Fuzzes raw and typed error precedence across a selected command boundary.
#[cfg(feature = "derive")]
#[test]
fn subcommand_binding_preserves_error_precedence() {
    let strategy = (scalar_text_strategy(), scalar_text_strategy());
    let config = proptest_config("subcommand_binding_preserves_error_precedence");
    let cases = config.cases;
    let coverage = RefCell::new(SubcommandErrorCoverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |(first, second)| {
        let first_arg = OsString::from(format!("--port={}", first.value));
        let second_arg = OsString::from(format!("--port={}", second.value));

        prop_assert_eq!(
            SubcommandErrors::try_parse_args([
                OsString::from("--root"),
                OsString::from("--root"),
                OsString::from("child"),
                OsString::from("--unknown"),
            ]),
            Err(TypedError::UnknownFlag { token: b"--unknown".to_vec() }),
        );
        prop_assert_eq!(
            SubcommandErrors::try_parse_args([
                OsString::from("child"),
                first_arg.clone(),
                second_arg,
                OsString::from("--required=given"),
            ]),
            Err(TypedError::DuplicateArgument { name: "port" }),
        );
        prop_assert_eq!(
            SubcommandErrors::try_parse_args([OsString::from("child"), first_arg]),
            Err(TypedError::MissingRequired { name: "required" }),
        );
        prop_assert_eq!(
            SubcommandErrors::try_parse_args([OsString::from("--root"), OsString::from("--root"),]),
            Err(TypedError::DuplicateArgument { name: "root" }),
        );

        let mut coverage = coverage.borrow_mut();
        coverage.classes[first.kind.index()] += 1;
        for check in &mut coverage.checks {
            *check += 1;
        }
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx subcommand error-precedence property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!("[subcommand fuzz] PASS: {cases} command error-precedence cases");
    eprintln!(
        "[subcommand fuzz] first-value classes: valid_u16={} | overflow={} | negative={} | empty={} | non_numeric={}",
        coverage.classes[0],
        coverage.classes[1],
        coverage.classes[2],
        coverage.classes[3],
        coverage.classes[4],
    );
    eprintln!(
        "[subcommand fuzz] precedence checks: child_raw_over_parent_duplicate={} | child_duplicate_over_conversion={} | child_required_over_conversion={} | parent_duplicate_over_missing_command={}",
        coverage.checks[0], coverage.checks[1], coverage.checks[2], coverage.checks[3],
    );
}

/// Fuzzes deferred duplicate checking and raw syntax error precedence.
#[cfg(feature = "derive")]
#[test]
fn typed_scalar_errors_follow_binding_precedence() {
    let strategy = (scalar_text_strategy(), scalar_text_strategy());
    let config = proptest_config("typed_scalar_errors_follow_binding_precedence");
    let cases = config.cases;
    let coverage = RefCell::new(TypedScalarCoverage::default());
    let mut runner = TestRunner::new(config);

    let result = runner.run(&strategy, |(first, second)| {
        let first_value = first.value.as_str();
        let second_value = second.value.as_str();
        let first_arg = OsString::from(format!("--port={first_value}"));
        let second_arg = OsString::from(format!("--port={second_value}"));

        let single = TypedScalar::try_parse_args([first_arg.clone()]);
        let parsed = match first_value.parse::<u16>() {
            Ok(port) => {
                prop_assert_eq!(single, Ok(TypedScalar { port: Some(port) }));
                true
            }
            Err(_) => {
                match single {
                    Err(TypedError::InvalidValue(error)) => {
                        prop_assert_eq!(error.name, "port");
                        prop_assert_eq!(error.value.as_str(), first_value);
                        prop_assert!(!error.reason.is_empty());
                    }
                    other => {
                        prop_assert!(false, "unexpected scalar conversion result: {other:?}")
                    }
                }
                false
            }
        };

        prop_assert_eq!(
            TypedScalar::try_parse_args([first_arg.clone(), second_arg.clone()]),
            Err(TypedError::DuplicateArgument { name: "port" }),
        );
        prop_assert_eq!(
            TypedScalar::try_parse_args([first_arg, second_arg, OsString::from("--unknown")]),
            Err(TypedError::UnknownFlag { token: b"--unknown".to_vec() }),
        );
        coverage.borrow_mut().record(&first, parsed);
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx typed error-precedence property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!("[typed fuzz] PASS: {cases} typed scalar/error-precedence cases");
    eprintln!(
        "[typed fuzz] first-value classes: valid_u16={} | overflow={} | negative={} | empty={} | non_numeric={} | non_ascii={}",
        coverage.classes[0],
        coverage.classes[1],
        coverage.classes[2],
        coverage.classes[3],
        coverage.classes[4],
        coverage.non_ascii_values,
    );
    eprintln!(
        "[typed fuzz] single outcomes: parsed={} | invalid_value={}",
        coverage.outcomes[0], coverage.outcomes[1],
    );
    eprintln!(
        "[typed fuzz] precedence checks: duplicate_over_conversion={} | raw_syntax_over_duplicate={}",
        coverage.precedence[0], coverage.precedence[1],
    );
}

/// Fuzzes lossless OS-backed binding and strict UTF-8 rejection on Unix.
#[cfg(all(feature = "derive", unix))]
#[test]
fn typed_binding_preserves_non_utf8_os_values() {
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

    let strategy = invalid_utf8_strategy();
    let config = proptest_config("typed_binding_preserves_non_utf8_os_values");
    let cases = config.cases;
    let coverage = RefCell::new(TypedOsCoverage::default());
    let mut runner = TestRunner::new(config);
    let result = runner.run(&strategy, |bytes| {
        let raw = OsString::from_vec(bytes.clone());
        let parsed = TypedPath::try_parse_args([OsString::from("--"), raw.clone()]);
        let parsed = parsed.expect("arbitrary Unix OS bytes must bind to PathBuf");
        prop_assert_eq!(parsed.path.as_os_str().as_bytes(), bytes.as_slice());

        prop_assert_eq!(
            TypedText::try_parse_args([OsString::from("--"), raw]),
            Err(TypedError::InvalidUtf8 { name: "value", value: bytes.clone() }),
        );

        let mut attached_path = b"--path=".to_vec();
        attached_path.extend_from_slice(&bytes);
        let parsed = TypedPathFlag::try_parse_args([OsString::from_vec(attached_path)]);
        let parsed = parsed.expect("attached arbitrary Unix OS bytes must bind to PathBuf");
        prop_assert_eq!(parsed.path.as_os_str().as_bytes(), bytes.as_slice());

        let mut attached_text = b"--value=".to_vec();
        attached_text.extend_from_slice(&bytes);
        prop_assert_eq!(
            TypedTextFlag::try_parse_args([OsString::from_vec(attached_text)]),
            Err(TypedError::InvalidUtf8 { name: "value", value: bytes.clone() }),
        );
        coverage.borrow_mut().record(&bytes);
        Ok(())
    });
    if let Err(error) = result {
        panic!("Argx typed non-UTF-8 property failed: {error}");
    }

    let coverage = coverage.into_inner();
    eprintln!("[typed fuzz] PASS: {cases} non-UTF-8 typed binding cases");
    eprintln!(
        "[typed fuzz] generated bytes: total={} | high_bit={} | values_with_nul={}",
        coverage.generated[0], coverage.generated[1], coverage.generated[2],
    );
    eprintln!(
        "[typed fuzz] PathBuf round-trips: positional={} | attached={}",
        coverage.path_round_trips[0], coverage.path_round_trips[1],
    );
    eprintln!(
        "[typed fuzz] String rejections: positional={} | attached={}",
        coverage.text_rejections[0], coverage.text_rejections[1],
    );
}
