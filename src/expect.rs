use std::fmt::Write as _;

use crate::run::{Ending, Observation};

/// What the contract lets a suite demand of one run
#[derive(Debug)]
pub enum Expect {
    Output(Vec<u8>),
    Rejection(Diagnostic),
    Help,
    UsageError,
    /// The version flag alone. Only that the version appears is pinned; the
    /// text around it belongs to the implementation, as help's does.
    Version(String),
    /// A flag invocation with a mission on stdin. R20 and R21 both oblige a
    /// flag to leave stdin unread, which has two consequences an outside
    /// observer can see: the input is not taken, and its answer does not
    /// appear. The flag's own promise still has to hold, so it is judged by
    /// whichever expectation makes it.
    StdinUnread {
        promise: Box<Expect>,
        answer: Vec<u8>,
    },
}

/// More bytes than any pipe will buffer.
///
/// The drain check only says anything about a payload this size: a small one
/// is handed over in full whether the program reads it or not, because the
/// kernel accepts it on the program's behalf.
pub const MORE_THAN_A_PIPE_HOLDS: usize = 1 << 20;

#[derive(Debug, Default)]
pub struct Diagnostic {
    /// Every input line the diagnostic must name. §2.5 fixes the form as
    /// `line N`, so a number is matched as a whole: `line 19` does not answer
    /// a demand for `line 1`.
    pub lines: Vec<usize>,
    /// Rulings the diagnostic must cite, as groups. Within a group any one
    /// tag will do, because §2.5 asks for a governing ruling rather than a
    /// particular one. Every group must be answered, which is how a rejection
    /// carrying two independent violations is held to R25: one tag cannot
    /// stand in for both when the two groups name different rulings.
    pub requires: Vec<Vec<String>>,
    pub forbids_a_line_reference: bool,
}

impl Diagnostic {
    pub fn at_line(line: usize, tags: &[&str]) -> Self {
        Self {
            lines: vec![line],
            requires: vec![tags.iter().map(|tag| format!("({tag})")).collect()],
            forbids_a_line_reference: false,
        }
    }

    pub fn tagged(tags: &[&str]) -> Self {
        Self {
            lines: Vec::new(),
            requires: vec![tags.iter().map(|tag| format!("({tag})")).collect()],
            forbids_a_line_reference: false,
        }
    }

    /// Several independent violations, each on its own line, each governed by
    /// a ruling of its own. R25 asks for every violation found in one pass,
    /// and this is the only shape that asks for more than one.
    pub fn at_lines(demands: &[(usize, &[&str])]) -> Self {
        Self {
            lines: demands.iter().map(|(line, _)| *line).collect(),
            requires: demands
                .iter()
                .map(|(_, tags)| tags.iter().map(|tag| format!("({tag})")).collect())
                .collect(),
            forbids_a_line_reference: false,
        }
    }

    pub fn without_a_line(tags: &[&str]) -> Self {
        Self {
            lines: Vec::new(),
            requires: vec![tags.iter().map(|tag| format!("({tag})")).collect()],
            forbids_a_line_reference: true,
        }
    }
}

