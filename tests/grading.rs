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

#[test]
fn an_implementation_that_treats_any_odd_byte_as_whitespace_is_caught() {
    let fixtures = Fixtures::new("permissive-whitespace");
    // A conforming program behind a filter that turns vertical tab, form feed
    // and every non-ASCII byte into a space before it ever sees the input.
    // That is the shape of a real parser bug — one `is_whitespace` where the
    // grammar says space or tab — and it is invisible to every other mode: the
    // program still simulates correctly, still agrees with itself across
    // respellings, and still satisfies every invariant. Only an input that is
    // wrong in exactly one character says otherwise.
    let probe = Path::new(env!("CARGO_BIN_EXE_probe"));
    let implementation = fixtures.implementation(
        "permissive",
        &format!("tr '\\013\\014\\200-\\377' ' ' | {}", probe.display()),
    );

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--properties",
            "0",
            "--rejections",
            "300",
            "--seed",
            "11",
        ],
    );
    assert!(
        report
            .stdout
            .lines()
            .any(|line| line.starts_with("ACCEPTED") && line.contains("a foreign separator")),
        "a program that widens what separates tokens must be caught:\n{}",
        report.stdout
    );
}

#[test]
fn an_implementation_that_reads_stdin_before_its_arguments_is_caught() {
    let fixtures = Fixtures::new("stdin-first");
    // The conforming program behind a wrapper that consults stdin first and
    // falls back to the arguments only when there is nothing there. Every
    // other invocation case is graded with empty stdin, so every one of them
    // passes; the defect is only visible when a flag and a mission arrive
    // together, which no case asked for until now.
    let probe = Path::new(env!("CARGO_BIN_EXE_probe")).display().to_string();
    let implementation = fixtures.implementation(
        "stdin-first",
        &format!(
            "input=$(cat)\n\
             if [ -n \"$input\" ]; then printf '%s\\n' \"$input\" | {probe}; \
             else {probe} \"$@\"; fi"
        ),
    );

    let report = grade(&implementation);
    let why = report
        .failure("a help flag leaves the mission on stdin alone")
        .expect("the case must fail");
    assert!(
        why.contains("stdin was read to the end"),
        "taking a megabyte to decide what --help means is the finding: {why}"
    );
    for case in [
        "--help alone prints usage and exits 0",
        "-h alone prints usage and exits 0",
        "--version alone reports the contract version",
        "an unknown argument is a usage error",
    ] {
        assert!(
            report.passed(case),
            "this defect is invisible to {case:?}, which is why the new case \
             exists:\n{}",
            report.stdout
        );
    }
}

/// Which misreading of §2.2 each semantic case catches.
///
/// Every scent case in the catalogue claims to discriminate, and until
/// something fails it the claim is only an argument. The near-miss gallery
/// supplies a program per plausible misreading; this table says which cases
/// each one fails, and the assertions below run both ways. A case missing from
/// every row is a case no wrong program is known to fail. A defect that fails
/// a row it is not listed on is a case pinning more than it claims.
const NEAR_MISSES: [(&str, &[&str]); 6] = [
    (
        "direction-scent",
        &["a scent blocks a departure by a different edge"],
    ),
    (
        "scent-consumed",
        &["a scent is not used up by the robot it saves"],
    ),
    (
        "scent-at-start",
        &[
            "a scent marks the cell and never the robot",
            "a scent protects a robot that walked onto the cell",
        ],
    ),
    ("stop-on-ignore", &["an ignored move does not end the run"]),
    (
        "one-scent",
        &["scents on different cells each protect their own"],
    ),
    (
        "keep-simulating",
        &["a lost robot does not run the rest of its instructions"],
    ),
];

const SCENT_CASES: [&str; 7] = [
    "a scent blocks a departure by a different edge",
    "a scent is not used up by the robot it saves",
    "a scent marks the cell and never the robot",
    "an ignored move does not end the run",
    "scents on different cells each protect their own",
    "a scent protects a robot that walked onto the cell",
    "a lost robot does not run the rest of its instructions",
];

#[test]
fn every_misreading_of_the_scent_rule_fails_exactly_the_cases_it_should() {
    let fixtures = Fixtures::new("near-misses");
    let gallery = Path::new(env!("CARGO_BIN_EXE_misbehave"))
        .display()
        .to_string();

    for (defect, should_fail) in NEAR_MISSES {
        let implementation =
            fixtures.implementation(defect, &format!("MISBEHAVE={defect} exec {gallery}"));
        let report = grade(&implementation);
        for case in SCENT_CASES {
            let failed = report.failure(case).is_some();
            assert_eq!(
                failed,
                should_fail.contains(&case),
                "{defect}: {case:?} {}\n{}",
                if failed {
                    "failed and should not"
                } else {
                    "passed and should not"
                },
                report.stdout
            );
        }
    }
}

