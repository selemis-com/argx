#[derive(argx::Parser)]
#[argx(version = "1.0.0")]
struct ReservedLongVersion {
    #[argx(long = "version")]
    value: bool,
}

#[derive(argx::Parser)]
#[argx(long_version = "1.0.0 (build abc)")]
struct ReservedShortVersion {
    #[argx(short = 'V')]
    value: bool,
}

#[derive(argx::Args)]
#[argx(version = "1.0.0")]
struct ArgsMetadata;

#[derive(argx::Args)]
struct FlattenedVersionFlag {
    #[argx(long = "version")]
    value: bool,
}

#[derive(argx::Parser)]
#[argx(version = "1.0.0")]
struct FlattenedVersionCollision {
    #[argx(flatten)]
    values: FlattenedVersionFlag,
}

#[derive(argx::Args)]
struct VersionPayload {
    #[argx(long = "version")]
    value: bool,
}

#[derive(argx::Subcommand)]
enum VersionCommands {
    #[argx(version = "1.0.0")]
    Run(VersionPayload),
}

#[derive(argx::Parser)]
struct VersionRoot {
    #[argx(subcommand)]
    command: VersionCommands,
}

fn main() {}
