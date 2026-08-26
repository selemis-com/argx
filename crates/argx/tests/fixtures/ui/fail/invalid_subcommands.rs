#[derive(argx::Subcommand)]
enum Empty {}

#[derive(argx::Subcommand)]
enum DuplicateNames {
    #[argx(name = "same")]
    First,
    #[argx(name = "same")]
    Second,
}

#[derive(argx::Subcommand)]
enum InvalidName {
    #[argx(name = "-bad")]
    Bad,
}

#[derive(argx::Subcommand)]
enum NamedVariant {
    Bad { value: String },
}

#[derive(argx::Subcommand)]
enum TooManyPayloads {
    Bad(String, String),
}

#[derive(argx::Args)]
struct Payload;

#[derive(argx::Subcommand)]
enum WrappedPayload {
    Bad(Option<Payload>),
}

#[derive(argx::Subcommand)]
enum AttributedPayload {
    Run(#[argx(long)] Payload),
}

#[derive(argx::Parser)]
struct ParserPayload;

#[derive(argx::Subcommand)]
enum ParserPayloadCommand {
    Run(ParserPayload),
}

#[derive(argx::Parser)]
struct OptionalField {
    #[argx(subcommand)]
    command: Option<Commands>,
}

#[derive(argx::Parser)]
struct CollectionField {
    #[argx(subcommand)]
    command: Vec<Commands>,
}

#[derive(argx::Parser)]
struct ConflictingField {
    #[argx(subcommand, long)]
    command: Commands,
}

#[derive(argx::Parser)]
struct ValuedSubcommandField {
    #[argx(subcommand = true)]
    command: Commands,
}

#[derive(argx::Parser)]
struct FlattenConflict {
    #[argx(flatten, subcommand)]
    command: Commands,
}

#[derive(argx::Parser)]
struct DuplicateFields {
    #[argx(subcommand)]
    first: Commands,
    #[argx(subcommand)]
    second: Commands,
}

#[derive(argx::Subcommand)]
enum Commands {
    Run,
}

#[derive(argx::Parser)]
struct GenericField<T> {
    #[argx(subcommand)]
    command: T,
}

#[derive(argx::Subcommand)]
enum GenericPayload<T> {
    Run(T),
}

fn main() {}
