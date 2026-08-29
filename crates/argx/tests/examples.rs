//! Public-example inventory and behavior tests.
//!
//! This layer protects the repository's documented example surface. It deliberately does not
//! duplicate the exact help and diagnostic snapshots in `process.rs`; instead it proves that every
//! checked-in public example is part of the expected inventory, actually routes `--help` through
//! Argx, and successfully demonstrates the representative invocation advertised by its docs.

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, path::Path, process::Command};

    use snapbox::cmd::compile_examples;

    const PUBLIC_EXAMPLES: &[&str] =
        &["arguments", "basic", "commands", "complete", "completions", "configuration", "schema"];
    #[test]
    fn every_public_example_builds_renders_help_and_demonstrates_its_behavior() {
        let examples = compile_examples(["--all-features"])
            .unwrap_or_else(|error| panic!("failed to compile examples: {error}"));
        let mut actual = BTreeSet::new();

        for (name, binary) in examples {
            let binary = binary.unwrap_or_else(|error| panic!("failed to compile {name}: {error}"));
            let inserted = actual.insert(name.clone());
            assert!(inserted, "example `{name}` was returned more than once");

            assert_help(&name, &binary);
            assert_advertised_behavior(&name, &binary);
        }

        let expected = PUBLIC_EXAMPLES.iter().copied().collect::<BTreeSet<_>>();
        let actual = actual.iter().map(String::as_str).collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "public example inventory changed without updating this test");
    }
    fn assert_help(name: &str, binary: &Path) {
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

        if name == "arguments" {
            assert!(
                stdout.contains("Output format. [possible values: human, json, json-lines]",),
                "arguments --help omitted the derived vocabulary:\n{stdout}",
            );
        }
    }

    fn assert_advertised_behavior(name: &str, binary: &Path) {
        let mut command = Command::new(binary);
        command.env("NO_COLOR", "1");

        if name == "completions" {
            command.arg("bash");
            let output = command
                .output()
                .unwrap_or_else(|error| panic!("failed to execute {name}: {error}"));
            assert!(
                output.status.success(),
                "{name} representative invocation exited with {}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr),
            );
            assert!(output.stderr.is_empty(), "{name} wrote to stderr");

            let stdout = String::from_utf8(output.stdout)
                .unwrap_or_else(|error| panic!("{name} stdout was not UTF-8: {error}"));
            assert!(
                stdout.contains("complete -F _argx_complete_completions 'completions'"),
                "{name} did not register its Bash completion function:\n{stdout}",
            );
            assert!(
                stdout.contains("ARGX_COMPLETE_LINE="),
                "{name} did not emit the Argx completion protocol adapter:\n{stdout}",
            );
            return;
        }

        let (expected_stdout, expected_stderr) = match name {
            "arguments" => {
                command.args(["input.txt", "--format", "json", "--colour", "always"]);
                (
                    "",
                    concat!(
                        "input: input.txt\n",
                        "format: json\n",
                        "color: always\n",
                        "stdout: false\n",
                    ),
                )
            }
            "basic" => ("", ""),
            "commands" => {
                command.args(["--verbose", "add", "hello", "--force"]);
                ("", "verbose mode enabled\nforce add: hello\n")
            }
            "complete" => {
                command.args(["get", "object-7", "-O", "json"]);
                ("{\"id\":\"object-7\",\"limit\":20}\n", "")
            }
            "configuration" => {
                command.args(["--workers", "8", "--endpoint", "https://example.invalid"]);
                ("workers: 8\nendpoint: https://example.invalid\norigins: \n", "")
            }
            "schema" => {
                command.args(["objects", "get", "object-7"]);
                ("id: object-7\n", "")
            }
            other => panic!("missing behavior assertion for public example `{other}`"),
        };

        let output =
            command.output().unwrap_or_else(|error| panic!("failed to execute {name}: {error}"));
        assert!(
            output.status.success(),
            "{name} representative invocation exited with {}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );

        let stdout = String::from_utf8(output.stdout)
            .unwrap_or_else(|error| panic!("{name} stdout was not UTF-8: {error}"));
        let stderr = String::from_utf8(output.stderr)
            .unwrap_or_else(|error| panic!("{name} stderr was not UTF-8: {error}"));

        assert_eq!(stdout, expected_stdout, "unexpected stdout from {name}");
        assert_eq!(stderr, expected_stderr, "unexpected stderr from {name}");
    }
}