#[test]
fn every_semantic_case_is_failed_by_some_wrong_program() {
    // A case nothing fails is a case that has never been shown to do anything.
    for case in SCENT_CASES {
        assert!(
            NEAR_MISSES.iter().any(|(_, cases)| cases.contains(&case)),
            "no near miss in the gallery fails {case:?}, so nothing shows it \
             discriminates"
        );
    }
}

#[test]
fn the_invariants_are_asked_of_respelled_input_as_well_as_canonical() {
    let fixtures = Fixtures::new("respelled-properties");
    // A class that never runs and a class that finds nothing print the same
    // thing, and a respelling that never gets drawn is exactly that. So this
    // one keeps what it was fed: the conforming program behind a tee.
    let seen = fixtures.directory.join("fed");
    let implementation = fixtures.implementation(
        "recorder",
        &format!(
            "tee -a {} | {}",
            seen.display(),
            Path::new(env!("CARGO_BIN_EXE_probe")).display()
        ),
    );

    let report = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--rejections",
            "0",
            "--differential",
            "0",
            "--properties",
            "20",
            "--seed",
            "17",
        ],
    );
    // Only this mode's verdict is the subject. The wrapper forwards no
    // arguments and drains stdin to record it, so the flag cases have nothing
    // to say here.
    assert!(
        report
            .stdout
            .lines()
            .any(|line| line.starts_with("properties:") && line.ends_with("0 violation(s)")),
        "the conforming program must satisfy every invariant over a \
         respelling:\n{}",
        report.stdout
    );

    let fed = fs::read(&seen).expect("the fixture to have recorded what it was fed");
    assert!(
        fed.contains(&b'\t') || fed.windows(2).any(|pair| pair == b"\r\n"),
        "every input the properties mode drew was canonical, so the respelled \
         half of this mode is not running"
    );
}

#[test]
fn an_implementation_that_answers_only_the_first_few_robots_is_caught() {
    let fixtures = Fixtures::new("first-few");
    // The conforming program, truncated to eight lines. Nothing in the
    // catalogue notices: its longest mission carries three robots, and neither
    // did the generated modes until a drawn mission could hold more than
    // eight. This is what the tail of the population draw is for.
    // Through a file rather than a pipe, so the exit code stays the probe's:
    // in a pipeline it would become head's, and every rejection case would
    // fail for a reason that has nothing to do with this defect.
    let answer = fixtures.directory.join("answer");
    let implementation = fixtures.implementation(
        "first-few",
        &format!(
            "{} \"$@\" > {answer}; code=$?\nhead -8 {answer}\nexit $code",
            Path::new(env!("CARGO_BIN_EXE_probe")).display(),
            answer = answer.display()
        ),
    );

    let report = grade(&implementation);
    assert!(
        report.conformed,
        "every case in the catalogue passes, which is the point:\n{}",
        report.stdout
    );

    let generated = grade_with(
        &implementation,
        &[
            "--spelling",
            "0",
            "--rejections",
            "0",
            "--differential",
            "0",
            "--properties",
            "40",
            "--seed",
            "23",
        ],
    );
    assert!(
        generated.stdout.contains("one line per robot"),
        "a crowd is the only thing that shows this, and it must:\n{}",
        generated.stdout
    );
}

#[test]
fn a_disagreement_is_reported_small_enough_to_read() {
    let fixtures = Fixtures::new("truncating");
    // A wrapper that cuts every line to fifteen characters, which is a real
    // defect shape: a fixed buffer, or a read that stops at a length nobody
    // chose on purpose. It answers short missions correctly, so the
    // disagreement it produces is a long one, and a long one is exactly what
    // nobody can debug.
    let implementation = fixtures.implementation(
        "truncating",
        &format!(
            "awk '{{ if (length($0) > 15) $0 = substr($0, 1, 15); print }}' | {}",
            Path::new(env!("CARGO_BIN_EXE_probe")).display()
        ),
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
            "20",
            "--seed",
            "41",
        ],
    );

    let mut lines = report
        .stdout
        .lines()
        .skip_while(|line| !line.starts_with("DISAGREES"));
    let drawn = lines
        .find(|line| line.trim_start().starts_with("mission:"))
        .expect("a disagreement to be found at all")
        .to_string();
    let reduced_at = lines
        .find(|line| line.trim_start().starts_with("reduced in"))
        .map(ToString::to_string)
        .expect("the first disagreement to be reduced");
    let reduced = lines
        .find(|line| line.trim_start().starts_with("mission:"))
        .expect("the reduction to name a mission")
        .to_string();

    assert!(
        reduced.len() < drawn.len() / 2,
        "a reduction that barely reduces is not worth the processes it costs\n\
         drawn:   {drawn}\n  reduced: {reduced}"
    );
    assert!(
        !reduced_at.contains("budget ran out"),
        "a defect this simple should reduce all the way: {reduced_at}"
    );
    // One robot, one instruction line, and the truncation boundary still in
    // it: the smallest input that can show a sixteenth character going missing.
    assert!(
        reduced.contains("\\n") && reduced.matches("\\n").count() == 3,
        "the reduction should be a grid line, one robot and its instructions: \
         {reduced}"
    );
}

