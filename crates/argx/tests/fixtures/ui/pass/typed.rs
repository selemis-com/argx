use std::{ffi::OsString, path::PathBuf};

use argx::Parser as _;

#[derive(argx::Parser)]
struct Generic<T> {
    value: T,
}

#[derive(argx::Parser)]
struct OsValues {
    path: PathBuf,
    #[argx(long)]
    output: Option<OsString>,
}

fn main() {
    let parsed = Generic::<u16>::try_parse_args(["42"]).expect("typed value");
    assert_eq!(parsed.value, 42);

    let parsed = OsValues::try_parse_args(["input"]).expect("path value");
    assert_eq!(parsed.path, PathBuf::from("input"));
    assert!(parsed.output.is_none());
}
