use argx::Parser as _;

#[derive(Debug, PartialEq, Eq, argx::Args)]
struct Generic<T> {
    #[argx(long)]
    value: Option<T>,
}

#[derive(Debug, PartialEq, Eq, argx::Args)]
struct Nested {
    #[argx(flatten)]
    generic: Generic<u16>,
    middle: String,
}

#[derive(Debug, PartialEq, Eq, argx::Args)]
struct GenericParent<T> {
    #[argx(flatten)]
    nested: Nested,
    #[argx(long)]
    own: Option<T>,
}

#[derive(Debug, PartialEq, Eq, argx::Parser)]
struct Cli {
    before: String,
    #[argx(flatten)]
    generic: GenericParent<u32>,
    after: String,
}

fn main() {
    let parsed = Cli::try_parse_from(["argx-test",
        "--value=42",
        "--own=7",
        "before",
        "middle",
        "after",
    ])
    .expect("flattened arguments");
    assert_eq!(parsed.generic.nested.generic.value, Some(42));
    assert_eq!(parsed.generic.own, Some(7));
    assert_eq!(parsed.before, "before");
    assert_eq!(parsed.generic.nested.middle, "middle");
    assert_eq!(parsed.after, "after");
}
