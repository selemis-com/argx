//! Compiler-UI test support.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{self, Output},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use snapbox::{
    cmd::{Command, OutputAssert},
    prelude::IntoData,
};

/// Serializes Cargo invocations that share the UI-test target directory.
static CARGO_LOCK: Mutex<()> = Mutex::new(());
/// Supplies unique temporary project names within the current test process.
static NEXT_PROJECT: AtomicUsize = AtomicUsize::new(0);

/// One temporary downstream Cargo project compiled by the UI harness.
#[derive(Debug)]
struct UiProject {
    /// Project root in the system temporary directory.
    root: PathBuf,
}

impl UiProject {
    /// Creates a downstream project for one fixture and dependency spelling.
    fn new(group: &str, fixture: &str, dependency: &str) -> Self {
        let id = NEXT_PROJECT.fetch_add(1, Ordering::Relaxed);
        let root =
            env::temp_dir().join(format!("argx-ui-{}-{id}-{group}-{fixture}", process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap_or_else(|error| {
                panic!("failed to clear temporary UI project `{}`: {error}", root.display())
            });
        }
        fs::create_dir_all(root.join("src")).unwrap_or_else(|error| {
            panic!("failed to create temporary UI project `{}`: {error}", root.display())
        });

        let facade = facade_root().to_string_lossy().replace('\\', "/");
        let dependency = if dependency == "argx" {
            format!("argx = {{ path = \"{facade}\" }}")
        } else {
            format!("{dependency} = {{ package = \"argx\", path = \"{facade}\" }}")
        };
        let manifest = format!(
            "[package]\nname = \"argx-ui-{group}-{fixture}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\n{dependency}\n"
        );
        fs::write(root.join("Cargo.toml"), manifest).unwrap_or_else(|error| {
            panic!("failed to write temporary UI manifest `{}`: {error}", root.display())
        });

        let source =
            facade_root().join("tests/fixtures/ui").join(group).join(format!("{fixture}.rs"));
        fs::copy(&source, root.join("src/main.rs")).unwrap_or_else(|error| {
            panic!("failed to copy UI fixture `{}`: {error}", source.display())
        });

        Self { root }
    }

    /// Builds the Cargo command used for this downstream project.
    fn command(&self) -> Command {
        let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        Command::new(cargo)
            .current_dir(&self.root)
            .env("CARGO_TARGET_DIR", ui_target_dir())
            .args(["check", "--quiet", "--color", "never"])
    }
}

impl Drop for UiProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Compiles one downstream fixture and asserts successful, silent output.
#[track_caller]
pub(crate) fn assert_ui_success(fixture: &str, dependency: &str) {
    ui_output("pass", fixture, dependency).success().stdout_eq("").stderr_eq("");
}

/// Compiles one downstream fixture and asserts its normalized failure diagnostics.
#[track_caller]
pub(crate) fn assert_ui_failure(fixture: &str, dependency: &str, expected_stderr: impl IntoData) {
    ui_output("fail", fixture, dependency).failure().stdout_eq("").stderr_eq(expected_stderr);
}

/// Compiles one downstream fixture and exposes normalized output to Snapbox.
#[track_caller]
fn ui_output(group: &str, fixture: &str, dependency: &str) -> OutputAssert {
    let _guard = CARGO_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let project = UiProject::new(group, fixture, dependency);
    let output = project
        .command()
        .output()
        .unwrap_or_else(|error| panic!("failed to compile UI fixture `{fixture}`: {error}"));
    OutputAssert::new(normalize_output(output, group == "fail"))
}

/// Removes Cargo and rustc noise that is not part of the diagnostic contract.
///
/// Successful fixtures retain every diagnostic so unexpected warnings fail the test. Failing
/// fixtures retain primary error headings while discarding compiler-owned spans and suggestions.
fn normalize_output(mut output: Output, primary_errors_only: bool) -> Output {
    let stderr = String::from_utf8_lossy(&output.stderr).replace('\\', "/");
    let mut lines = stderr
        .lines()
        .filter(|line| !line.starts_with("error: could not compile `argx-ui-"))
        .filter(|line| !line.starts_with("error: process didn't exit successfully:"))
        .filter(|line| !line.starts_with("error: aborting due to"))
        .filter(|line| !primary_errors_only || line.starts_with("error"))
        .filter(|line| {
            let Some((_, marker)) = line.split_once('|') else {
                return true;
            };
            let marker = marker.trim();
            marker.is_empty() || !marker.chars().all(|character| character == '^')
        })
        .collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }

    let mut normalized = lines.join("\n");
    if !normalized.is_empty() {
        normalized.push('\n');
    }
    output.stderr = normalized.into_bytes();
    output
}

/// Returns the `argx` facade crate root used by downstream UI projects.
fn facade_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Returns a shared UI-test target directory without leaving build artifacts under the crate.
fn ui_target_dir() -> PathBuf {
    if let Some(target) = env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(target).join("ui-tests");
    }

    facade_root()
        .parent()
        .and_then(Path::parent)
        .expect("argx must live below the workspace root")
        .join("target/ui-tests")
}