#[test]
fn a_failure_names_an_id_that_runs_it_again() {
    let fixtures = Fixtures::new("selection");
    // Wrong about one thing: a scent that only protects a departure by the
    // edge the earlier robot left through.
    let implementation = fixtures.implementation(
        "direction-scent",
        &format!(
            "MISBEHAVE=direction-scent exec {}",
            Path::new(env!("CARGO_BIN_EXE_misbehave")).display()
        ),
    );

    let report = grade(&implementation);
    let cited = report
        .stdout
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("case: "))
        .expect("a failure to cite a case id")
        .to_string();
    let (id, _) = cited.split_once(' ').expect("the id and how to run it");

    // The citation, used. This is the only claim an id has to make.
    let again = grade_with(&implementation, &["--case", id]);
    assert!(
        !again.conformed,
        "the case the id names must fail on its own:\n{}",
        again.stdout
    );
    assert_eq!(
        again
            .stdout
            .lines()
            .filter(|line| line.starts_with("FAIL"))
            .count(),
        1,
        "one id, one case:\n{}",
        again.stdout
    );

    // A group name selects the group and nothing else. The brief's own sample
    // is in this one, and R9's rationale records that it cannot tell the two
    // scent models apart - so a group that does not ask about corners passes a
    // program that is wrong about them.
    let group = grade_with(&implementation, &["--case", "missions"]);
    assert!(
        group.conformed,
        "nothing in the missions group distinguishes the scent models:\n{}",
        group.stdout
    );
    assert_eq!(
        group
            .stdout
            .lines()
            .filter(|line| line.starts_with("ok"))
            .count(),
        4,
        "a group name must select its group and not the catalogue:\n{}",
        group.stdout
    );
}

#[test]
fn a_selection_that_matches_nothing_is_an_error_and_not_a_clean_run() {
    let fixtures = Fixtures::new("selection-typo");
    let implementation = fixtures.implementation(
        "conforming",
        &format!(
            "exec {} \"$@\"",
            Path::new(env!("CARGO_BIN_EXE_probe")).display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_martian-robots-verify"))
        .arg("--bin")
        .arg(&implementation)
        .args(["--case", "scents"])
        .output()
        .expect("to run the suite");
    assert_eq!(
        output.status.code(),
        Some(2),
        "a mistyped id must be the suite failing to run, not the program passing"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no case id starts with"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_mode_that_judged_nothing_is_the_suite_failing_and_not_the_program_passing() {
    let fixtures = Fixtures::new("vacuous");
    let implementation = fixtures.implementation(
        "conforming",
        &format!(
            "exec {} \"$@\"",
            Path::new(env!("CARGO_BIN_EXE_probe")).display()
        ),
    );

    // Seed 2 with a budget of one builds no mutation at all: the drawn mission
    // has nothing a mutation can break. Before the floor this printed
    // `0 of 1 attempted, 0 failure(s)` and exited 0 — a consumer who turned the
    // budgets down to fit a CI minute got a green run that judged nothing.
    let output = Command::new(env!("CARGO_BIN_EXE_martian-robots-verify"))
        .arg("--bin")
        .arg(&implementation)
        .args([
            "--quiet",
            "--rejections",
            "1",
            "--spelling",
            "0",
            "--properties",
            "0",
            "--differential",
            "0",
            "--seed",
            "2",
        ])
        .output()
        .expect("to run the suite");

    assert_eq!(
        output.status.code(),
        Some(2),
        "a mode that judged nothing must be the suite could-not-run code"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("judged nothing"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_reader_whose_whitespace_came_from_its_language_is_caught() {
    let fixtures = Fixtures::new("narrow-whitespace");
    // Folds every Unicode whitespace character to a space *except* the eight
    // this suite used to inject. So it satisfies every check a hand-picked
    // list could make, and is caught only because the pool is now derived from
    // the Unicode property rather than written out. One `is_whitespace` call
    // is the whole defect, and it is the likeliest real bug in the family.
    let implementation = fixtures.implementation(
        "narrow-whitespace",
        &format!(
            "MISBEHAVE=narrow-whitespace exec {}",
            Path::new(env!("CARGO_BIN_EXE_misbehave")).display()
        ),
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
            "300",
        ],
    );
    assert!(
        report
            .stdout
            .lines()
            .any(|line| line.starts_with("ACCEPTED") && line.contains("a foreign separator")),
        "a character the grammar does not admit was accepted as a separator \
         and nothing said so:\n{}",
        report.stdout
    );
}
