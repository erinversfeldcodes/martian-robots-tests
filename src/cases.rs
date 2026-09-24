use std::ffi::OsString;

use crate::contract::{Contract, SAMPLE_INPUT, SAMPLE_OUTPUT};
use crate::expect::{Diagnostic, Expect};

pub struct Case {
    pub name: String,
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
            stdin: Vec::new(),
            expect,
        });
    }
}

pub fn catalogue(contract: &Contract) -> Vec<Case> {
    let mut build = Builder { cases: Vec::new() };
    missions(&mut build, contract);
    scent(&mut build);
    whitespace(&mut build);
    vocabulary(&mut build);
    framing(&mut build);
    diagnostics(&mut build, contract);
    bytes(&mut build);
    boundaries(&mut build, contract);
    invocation(&mut build);
    build.cases
}

fn scent(build: &mut Builder) {
    build.case(
        "a scent blocks a departure by a different edge",
        &["R9"],
        "the discriminator the brief's own sample cannot be: robot one is lost \
         north from a corner, robot two tries to leave the same cell going \
         east. Cell-based scent saves it; direction-based scent does not, and \
         passes every other case in this suite",
        "1 1\n1 1 N\nF\n1 1 E\nF\n",
        Expect::Output(b"1 1 N LOST\n1 1 E\n".to_vec()),
    );

    build.case(
        "a scent is not used up by the robot it saves",
        &["R10"],
        "two later robots attempt the same fatal move from the same cell; the \
         cell is scented once and protects both",
        "1 1\n1 1 N\nF\n1 1 N\nF\n1 1 N\nF\n",
        Expect::Output(b"1 1 N LOST\n1 1 N\n1 1 N\n".to_vec()),
    );

    build.case(
        "a scent marks the cell and never the robot",
        &["R23"],
        "robot two starts on the scented cell, walks off it unharmed, and is \
         still lost at an edge that carries no scent. Protection is occupancy, \
         not a property the robot keeps",
        "2 0\n0 0 W\nF\n0 0 E\nFFF\n",
        Expect::Output(b"0 0 W LOST\n2 0 E LOST\n".to_vec()),
    );
}

fn whitespace(build: &mut Builder) {
    build.case(
        "every legal spelling of the separators is accepted",
        &["R4"],
        "tabs, runs, mixed runs, and whitespace at both edges of every line - \
         the positive control without which the rejections below would be \
         satisfied by a program that refuses all unusual whitespace",
        "\t5  \t3 \n \t1\t1\tE  \n RFRFRFRF \t\n",
        Expect::Output(b"1 1 E\n".to_vec()),
    );

    build.case(
        "a form feed is not a separator",
        &["R4", "R12"],
        "R4 names spaces and tabs and nothing else, but almost every runtime's \
         default whitespace split accepts this. One character from a valid \
         mission; replacing it with a space restores one",
        "5\x0c3\n1 1 E\nRFRFRFRF\n",
        Expect::Rejection(Diagnostic::at_line(1, &["R4", "R12"])),
    );

    build.case(
        "a no-break space inside a legal run is not a separator",
        &["R4", "R12"],
        "hidden inside spaces a trim-and-split parser would accept, which is \
         where such a parser blames the legal space beside it",
        "5 3\n1\u{a0} 1 E\nRFRFRFRF\n",
        Expect::Rejection(Diagnostic::at_line(2, &["R4", "R12"])),
    );

    build.case(
        "a vertical tab at the end of an instruction line is not whitespace",
        &["R4", "R12", "R7"],
        "the trailing edge, where `ows` makes real whitespace invisible. \
         Q6 leaves open whether this reads as a bad separator or an unknown \
         instruction, so either ruling satisfies the diagnostic",
        "5 3\n1 1 E\nRFRFRFRF\x0b\n",
        Expect::Rejection(Diagnostic::at_line(3, &["R4", "R12", "R7"])),
    );
}

