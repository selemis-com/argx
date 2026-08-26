//! Public-example inventory and smoke tests.
//!
//! This layer protects the repository's documented example surface. It deliberately does not
//! duplicate the exact help and diagnostic snapshots in `cli.rs`; instead it proves that every
//! checked-in public example is part of the expected inventory, compiles, and actually routes
//! `--help` through Argx rather than merely exiting successfully.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::process::Command;

    use snapbox::cmd::compile_examples;

    const PUBLIC_EXAMPLES: &[&str] = &[
        "aliases",
        "basic",
        "constraints",
        "defaults",
        "environment",
        "flatten",
        "structured_help",
        "subcommands",
        "version",
    ];

    #[test]
    fn every_public_example_builds_and_renders_help() {
        let examples = compile_examples(["--all-features"])
            .unwrap_or_else(|error| panic!("failed to compile examples: {error}"));
        let mut actual = BTreeSet::new();

        for (name, binary) in examples {
            let binary = binary.unwrap_or_else(|error| panic!("failed to compile {name}: {error}"));
            let inserted = actual.insert(name.clone());
            assert!(inserted, "example `{name}` was returned more than once");

            let output = Command::new(binary)
                .env("NO_COLOR", "1")
                .arg("--help")
                .output()
                .unwrap_or_else(|error| panic!("failed to execute {name} --help: {error}"));

            assert!(
                output.status.success(),
                "{name} --help exited with {}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr),
            );
            assert!(output.stderr.is_empty(), "{name} --help wrote to stderr");

            let stdout = String::from_utf8(output.stdout)
                .unwrap_or_else(|error| panic!("{name} --help was not UTF-8: {error}"));
            assert!(
                stdout.contains("Usage:"),
                "{name} --help succeeded without rendering Argx help:\n{stdout}",
            );
        }

        let expected = PUBLIC_EXAMPLES.iter().copied().collect::<BTreeSet<_>>();
        let actual = actual.iter().map(String::as_str).collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "public example inventory changed without updating this test");
    }
}
