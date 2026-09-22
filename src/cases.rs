use std::ffi::OsString;

use crate::contract::{Contract, SAMPLE_INPUT, SAMPLE_OUTPUT};
use crate::expect::{Diagnostic, Expect};

pub struct Case {
    pub name: String,
    /// The ruling ids this case pins. Empty where the contract's prose
    /// governs and no numbered ruling does; `note` says which prose.
    pub enforces: Vec<String>,
    pub note: String,
    pub arguments: Vec<OsString>,
    pub stdin: Vec<u8>,
    pub expect: Expect,
}

struct Builder {
    cases: Vec<Case>,
}

impl Builder {
    fn case(
        &mut self,
        name: &str,
        enforces: &[&str],
        note: &str,
        stdin: impl Into<Vec<u8>>,
        expect: Expect,
    ) {
        self.cases.push(Case {
            name: name.to_string(),
            enforces: enforces.iter().map(ToString::to_string).collect(),
            note: note.to_string(),
            arguments: Vec::new(),
            stdin: stdin.into(),
            expect,
        });
    }

    fn invocation(
        &mut self,
        name: &str,
        enforces: &[&str],
        note: &str,
        args: &[&str],
        expect: Expect,
    ) {
        self.cases.push(Case {
            name: name.to_string(),
            enforces: enforces.iter().map(ToString::to_string).collect(),
            note: note.to_string(),
            arguments: args.iter().map(OsString::from).collect(),
            // Closed immediately for every invocation case. "Reads no stdin"
            // is not observable from out here; feeding EOF at least means a
            // program that wrongly reads it fails on output rather than
            // hanging until the deadline.
            stdin: Vec::new(),
            expect,
        });
    }
}

/// The catalogue. Numbers come from the contract, never from here: a case
/// that spells out a limit is a second copy of it that can go stale.
pub fn catalogue(contract: &Contract) -> Vec<Case> {
    let mut build = Builder { cases: Vec::new() };
    missions(&mut build, contract);
    boundaries(&mut build, contract);
    invocation(&mut build);
    build.cases
}

fn missions(build: &mut Builder, _contract: &Contract) {
    build.case(
        "the brief's sample, byte for byte",
        &[],
        "§2.3: the sample is normative. It pins no ruling on its own - R9's \
         rationale records that it cannot even distinguish the two scent models",
        SAMPLE_INPUT,
        Expect::Output(SAMPLE_OUTPUT.to_vec()),
    );

    build.case(
        "a robot with no instructions reports where it started",
        &["R2"],
        "the line after a position line is that robot's instruction line even \
         when blank, which is what makes zero instructions expressible",
        "5 3\n1 1 E\n\n",
        Expect::Output(b"1 1 E\n".to_vec()),
    );

    build.case(
        "a grid line alone is a mission with nothing to do",
        &["R8"],
        "no robots at all: empty stdout, and a success rather than a rejection",
        "5 3\n",
        Expect::Output(Vec::new()),
    );

    build.case(
        "the smallest world is one cell",
        &["R3"],
        "`0 0` is a world, not a degenerate case to reject",
        "0 0\n0 0 N\n\n",
        Expect::Output(b"0 0 N\n".to_vec()),
    );
}