fn vocabulary(build: &mut Builder) {
    build.case(
        "an orientation of the right token count and the wrong width",
        &["R12", "R7"],
        "three tokens, so a parser that counts tokens and reads the first \
         character of the third accepts it. Q5 leaves open which ruling owns \
         this, so either satisfies the diagnostic",
        "5 3\n1 1 EE\nRFRFRFRF\n",
        Expect::Rejection(Diagnostic::at_line(2, &["R12", "R7"])),
    );

    build.case(
        "a coordinate with a letter glued to it",
        &["R12"],
        "an atoi-style parser reads the digits and discards the tail, which is \
         how a plausible wrong acceptance happens",
        "5x 3\n1 1 E\nRFRFRFRF\n",
        Expect::Rejection(Diagnostic::at_line(1, &["R12"])),
    );

    build.case(
        "an extra token on a position line",
        &["R12"],
        "the line must match the grammar exactly once whitespace is normalised",
        "5 3\n1 1 E X\nRFRFRFRF\n",
        Expect::Rejection(Diagnostic::at_line(2, &["R12"])),
    );

    build.case(
        "a lowercase orientation is not an orientation",
        &["R7", "R12"],
        "the vocabulary is strict-uppercase until a future command type says \
         otherwise",
        "5 3\n1 1 e\nRFRFRFRF\n",
        Expect::Rejection(Diagnostic::at_line(2, &["R7", "R12"])),
    );

    build.case(
        "a letter outside the instruction vocabulary",
        &["R7", "R12"],
        "one character that is not L, R or F",
        "5 3\n1 1 E\nRFXRF\n",
        Expect::Rejection(Diagnostic::at_line(3, &["R7", "R12"])),
    );

    build.case(
        "leading zeros are a spelling, not a different number",
        &["R16"],
        "accepted on input, and never echoed back: output numbers are canonical",
        "05 03\n01 01 E\nRFRFRFRF\n",
        Expect::Output(b"1 1 E\n".to_vec()),
    );
}

fn framing(build: &mut Builder) {
    build.case(
        "blank and whitespace-only lines before the grid line are ignored",
        &["R17", "R15"],
        "a line of only whitespace counts as blank, and blanks are admitted \
         ahead of the grid line as well as between blocks",
        "   \n\n5 3\n1 1 E\nRFRFRFRF\n",
        Expect::Output(b"1 1 E\n".to_vec()),
    );

    build.case(
        "an unterminated final line is a line",
        &["R14"],
        "end of input acts as an implicit end-of-line",
        "5 3\n1 1 E\nRFRFRFRF",
        Expect::Output(b"1 1 E\n".to_vec()),
    );

    build.case(
        "carriage returns are accepted on input and never emitted",
        &["R11"],
        "CRLF in, LF out",
        "5 3\r\n1 1 E\r\nRFRFRFRF\r\n",
        Expect::Output(b"1 1 E\n".to_vec()),
    );

    build.case(
        "a bare carriage return completes a non-empty final line",
        &["R19"],
        "half a CRLF, completed by the implicit end-of-line",
        "5 3\n1 1 E\nRFRFRFRF\r",
        Expect::Output(b"1 1 E\n".to_vec()),
    );

    build.case(
        "a position line with no instruction line after it is refused",
        &["R13", "R18"],
        "the implicit end-of-line does not manufacture the blank line that \
         would make this a robot with no instructions - R18 is what keeps R13 \
         standing, and the diagnostic anchors to the position line",
        "5 3\n1 1 E\n",
        Expect::Rejection(Diagnostic::at_line(2, &["R13"])),
    );

    build.case(
        "a lone carriage return may not manufacture an instruction line",
        &["R19"],
        "the guard: without it one invisible byte turns this rejection into an \
         accepted mission. Which line carries the defect is contestable, so \
         only the ruling is required",
        "5 3\n1 1 E\n\r",
        Expect::Rejection(Diagnostic::tagged(&["R19", "R13"])),
    );

    build.case(
        "a carriage return that is not part of a line ending is refused",
        &["R19", "R11"],
        "a CR not followed by LF, in the middle of the input",
        "5 3\r1 1 E\nRFRFRFRF\n",
        Expect::Rejection(Diagnostic::tagged(&["R19", "R11", "R12"])),
    );
}

fn diagnostics(build: &mut Builder, contract: &Contract) {
    let over = contract.limits.max_coordinate + 1;
    build.case(
        "independent problems in different blocks are all reported",
        &["R25"],
        "one pass, not one diagnostic: each line that breaks a rule on its own \
         is diagnosed, whatever is wrong elsewhere. A program that stops at \
         its first problem is the only thing this case catches",
        format!("5 3\n1 1 e\n\n\n{over} 1 E\n\n"),
        Expect::Rejection(Diagnostic {
            required: vec!["line 2".to_string(), "line 5".to_string()],
            any_of: Vec::new(),
            forbids_a_line_reference: false,
        }),
    );
}

fn bytes(build: &mut Builder) {
    build.case(
        "a byte that cannot begin a character is not contract input",
        &["R22"],
        "a lone continuation byte on an instruction line. The contract's text \
         is ASCII, so this is invalid input rather than a decoding problem to \
         paper over",
        b"5 3\n1 1 E\nRF\x80\n".to_vec(),
        Expect::Rejection(Diagnostic::at_line(3, &["R22"])),
    );

    build.case(
        "a truncated character at end of input is not contract input",
        &["R22"],
        "the first byte of a multi-byte sequence with nothing following it, \
         which a lossy decoder silently replaces",
        b"5 3\n1 1 E\nRFRFRFRF\xc3".to_vec(),
        Expect::Rejection(Diagnostic::tagged(&["R22"])),
    );
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
