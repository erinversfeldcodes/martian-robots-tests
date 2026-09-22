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
    grade_with(implementation, &["--spelling", "0"])
}

/// Graded with the generated modes turned down, unless a test is about them:
/// a case-level assertion should not depend on what a generator happened to
/// draw.
fn grade_with(implementation: &Path, extra: &[&str]) -> Report {
    let output = Command::new(env!("CARGO_BIN_EXE_martian-robots-verify"))
        .arg("--bin")
        .arg(implementation)
        .args(extra)
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

#[test]
fn an_implementation_that_understands_only_one_spelling_is_caught() {
    let fixtures = Fixtures::new("one-spelling");
    // Answers every mission the same way — but refuses any input containing a
    // tab. Self-consistent on canonical input, so nothing in the catalogue
    // built from hand-chosen spellings would notice; it disagrees with itself
    // the moment the same mission is written another legal way.
    let implementation = fixtures.implementation(
        "one-spelling",
        "input=$(cat)\ncase \"$input\" in\n  *\"$(printf '\\t')\"*) printf 'line 1: no (R4)\\n' >&2; exit 1 ;;\nesac\nexit 0",
    );

    let report = grade_with(&implementation, &["--spelling", "40", "--seed", "1"]);
    assert!(
        report.stdout.contains("DIVERGES"),
        "a spelling-sensitive implementation must diverge from itself:\n{}",
        report.stdout
    );
    assert!(!report.conformed);
}

#[test]
fn a_seed_names_the_same_corpus_twice() {
    let fixtures = Fixtures::new("seeded");
    let implementation = fixtures.implementation("quiet", "cat > /dev/null\nexit 0");

    let once = grade_with(&implementation, &["--spelling", "10", "--seed", "7"]);
    let again = grade_with(&implementation, &["--spelling", "10", "--seed", "7"]);
    assert_eq!(once.stdout, again.stdout, "a seeded run must be replayable");
    assert!(once.stdout.contains("seed 7"), "{}", once.stdout);
}

#[test]
fn an_implementation_that_answers_the_same_thing_always_is_caught() {
    let fixtures = Fixtures::new("one-answer");
    // Structurally perfect and semantically empty: a well-formed line,
    // canonical numbers, a real orientation. Nothing about its shape is wrong,
    // so only a predicate about meaning can catch it.
    let implementation =
        fixtures.implementation("one-answer", "cat > /dev/null\nprintf '0 0 N\\n'");

    let report = grade_with(
        &implementation,
        &["--spelling", "0", "--properties", "40", "--seed", "3"],
    );
    assert!(
        report.stdout.contains("VIOLATED"),
        "invariants must catch an answer that is shaped right and means nothing:\n{}",
        report.stdout
    );
    assert!(!report.conformed);
}

#[test]
fn an_implementation_that_answers_differently_each_run_is_caught() {
    let fixtures = Fixtures::new("unstable");
    // Hidden state, which no single input can see: every case and every
    // respelling is graded against one run, so only running the same input
    // twice says anything about it.
    let implementation =
        fixtures.implementation("unstable", "cat > /dev/null\nprintf '0 %s N\\n' \"$$\"");

    let report = grade_with(
        &implementation,
        &["--spelling", "0", "--properties", "20", "--seed", "4"],
    );
    assert!(
        report
            .stdout
            .contains("the same input twice gives the same answer"),
        "a program that answers differently each run must be caught:\n{}",
        report.stdout
    );
}

#[test]
fn every_invariant_is_evaluated_by_the_corpus_it_runs_against() {
    let fixtures = Fixtures::new("firing");
    // Answers, and answers with a loss: several predicates say nothing about
    // an implementation that reports nothing, and a corpus cannot evaluate
    // "no two robots are lost on the same cell" against a program that never
    // loses one.
    let implementation =
        fixtures.implementation("always-lost", "cat > /dev/null\nprintf '0 0 N LOST\\n'");

    let report = grade_with(
        &implementation,
        &["--spelling", "0", "--properties", "60", "--seed", "5"],
    );
    for line in report.stdout.lines() {
        // The firing counts are printed as `<count> <name>`; a zero there is a
        // predicate that never evaluated, which reads exactly like one that
        // always held.
        if let Some((count, name)) = line.trim().split_once(' ')
            && let Ok(fired) = count.parse::<u32>()
        {
            assert!(
                fired > 0,
                "no mission evaluated {name:?}:\n{}",
                report.stdout
            );
        }
    }
}

#[test]
fn an_implementation_that_rejects_in_silence_is_caught_by_the_generator_too() {
    let fixtures = Fixtures::new("silent-rejection");
    // Refuses everything with no diagnostic. The catalogue catches this, and
    // so must the generated rejections: a mode that checked only the exit code
    // and an empty stdout would let it pass over a much larger space.
    let implementation = fixtures.implementation("silent", "cat > /dev/null\nexit 1");

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "0",
            "--rejections",
            "40",
            "--seed",
            "6",
        ],
    );
    assert!(
        report.stdout.contains("ACCEPTED") && report.stdout.contains("no diagnostic on stderr"),
        "generated rejections must judge the diagnostic, not just the exit code:\n{}",
        report.stdout
    );
}

#[test]
fn an_implementation_that_blames_the_wrong_line_is_caught() {
    let fixtures = Fixtures::new("wrong-line");
    // Rejects everything, with a diagnostic that always blames line 1 and
    // cites a real ruling. Only deriving the expected line from how the
    // mutation was built catches this.
    let implementation = fixtures.implementation(
        "wrong-line",
        "cat > /dev/null\nprintf 'line 1: something (R5)\\n' >&2\nexit 1",
    );

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "0",
            "--rejections",
            "40",
            "--seed",
            "7",
        ],
    );
    assert!(
        report.stdout.contains("does not name"),
        "the line must come from the mutation, not from the program:\n{}",
        report.stdout
    );
}
