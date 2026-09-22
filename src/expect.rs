use std::fmt::Write as _;

use crate::run::{Ending, Observation};

/// What the contract lets a suite demand of one run, and nothing more.
///
/// The line reference and the ruling tag are contract (§2.5); the sentence
/// around them is implementation-chosen, so a judge that asks for prose fails
/// conforming programs. Stderr on success is Q2 — open — so success ignores
/// it entirely.
#[derive(Debug)]
pub enum Expect {
    /// Valid input: stdout byte-exact, exit 0, stderr unexamined.
    Output(Vec<u8>),
    /// Invalid input: no stdout, a non-zero exit, and a diagnostic that
    /// carries what §2.5 requires of it.
    Rejection(Diagnostic),
    /// A help flag alone: usage on stdout, exit 0. No ruling constrains the
    /// text, so nothing here does either.
    Help,
    /// Any other argument: a usage error on stderr, non-zero exit, no stdout.
    UsageError,
}

/// The three-part obligation §2.5 places on a diagnostic.
#[derive(Debug, Default)]
pub struct Diagnostic {
    /// Substrings that must appear — the `line N` reference, where a physical
    /// line is attributable.
    pub required: Vec<String>,
    /// Tags of which at least one must appear. Rulings overlap, and §2.5 says
    /// one governing tag suffices, so a suite that demands a particular one
    /// invents contract.
    pub any_of: Vec<String>,
    /// §2.5: a violation with no attributable physical line carries no line
    /// reference at all. This is the half that catches a fabricated line
    /// number — and it looks for a reference, not for the word, because a
    /// diagnostic is free to say "grid line" without naming one.
    pub forbids_a_line_reference: bool,
}

impl Diagnostic {
    pub fn at_line(line: usize, tags: &[&str]) -> Self {
        Self {
            required: vec![format!("line {line}")],
            any_of: tags.iter().map(|tag| format!("({tag})")).collect(),
            forbids_a_line_reference: false,
        }
    }

    /// A violation with no attributable line: §2.5 says it carries no line
    /// reference, so naming one is wrong rather than merely unhelpful.
    pub fn without_a_line(tags: &[&str]) -> Self {
        Self {
            required: Vec::new(),
            any_of: tags.iter().map(|tag| format!("({tag})")).collect(),
            forbids_a_line_reference: true,
        }
    }
}

impl Expect {
    /// `None` when the observation satisfies the expectation, otherwise why
    /// it did not — phrased for someone reading a CI log.
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
        for needle in &self.required {
            if !stderr.contains(needle.as_str()) {
                return Some(format!("diagnostic does not name {needle:?}"));
            }
        }
        if self.forbids_a_line_reference
            && let Some(reference) = line_reference(stderr)
        {
            return Some(format!(
                "diagnostic names {reference:?}, and no line is attributable"
            ));
        }
        if !self.any_of.is_empty() && !self.any_of.iter().any(|tag| stderr.contains(tag.as_str())) {
            return Some(format!("diagnostic carries none of {:?}", self.any_of));
        }
        None
    }
}

/// The first `line N` in a diagnostic, if it carries one. "grid line" is not
/// a reference; "line 1" is.
fn line_reference(stderr: &str) -> Option<String> {
    let mut rest = stderr;
    while let Some(at) = rest.find("line ") {
        let after = &rest[at + "line ".len()..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            return Some(format!("line {digits}"));
        }
        rest = after;
    }
    None
}

fn code_of(seen: &Observation) -> String {
    match seen.ending {
        Ending::Code(code) => code.to_string(),
        Ending::Signal => "signal".to_string(),
        Ending::Timeout => "timeout".to_string(),
    }
}

/// Bytes as a reader can see them: invisible characters are the whole subject
/// of several rulings, and a CI log that prints them raw shows nothing.
pub fn show(bytes: &[u8]) -> String {
    let mut shown = String::with_capacity(bytes.len() + 2);
    shown.push('"');
    for &byte in bytes {
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
}
