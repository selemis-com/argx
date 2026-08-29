use std::{ffi::OsString, path::PathBuf};

use argx::Parser as _;

#[derive(argx::Parser)]
struct Generic<T> {
    value: T,
}

#[derive(Debug, PartialEq, Eq)]
struct ParseOnly(u16);

impl std::str::FromStr for ParseOnly {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

#[derive(argx::Parser)]
struct ParseOnlyCli {
    value: ParseOnly,
}

#[derive(argx::Parser)]
struct OsValues {
    path: PathBuf,
    #[argx(long = "destination")]
    output: Option<OsString>,
}

fn main() {
    let parsed = Generic::<u16>::try_parse_args(["42"]).expect("typed value");
    assert_eq!(parsed.value, 42);

    let parsed = ParseOnlyCli::try_parse_args(["7"]).expect("custom parsed value");
    assert_eq!(parsed.value, ParseOnly(7));

    let parsed = OsValues::try_parse_args(["input"]).expect("path value");
    assert_eq!(parsed.path, PathBuf::from("input"));
    assert!(parsed.output.is_none());
}
