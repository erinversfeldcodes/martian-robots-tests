//! The suite graded through its own boundary.
//!
//! Every other test in this repository exercises a module. These run the real
//! binary against implementations built to be wrong in one specific way, and
//! assert that the case written for that defect is the one that reports it. A
//! class that finds nothing and a class that never runs print the same thing;
//! these are how they are told apart.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Fixtures {
    directory: PathBuf,
}

impl Fixtures {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!("martian-robots-verify-{name}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a directory to put fixtures in");
        Self { directory }
    }

    /// An implementation that is wrong in exactly one way.
    fn implementation(&self, name: &str, script: &str) -> PathBuf {
        let path = self.directory.join(name);
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("to write a fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("to make it runnable");
        path
    }
}

impl Drop for Fixtures {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct Report {
    stdout: String,
    conformed: bool,
}

impl Report {
    /// Why the named case failed, or `None` if it did not fail.
    fn failure(&self, case: &str) -> Option<String> {
        let mut lines = self
            .stdout
            .lines()
            .skip_while(|line| !(line.starts_with("FAIL") && line.contains(case)));
        lines.next()?;
        Some(
            lines
                .take_while(|line| line.starts_with("      "))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    fn passed(&self, case: &str) -> bool {
        self.stdout
            .lines()
            .any(|line| line.starts_with("ok") && line.contains(case))
    }
}

fn grade(implementation: &Path) -> Report {
    let output = Command::new(env!("CARGO_BIN_EXE_martian-robots-verify"))
        .arg("--bin")
        .arg(implementation)
        .output()
        .expect("to run the suite");
    Report {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        conformed: output.status.success(),
    }
}

#[test]
fn an_implementation_that_prints_as_it_goes_is_caught_by_the_all_or_nothing_case() {
    let fixtures = Fixtures::new("streams");
    // Prints a robot's answer, then rejects. Every rejection case whose defect
    // is on the first line would let this pass: nothing has been printed yet
    // when the input is refused.
    let implementation = fixtures.implementation(
        "streams",
        "cat > /dev/null\nprintf '1 1 E\\n'\nprintf 'line 8: too big (R5)\\n' >&2\nexit 1",
    );

    let report = grade(&implementation);
    let why = report
        .failure("nothing is printed for the robots before a bad one")
        .expect("the all-or-nothing case must catch an implementation that streams");
    assert!(
        why.contains("no stdout"),
        "caught for the wrong reason: {why}"
    );
    assert!(!report.conformed);
}

#[test]
fn an_implementation_that_rejects_without_saying_why_is_caught() {
    let fixtures = Fixtures::new("silent");
    let implementation = fixtures.implementation("silent", "cat > /dev/null\nexit 1");

    let report = grade(&implementation);
    let why = report
        .failure("a grid coordinate past the maximum is refused")
        .expect("a rejection with no diagnostic must be caught");
    assert!(
        why.contains("no diagnostic on stderr"),
        "caught for the wrong reason: {why}"
    );
}

#[test]
fn an_implementation_that_invents_a_line_number_is_caught() {
    let fixtures = Fixtures::new("invents");
    // Correct in every respect except that it attributes the missing grid line
    // to a line that is not there — which §2.5 forbids, and which a suite that
    // only checked for required substrings would never notice.
    let implementation = fixtures.implementation(
        "invents",
        "cat > /dev/null\nprintf 'line 1: no grid line (R12)\\n' >&2\nexit 1",
    );

    let report = grade(&implementation);
    let why = report
        .failure("empty input is refused without inventing a line to blame")
        .expect("a fabricated line reference must be caught");
    assert!(
        why.contains("no line is attributable"),
        "caught for the wrong reason: {why}"
    );
}

#[test]
fn an_implementation_that_ignores_its_arguments_is_caught() {
    let fixtures = Fixtures::new("ignores-argv");
    // The defect R20's rationale exists for: argv is not looked at, so a help
    // flag produces whatever the empty stdin produces.
    let implementation = fixtures.implementation("ignores-argv", "cat > /dev/null\nexit 0");

    let report = grade(&implementation);
    let why = report
        .failure("--help alone prints usage and exits 0")
        .expect("a program that ignores argv must fail the help case");
    assert!(
        why.contains("no usage on stdout"),
        "caught for the wrong reason: {why}"
    );
}

#[test]
fn a_diagnostic_may_name_a_rule_this_suite_did_not_expect() {
    let fixtures = Fixtures::new("other-tag");
    // R1 and R5 both govern a start that is off the world and over the limit.
    // §2.5 says one governing tag suffices, so a suite that demanded a
    // particular one would fail a conforming implementation.
    let implementation = fixtures.implementation(
        "other-tag",
        "cat > /dev/null\nprintf 'line 2: outside the world (R1)\\n' >&2\nexit 1",
    );

    let report = grade(&implementation);
    assert!(
        report.passed("a robot coordinate past the maximum is refused"),
        "the tag obligation must accept any governing ruling:\n{}",
        report.stdout
    );
}

#[test]
fn a_failing_run_says_so_in_its_exit_code() {
    let fixtures = Fixtures::new("exit-code");
    let implementation = fixtures.implementation("nothing", "cat > /dev/null\nexit 0");
    assert!(!grade(&implementation).conformed);
}
