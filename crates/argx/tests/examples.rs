//! Smoke tests for every checked-in public example.

#[cfg(test)]
mod tests {
    use snapbox::cmd::{Command, compile_examples};

    #[test]
    fn every_example_builds_and_accepts_help() {
        let examples = compile_examples(["--all-features"])
            .unwrap_or_else(|error| panic!("failed to compile examples: {error}"));
        let mut count = 0_usize;
        for (name, binary) in examples {
            let binary = binary.unwrap_or_else(|error| panic!("failed to compile {name}: {error}"));
            Command::new(binary)
                .env("NO_COLOR", "1")
                .arg("--help")
                .assert()
                .success()
                .stderr_eq("");
            count += 1;
        }
        assert!(count > 0, "at least one public example must be compiled");
    }
}