impl Expect {
    pub fn judge(&self, seen: &Observation) -> Option<String> {
        match seen.ending {
            Ending::Timeout => {
                return Some("did not terminate (grader policy: Q3)".to_string());
            }
            Ending::Signal => return Some("died on a signal (grader policy: Q3)".to_string()),
            Ending::Code(_) => {}
        }

        match self {
            Self::Output(expected) => {
                if !seen.exited_zero() {
                    return Some(format!("exit {}, expected 0", code_of(seen)));
                }
                if &seen.stdout != expected {
                    return Some(format!(
                        "stdout differs\n      expected: {}\n      got:      {}",
                        show(expected),
                        show(&seen.stdout)
                    ));
                }
                None
            }
            Self::Rejection(diagnostic) => {
                if !seen.stdout.is_empty() {
                    return Some(format!(
                        "rejected input must produce no stdout, got {}",
                        show(&seen.stdout)
                    ));
                }
                if !seen.exited_non_zero() {
                    return Some(format!("exit {}, expected non-zero", code_of(seen)));
                }
                if seen.stderr.is_empty() {
                    return Some("no diagnostic on stderr".to_string());
                }
                diagnostic.judge(&String::from_utf8_lossy(&seen.stderr))
            }
            Self::Help => {
                if !seen.exited_zero() {
                    return Some(format!("exit {}, expected 0", code_of(seen)));
                }
                if seen.stdout.is_empty() {
                    return Some("no usage on stdout".to_string());
                }
                None
            }
            Self::Version(version) => {
                if !seen.exited_zero() {
                    return Some(format!("exit {}, expected 0", code_of(seen)));
                }
                let said = String::from_utf8_lossy(&seen.stdout);
                if !said.contains(version.as_str()) {
                    return Some(format!(
                        "stdout does not report {version}: {}",
                        show(&seen.stdout)
                    ));
                }
                None
            }
            Self::StdinUnread { promise, answer } => {
                if let Some(why) = promise.judge(seen) {
                    return Some(why);
                }
                if seen.drained_stdin {
                    return Some("stdin was read to the end".to_string());
                }
                if !answer.is_empty()
                    && seen
                        .stdout
                        .windows(answer.len())
                        .any(|window| window == answer.as_slice())
                {
                    return Some(format!(
                        "stdin was read and answered: stdout contains {}",
                        show(answer)
                    ));
                }
                None
            }
            Self::UsageError => {
                if !seen.stdout.is_empty() {
                    return Some(format!(
                        "a usage error must produce no stdout, got {}",
                        show(&seen.stdout)
                    ));
                }
                if !seen.exited_non_zero() {
                    return Some(format!("exit {}, expected non-zero", code_of(seen)));
                }
                if seen.stderr.is_empty() {
                    return Some("no usage message on stderr".to_string());
                }
                None
            }
        }
    }
}

impl Diagnostic {
    fn judge(&self, stderr: &str) -> Option<String> {
        let named = line_references(stderr);
        for line in &self.lines {
            if !named.contains(line) {
                return Some(format!(
                    "diagnostic does not name line {line}; it names {named:?}"
                ));
            }
        }
        if self.forbids_a_line_reference && !named.is_empty() {
            return Some(format!(
                "diagnostic names line {named:?}, and no line is attributable"
            ));
        }
        for group in &self.requires {
            if !group.is_empty() && !group.iter().any(|tag| stderr.contains(tag.as_str())) {
                return Some(format!("diagnostic carries none of {group:?}"));
            }
        }
        None
    }
}