fn boundaries(build: &mut Builder, contract: &Contract) {
    let max = contract.limits.max_coordinate;
    let instructions = contract.limits.max_instructions;

    build.case(
        "empty input is refused without inventing a line to blame",
        &[],
        "§2.5: a violation with no attributable physical line carries no line \
         reference, so naming one is wrong rather than merely unhelpful. The \
         missing grid line is the contract's own example",
        "",
        Expect::Rejection(Diagnostic::without_a_line(&[])),
    );

    build.case(
        "a coordinate at the maximum is accepted",
        &["R5"],
        "the limit is inclusive: the boundary value itself is valid input",
        format!("{max} {max}\n{max} {max} N\n\n"),
        Expect::Output(format!("{max} {max} N\n").into_bytes()),
    );

    build.case(
        "a grid coordinate past the maximum is refused",
        &["R5"],
        "one past the boundary, on the grid line",
        format!("{} 3\n", max + 1),
        Expect::Rejection(Diagnostic::at_line(1, &["R5"])),
    );

    build.case(
        "a robot coordinate past the maximum is refused",
        &["R5"],
        "one past the boundary, on a position line. A coordinate over the \
         limit is necessarily off the world too, so R1 governs as well and \
         §2.5 lets a diagnostic cite either",
        format!("5 3\n{} 1 E\n\n", max + 1),
        Expect::Rejection(Diagnostic::at_line(2, &["R5", "R1"])),
    );

    build.case(
        "a start off the world is refused even when it is under the maximum",
        &["R1"],
        "isolates R1 from R5: the coordinate is legal, the position is not",
        "5 3\n6 1 E\n\n",
        Expect::Rejection(Diagnostic::at_line(2, &["R1"])),
    );

    build.case(
        "an instruction string at the maximum length is accepted",
        &["R6"],
        "the longest legal string, of turns only, so the answer depends on no \
         movement rule",
        format!("5 3\n1 1 N\n{}\n", "L".repeat(instructions as usize)),
        Expect::Output(format!("1 1 {}\n", after_left_turns(instructions)).into_bytes()),
    );

    build.case(
        "an instruction string past the maximum length is refused",
        &["R6"],
        "one past the boundary",
        format!("5 3\n1 1 N\n{}\n", "L".repeat(instructions as usize + 1)),
        Expect::Rejection(Diagnostic::at_line(3, &["R6"])),
    );

    build.case(
        "nothing is printed for the robots before a bad one",
        &["R5"],
        "§2.4 is all-or-nothing, and the only way to see it is to put the \
         defect in a later block: a program that prints each robot as it goes \
         has already produced output a correct run never would",
        format!("5 3\n1 1 E\n\n\n1 1 E\n\n\n{} 1 E\n\n", max + 1),
        Expect::Rejection(Diagnostic::at_line(8, &["R5", "R1"])),
    );
}

fn invocation(build: &mut Builder) {
    build.invocation(
        "--help alone prints usage and exits 0",
        &["R20"],
        "no ruling constrains the text, so nothing is asked of it but existence",
        &["--help"],
        Expect::Help,
    );

    build.invocation(
        "-h alone prints usage and exits 0",
        &["R20"],
        "the short spelling is the same promise",
        &["-h"],
        Expect::Help,
    );

    build.invocation(
        "an unknown argument is a usage error",
        &["R21"],
        "stderr, non-zero, and nothing on stdout",
        &["--simulate"],
        Expect::UsageError,
    );

    build.invocation(
        "an empty argument is still an argument",
        &["R21"],
        "the empty string is not the absence of an argument",
        &[""],
        Expect::UsageError,
    );

    build.invocation(
        "a help flag with another argument is a usage error",
        &["R24"],
        "a help flag prints help only when it is the sole argument",
        &["--help", "extra"],
        Expect::UsageError,
    );

    build.invocation(
        "a help flag after another argument is a usage error",
        &["R24"],
        "the same in the other order, which an argument loop that returns on \
         the first help flag it sees would pass",
        &["extra", "-h"],
        Expect::UsageError,
    );
}

/// Where a robot facing north ends up after this many left turns. Derived
/// rather than written down, so the case follows the contract's limit.
fn after_left_turns(turns: u32) -> &'static str {
    ["N", "W", "S", "E"][(turns % 4) as usize]
}

#[cfg(test)]
mod tests {
    use super::{after_left_turns, catalogue};
    use crate::contract::{Contract, Decision};

    #[test]
    fn every_cited_ruling_exists_and_is_ruled() {
        let contract = Contract::load().unwrap();
        for case in catalogue(&contract) {
            for id in &case.enforces {
                let ruling = contract
                    .rulings
                    .iter()
                    .find(|ruling| &ruling.id == id)
                    .unwrap_or_else(|| {
                        panic!("{} cites {id}, which is not in the contract", case.name)
                    });
                assert!(
                    matches!(ruling.decision, Decision::Ruled { .. }),
                    "{} cites {id}, which is an open question and may not be tested",
                    case.name
                );
            }
        }
    }

    #[test]
    fn a_case_that_cites_nothing_says_why() {
        let contract = Contract::load().unwrap();
        for case in catalogue(&contract) {
            if case.enforces.is_empty() {
                assert!(
                    !case.note.is_empty(),
                    "{} pins no ruling and gives no reason to exist",
                    case.name
                );
            }
        }
    }

    #[test]
    fn left_turns_come_back_round() {
        assert_eq!(after_left_turns(0), "N");
        assert_eq!(after_left_turns(1), "W");
        assert_eq!(after_left_turns(4), "N");
        assert_eq!(after_left_turns(99), "E");
    }
}
