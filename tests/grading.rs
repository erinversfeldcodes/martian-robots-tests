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
    grade_with(
        implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "0",
            "--rejections",
            "0",
            "--differential",
            "0",
        ],
    )
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

#[test]
fn an_implementation_that_is_wrong_in_a_way_nothing_else_sees_is_caught() {
    let fixtures = Fixtures::new("plausible");
    // Shaped right, self-consistent, and wrong: one well-formed line, on any
    // grid, whatever the instructions say. Every respelling agrees with every
    // other, and only a second implementation can say the answer is not the
    // mission's.
    let implementation = fixtures.implementation("plausible", "cat > /dev/null\nprintf '0 0 N\\n'");

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "0",
            "--rejections",
            "0",
            "--differential",
            "40",
            "--seed",
            "8",
        ],
    );
    assert!(
        report.stdout.contains("DISAGREES with the reference"),
        "the differential must catch a wrong answer nothing else sees:\n{}",
        report.stdout
    );
    assert!(!report.conformed);
}

#[test]
fn an_implementation_that_refuses_everything_cannot_pass_the_spelling_mode() {
    let fixtures = Fixtures::new("refuses-all");
    // Agrees with itself perfectly, because it says the same thing to every
    // input. Folding a refusal into a sentinel and comparing it like an
    // answer reported no divergences on a program that answers nothing.
    let implementation = fixtures.implementation(
        "refuses-all",
        "cat > /dev/null\nprintf 'line 1: no (R5)\\n' >&2\nexit 1",
    );

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "20",
            "--properties",
            "0",
            "--rejections",
            "0",
            "--differential",
            "0",
            "--seed",
            "1",
        ],
    );
    assert!(
        report.stdout.contains("REFUSED a valid mission"),
        "a refused valid mission is not agreement:\n{}",
        report.stdout
    );
    assert!(!report.conformed);
}

#[test]
fn a_formatting_defect_is_reported_as_a_formatting_defect() {
    let fixtures = Fixtures::new("zero-pads");
    let implementation =
        fixtures.implementation("zero-pads", "cat > /dev/null\nprintf '01 1 N\\n'");

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "10",
            "--rejections",
            "0",
            "--differential",
            "0",
            "--seed",
            "1",
        ],
    );
    assert!(
        report.stdout.contains("every line is canonical"),
        "{}",
        report.stdout
    );
    assert!(
        !report.stdout.contains("VIOLATED one line per robot"),
        "an unreadable answer says nothing about how many robots were reported:\n{}",
        report.stdout
    );
}

#[test]
fn a_diagnostic_that_always_blames_one_line_is_caught() {
    let fixtures = Fixtures::new("always-19");
    // Blames line 19 for everything. `"line 19"` contains `"line 1"`, so a
    // substring judge accepted it for every case that demanded line 1.
    let implementation = fixtures.implementation(
        "always-19",
        "cat > /dev/null\nprintf 'line 19: something (R5)\\n' >&2\nexit 1",
    );

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "0",
            "--rejections",
            "0",
            "--differential",
            "0",
        ],
    );
    assert!(
        report
            .failure("a grid coordinate past the maximum is refused")
            .is_some_and(|why| why.contains("does not name line 1")),
        "a line number must be matched whole:\n{}",
        report.stdout
    );
}

#[test]
fn a_program_built_to_conform_passes_everything() {
    // The positive control. Every other fixture here is wrong on purpose and
    // proves a check can go red; this one is right on purpose and proves the
    // checks are not red for a program that has done nothing wrong. A suite
    // cannot see that from inside: over-pinning looks exactly like a thorough
    // gate until somebody tries to satisfy it.
    let report = grade_with(
        Path::new(env!("CARGO_BIN_EXE_probe")),
        &[
            "--spelling",
            "30",
            "--properties",
            "30",
            "--rejections",
            "30",
            "--differential",
            "30",
            "--seed",
            "1",
        ],
    );
    assert!(
        report.conformed,
        "the suite fails a program built to conform:\n{}",
        report.stdout
    );
}

#[test]
fn the_control_stays_eccentric_where_the_contract_is_silent() {
    // The control is only worth something while it keeps making the choices
    // the contract leaves free *differently*. Made conventional, it would
    // stop proving the suite accepts anything but one house style.
    let probe = Path::new(env!("CARGO_BIN_EXE_probe"));

    let refused = Command::new(probe)
        .arg("--nonsense")
        .output()
        .expect("to run the probe");
    assert_eq!(
        refused.status.code(),
        Some(7),
        "the control should not exit 1, which is the code a suite might assume"
    );
    assert!(
        !String::from_utf8_lossy(&refused.stderr)
            .to_lowercase()
            .contains("usage"),
        "the control should not say the word a suite might look for"
    );

    let helped = Command::new(probe).arg("--help").output().expect("to run");
    assert!(
        !String::from_utf8_lossy(&helped.stdout)
            .to_lowercase()
            .contains("usage"),
        "nor in its help"
    );
}

#[test]
fn a_parser_that_swallows_blank_lines_is_caught_by_the_framing_mutations() {
    let fixtures = Fixtures::new("eats-blanks");
    // A parser that skips blanks wherever it finds them, including where one
    // is an instruction line meaning "this robot does nothing". Every case
    // and every respelling in this suite uses blanks only as separators, so
    // only a mutation that puts one somewhere load-bearing notices.
    let implementation = fixtures.implementation(
        "eats-blanks",
        "grep -v '^[[:space:]]*$' | printf '0 0 N\\n'",
    );

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "0",
            "--differential",
            "0",
            "--rejections",
            "60",
            "--seed",
            "3",
        ],
    );
    assert!(
        report.stdout.contains("ACCEPTED"),
        "a program that answers invalid framing must be caught:\n{}",
        report.stdout
    );
}
