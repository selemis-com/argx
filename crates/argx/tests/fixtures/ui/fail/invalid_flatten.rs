#[derive(argx::Args)]
struct Shared {
    #[argx(long = "same")]
    same: bool,
}

#[derive(argx::Parser)]
struct DuplicateAcrossFlatten {
    #[argx(long = "same")]
    parent: bool,
    #[argx(flatten)]
    shared: Shared,
}

#[derive(argx::Args)]
struct SharedShort {
    #[argx(short = 'x')]
    shared: bool,
}

#[derive(argx::Parser)]
struct DuplicateShortAcrossFlatten {
    #[argx(short = 'x')]
    parent: bool,
    #[argx(flatten)]
    shared: SharedShort,
}

#[derive(argx::Args)]
struct OnePositional {
    value: String,
}

#[derive(argx::Parser)]
struct SameGroupTwice {
    #[argx(flatten)]
    first: OnePositional,
    #[argx(flatten)]
    second: OnePositional,
}

#[derive(argx::Args)]
struct OptionalPositional {
    value: Option<String>,
}

#[derive(argx::Parser)]
struct InvalidPositionalBoundary {
    #[argx(flatten)]
    optional: OptionalPositional,
    required: String,
}

#[derive(argx::Args)]
struct Generic<T> {
    #[argx(long)]
    value: Option<T>,
}

#[derive(argx::Parser)]
struct GenericDependent<T> {
    #[argx(flatten)]
    generic: Generic<T>,
}

#[derive(argx::Parser)]
struct OptionalFlatten {
    #[argx(flatten)]
    shared: Option<Shared>,
}

#[derive(argx::Parser)]
struct CollectionFlatten {
    #[argx(flatten)]
    shared: Vec<Shared>,
}

#[derive(argx::Parser)]
struct MixedFlattenAttributes {
    #[argx(flatten, long)]
    shared: Shared,
}

#[derive(argx::Parser)]
struct FlattenWithValue {
    #[argx(flatten = true)]
    shared: Shared,
}

fn main() {}