/// Every line a diagnostic names, as whole numbers.
///
/// Whole numbers because `line 19` is not an answer to a demand for `line 1`,
/// and a word boundary because "outline 3" names no line. A diagnostic stays
/// free to use the word without naming one: "no grid line" names nothing.
fn line_references(stderr: &str) -> Vec<usize> {
    let mut named = Vec::new();
    let bytes = stderr.as_bytes();
    let mut from = 0;

    while let Some(offset) = stderr[from..].find("line ") {
        let at = from + offset;
        let starts_a_word = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        let after = at + "line ".len();
        let digits: String = stderr[after..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();

        if starts_a_word && !digits.is_empty() {
            let whole = stderr[after + digits.len()..]
                .chars()
                .next()
                .is_none_or(|next| !next.is_ascii_digit());
            if whole && let Ok(line) = digits.parse() {
                named.push(line);
            }
        }
        from = after;
    }

    named
}

fn code_of(seen: &Observation) -> String {
    match seen.ending {
        Ending::Code(code) => code.to_string(),
        Ending::Signal => "signal".to_string(),
        Ending::Timeout => "timeout".to_string(),
    }
}

/// Bytes as a readable string, cut short: a case may carry a megabyte of
/// padding, and a failure nobody can scroll through explains nothing.
const SHOW_AT_MOST: usize = 240;

pub fn show(bytes: &[u8]) -> String {
    let (shown_bytes, elided) = if bytes.len() > SHOW_AT_MOST {
        (&bytes[..SHOW_AT_MOST], bytes.len() - SHOW_AT_MOST)
    } else {
        (bytes, 0)
    };
    let mut shown = String::with_capacity(shown_bytes.len() + 2);
    shown.push('"');
    for &byte in shown_bytes {
        match byte {
            b'\n' => shown.push_str("\\n"),
            b'\r' => shown.push_str("\\r"),
            b'\t' => shown.push_str("\\t"),
            b'"' => shown.push_str("\\\""),
            b'\\' => shown.push_str("\\\\"),
            0x20..=0x7e => shown.push(byte as char),
            other => {
                let _ = write!(shown, "\\x{other:02x}");
            }
        }
    }
    shown.push('"');
    if elided > 0 {
        let _ = write!(shown, " and {elided} more byte(s)");
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, Expect, show};

    use crate::run::{Ending, Observation};

    fn seen(stdout: &[u8], stderr: &[u8], ending: Ending) -> Observation {
        Observation {
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
            ending,
            drained_stdin: false,
        }
    }

    #[test]
    fn success_is_byte_exact_and_ignores_stderr() {
        let expect = Expect::Output(b"1 1 E\n".to_vec());
        assert!(
            expect
                .judge(&seen(b"1 1 E\n", b"chatter", Ending::Code(0)))
                .is_none()
        );
        assert!(
            expect
                .judge(&seen(b"1 1 E", b"", Ending::Code(0)))
                .is_some(),
            "a missing final newline is a different answer"
        );
        assert!(
            expect
                .judge(&seen(b"1 1 E\n", b"", Ending::Code(1)))
                .is_some()
        );
    }

    #[test]
    fn rejection_demands_silence_on_stdout() {
        let expect = Expect::Rejection(Diagnostic::default());
        assert!(
            expect
                .judge(&seen(b"1 1 E\n", b"line 2: bad (R5)", Ending::Code(1)))
                .is_some(),
            "output already printed is output a correct run would never produce"
        );
    }

    #[test]
    fn rejection_demands_a_diagnostic_and_a_non_zero_exit() {
        let expect = Expect::Rejection(Diagnostic::default());
        assert!(expect.judge(&seen(b"", b"", Ending::Code(1))).is_some());
        assert!(
            expect
                .judge(&seen(b"", b"something", Ending::Code(0)))
                .is_some()
        );
        assert!(
            expect
                .judge(&seen(b"", b"something", Ending::Code(2)))
                .is_none(),
            "no ruling names a particular non-zero code"
        );
    }

    #[test]
    fn a_tag_obligation_is_satisfied_by_any_governing_ruling() {
        let expect = Expect::Rejection(Diagnostic::at_line(2, &["R1", "R5"]));
        assert!(
            expect
                .judge(&seen(b"", b"line 2: off the world (R1)", Ending::Code(1)))
                .is_none()
        );
        assert!(
            expect
                .judge(&seen(b"", b"line 2: too big (R5)", Ending::Code(1)))
                .is_none()
        );
        assert!(
            expect
                .judge(&seen(b"", b"line 2: no idea", Ending::Code(1)))
                .is_some()
        );
        assert!(
            expect
                .judge(&seen(b"", b"line 9: off the world (R1)", Ending::Code(1)))
                .is_some()
        );
    }

    #[test]
    fn a_line_number_is_matched_whole() {
        let expect = Expect::Rejection(Diagnostic::at_line(1, &["R5"]));
        assert!(
            expect
                .judge(&seen(b"", b"line 19: too big (R5)", Ending::Code(1)))
                .is_some(),
            "line 19 is not an answer to a demand for line 1"
        );
        assert!(
            expect
                .judge(&seen(b"", b"line 1: too big (R5)", Ending::Code(1)))
                .is_none()
        );
    }

    #[test]
    fn the_word_line_inside_another_word_names_nothing() {
        let expect = Expect::Rejection(Diagnostic::without_a_line(&[]));
        assert!(
            expect
                .judge(&seen(b"", b"outline 3 is malformed", Ending::Code(1)))
                .is_none(),
            "outline 3 names no input line"
        );
    }

    #[test]
    fn a_line_reference_is_forbidden_where_no_line_is_attributable() {
        let expect = Expect::Rejection(Diagnostic::without_a_line(&["R12"]));
        assert!(
            expect
                .judge(&seen(b"", b"no grid line (R12)", Ending::Code(1)))
                .is_none(),
            "a diagnostic may use the word line without naming one"
        );
        assert!(
            expect
                .judge(&seen(b"", b"line 1: no grid line (R12)", Ending::Code(1)))
                .is_some(),
            "a fabricated line number is wrong, not merely unhelpful"
        );
    }

    #[test]
    fn a_version_must_actually_name_the_version() {
        let expect = Expect::Version("2.0.0".to_string());
        assert!(
            expect
                .judge(&seen(
                    b"martian-robots 2.0.0 (contract 2.0.0)\n",
                    b"",
                    Ending::Code(0)
                ))
                .is_none(),
            "the text around the version belongs to the implementation"
        );
        assert!(
            expect
                .judge(&seen(b"1.0.0\n", b"", Ending::Code(0)))
                .is_some(),
            "a different version is a different contract"
        );
        assert!(
            expect
                .judge(&seen(b"2.0.0\n", b"", Ending::Code(1)))
                .is_some()
        );
    }

    #[test]
    fn help_asks_for_output_and_never_for_words() {
        assert!(
            Expect::Help
                .judge(&seen(b"anything at all\n", b"", Ending::Code(0)))
                .is_none()
        );
        assert!(
            Expect::Help
                .judge(&seen(b"", b"", Ending::Code(0)))
                .is_some()
        );
    }

    #[test]
    fn not_terminating_and_dying_are_reported_as_grader_policy() {
        let expect = Expect::Output(b"".to_vec());
        assert!(
            expect
                .judge(&seen(b"", b"", Ending::Timeout))
                .unwrap()
                .contains("Q3")
        );
        assert!(
            expect
                .judge(&seen(b"", b"", Ending::Signal))
                .unwrap()
                .contains("Q3")
        );
    }

    #[test]
    fn invisible_bytes_are_shown() {
        assert_eq!(show(b"a\tb\r\n\xc2\xa0"), r#""a\tb\r\n\xc2\xa0""#);
    }

    fn asked_for_help(stdout: &[u8], drained_stdin: bool) -> Observation {
        Observation {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            ending: Ending::Code(0),
            drained_stdin,
        }
    }

    fn unread() -> Expect {
        Expect::StdinUnread {
            promise: Box::new(Expect::Help),
            answer: b"1 1 E\n".to_vec(),
        }
    }

    #[test]
    fn help_that_leaves_the_input_alone_passes() {
        assert_eq!(
            unread().judge(&asked_for_help(b"usage: ...\n", false)),
            None
        );
    }

    #[test]
    fn taking_the_whole_input_is_caught() {
        let why = unread()
            .judge(&asked_for_help(b"usage: ...\n", true))
            .unwrap();
        assert!(why.contains("read to the end"), "{why}");
    }

    #[test]
    fn answering_part_of_the_input_is_caught_even_when_it_was_not_all_taken() {
        // A program that reads what the pipe already holds, answers it, and
        // exits. The drain check cannot see this one: the rest of the payload
        // was never accepted.
        let why = unread().judge(&asked_for_help(b"1 1 E\n", false)).unwrap();
        assert!(why.contains("read and answered"), "{why}");
    }

    #[test]
    fn a_flag_that_leaves_stdin_alone_still_has_to_keep_its_own_promise() {
        let why = unread().judge(&asked_for_help(b"", false)).unwrap();
        assert!(why.contains("no usage on stdout"), "{why}");
    }
}
