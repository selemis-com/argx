#[derive(argx::Parser)]
struct MissingTarget {
    #[argx(long, requires = "token")]
    endpoint: Option<String>,
}

#[derive(argx::Parser)]
struct SelfRequirement {
    #[argx(long, requires = "value")]
    value: bool,
}

#[derive(argx::Parser)]
struct DuplicateRequirement {
    #[argx(long, requires = "token", requires = "token")]
    endpoint: Option<String>,
    #[argx(long)]
    token: Option<String>,
}

#[derive(argx::Parser)]
struct EmptyConstraintArray {
    #[argx(long, requires = [])]
    endpoint: Option<String>,
}

#[derive(argx::Parser)]
struct NonStringConstraintTarget {
    #[argx(long, conflicts = ["quiet", 42])]
    output: Option<String>,
    #[argx(long)]
    quiet: bool,
}

#[derive(argx::Parser)]
struct ContradictoryRelationship {
    #[argx(long, requires = "token", conflicts = "token")]
    endpoint: Option<String>,
    #[argx(long)]
    token: Option<String>,
}

#[derive(argx::Subcommand)]
enum Command {
    Run,
}

#[derive(argx::Parser)]
struct NonArgumentTarget {
    #[argx(long, requires = "command")]
    verbose: bool,
    #[argx(subcommand)]
    command: Command,
}

#[derive(argx::Args)]
struct Shared {
    #[argx(long = "first")]
    value: bool,
}

#[derive(argx::Args)]
struct Other {
    #[argx(long = "second")]
    value: bool,
}

#[derive(argx::Parser)]
struct AmbiguousFlattenTarget {
    #[argx(long, requires = "value")]
    source: bool,
    #[argx(flatten)]
    first: Shared,
    #[argx(flatten)]
    second: Other,
}

fn main() {}

#[derive(argx::Args)]
struct FlattenTarget {
    #[argx(long)]
    value: bool,
}

#[derive(argx::Parser)]
struct RelationshipOnFlatten {
    #[argx(flatten, requires = "value")]
    shared: FlattenTarget,
}

#[derive(argx::Parser)]
struct RelationshipOnSubcommand {
    #[argx(subcommand, conflicts = "verbose")]
    command: Command,
    #[argx(long)]
    verbose: bool,
}
