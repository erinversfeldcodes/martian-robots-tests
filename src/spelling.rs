//! Ways of writing one mission down, all of which the contract says mean the
//! same thing.
//!
//! The spellings vary line by line rather than once per input, so a single
//! rendering can carry a tab-indented grid line, a double-spaced position line
//! and a trailing-whitespace instruction line at once. A parser that
//! normalises inconsistently — right on canonical input, wrong when the same
//! mission is written another way — passes every hand-written case and fails
//! here.
//!
//! Two shapes are excluded on purpose, and the exclusions are checked on the
//! bytes rather than trusted from the code that emits them: mixing LF and CRLF
//! within one input is Q1, and an unterminated final line of only whitespace
//! is Q4. Both are open questions, and a generator that emitted them would be
//! testing something the contract has not decided.

use crate::mission::Mission;
use crate::rng::Rng;

const RUNS: [&str; 5] = [" ", "  ", "\t", " \t", "\t "];
const EDGES: [&str; 4] = ["", " ", "\t", "  "];

pub struct Spelling {
    crlf: bool,
    omit_final_eol: bool,
    blanks_before_grid: u32,
    /// Drawn per line, and cycled, so one rendering carries several.
    runs: Vec<&'static str>,
    leading: Vec<&'static str>,
    trailing: Vec<&'static str>,
    zeros: Vec<usize>,
    blanks_after_block: Vec<u32>,
    /// Blank separator lines are sometimes whitespace rather than nothing,
    /// which R15 says is the same thing.
    blank_is_whitespace: bool,
}

impl Spelling {
    /// The spelling with nothing optional in it. A test fixture: the mode
    /// itself uses `Mission::canonical`, and the two are asserted equal.
    #[cfg(test)]
    pub fn plain() -> Self {
        Self {
            crlf: false,
            omit_final_eol: false,
            blanks_before_grid: 0,
            runs: vec![" "],
            leading: vec![""],
            trailing: vec![""],
            zeros: vec![0],
            blanks_after_block: vec![0],
            blank_is_whitespace: false,
        }
    }

    pub fn draw(rng: &mut Rng) -> Self {
        let many = |rng: &mut Rng, pool: &[&'static str]| -> Vec<&'static str> {
            (0..4).map(|_| *rng.pick(pool)).collect()
        };
        Self {
            crlf: rng.chance(2),
            omit_final_eol: rng.chance(3),
            blanks_before_grid: rng.below(3),
            runs: many(rng, &RUNS),
            leading: many(rng, &EDGES),
            trailing: many(rng, &EDGES),
            zeros: (0..4).map(|_| rng.below(3) as usize).collect(),
            blanks_after_block: (0..4).map(|_| rng.below(3)).collect(),
            blank_is_whitespace: rng.chance(2),
        }
    }

    fn run(&self, line: usize) -> &str {
        self.runs[line % self.runs.len()]
    }

    fn number(&self, line: usize, value: u32) -> String {
        format!("{}{value}", "0".repeat(self.zeros[line % self.zeros.len()]))
    }

    fn wrap(&self, line: usize, content: &str) -> String {
        format!(
            "{}{content}{}",
            self.leading[line % self.leading.len()],
            self.trailing[line % self.trailing.len()]
        )
    }
}

/// Write a mission down in one of the ways the contract permits.
pub fn render(mission: &Mission, spelling: &Spelling) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let blank = if spelling.blank_is_whitespace {
        "  "
    } else {
        ""
    };

    for _ in 0..spelling.blanks_before_grid {
        lines.push(blank.to_string());
    }

    let mut at = 0;
    lines.push(spelling.wrap(
        at,
        &format!(
            "{}{}{}",
            spelling.number(at, mission.max_x),
            spelling.run(at),
            spelling.number(at, mission.max_y)
        ),
    ));
    at += 1;

    for (index, robot) in mission.robots.iter().enumerate() {
        lines.push(spelling.wrap(
            at,
            &format!(
                "{}{}{}{}{}",
                spelling.number(at, robot.x),
                spelling.run(at),
                spelling.number(at, robot.y),
                spelling.run(at),
                robot.facing
            ),
        ));
        at += 1;
        lines.push(spelling.wrap(at, &robot.instructions));
        at += 1;

        // Blanks separate blocks. None after the last, because a trailing
        // blank would be the only line an omitted final end-of-line could
        // land on, which is Q4.
        if index + 1 < mission.robots.len() {
            for _ in 0..spelling.blanks_after_block[index % spelling.blanks_after_block.len()] {
                lines.push(blank.to_string());
            }
        }
    }

    let ending = if spelling.crlf { "\r\n" } else { "\n" };
    let mut rendered = String::new();
    for (index, line) in lines.iter().enumerate() {
        rendered.push_str(line);
        let last = index + 1 == lines.len();
        // The final end-of-line is omitted only when the line it would
        // terminate carries something other than whitespace: R14 admits a
        // non-empty unterminated line, and Q4 leaves a whitespace-only one
        // open.
        if !(last && spelling.omit_final_eol && !line.trim_matches([' ', '\t']).is_empty()) {
            rendered.push_str(ending);
        }
    }
    rendered.into_bytes()
}

/// What a rendering must never be, checked on the bytes themselves.
pub fn permitted(rendered: &[u8]) -> Result<(), String> {
    let mut crlf = false;
    let mut lf = false;
    let mut index = 0;
    while index < rendered.len() {
        match rendered[index] {
            b'\r' => {
                if rendered.get(index + 1) != Some(&b'\n') {
                    return Err("a carriage return that is not part of a line ending".to_string());
                }
                crlf = true;
                index += 2;
            }
            b'\n' => {
                lf = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
    if crlf && lf {
        return Err("LF and CRLF in one input, which is Q1".to_string());
    }

    let ends_with_a_line_ending = rendered.last() == Some(&b'\n');
    if !ends_with_a_line_ending {
        let tail = rendered
            .rsplit(|&byte| byte == b'\n')
            .next()
            .unwrap_or_default();
        if tail.iter().all(|byte| matches!(byte, b' ' | b'\t' | b'\r')) {
            return Err("an unterminated final line of only whitespace, which is Q4".to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Spelling, permitted, render};
    use crate::mission::Mission;
    use crate::rng::Rng;

    #[test]
    fn the_plain_spelling_is_the_canonical_one() {
        let mut rng = Rng::from_seed(5);
        for _ in 0..200 {
            let mission = Mission::draw(&mut rng, 50);
            assert_eq!(render(&mission, &Spelling::plain()), mission.canonical());
        }
    }

    #[test]
    fn every_drawn_spelling_says_the_same_mission() {
        let mut rng = Rng::from_seed(20_260_922);
        for _ in 0..400 {
            let mission = Mission::draw(&mut rng, 50);
            for _ in 0..8 {
                let rendered = render(&mission, &Spelling::draw(&mut rng));
                assert_eq!(
                    Mission::read_back(&rendered).as_ref(),
                    Ok(&mission),
                    "rendering changed the mission: {:?}",
                    String::from_utf8_lossy(&rendered)
                );
            }
        }
    }

    #[test]
    fn no_drawn_spelling_strays_into_an_open_question() {
        let mut rng = Rng::from_seed(99);
        for _ in 0..400 {
            let mission = Mission::draw(&mut rng, 50);
            for _ in 0..8 {
                let rendered = render(&mission, &Spelling::draw(&mut rng));
                permitted(&rendered).unwrap_or_else(|why| {
                    panic!("{why}: {:?}", String::from_utf8_lossy(&rendered))
                });
            }
        }
    }

    #[test]
    fn the_exclusions_are_real_checks() {
        assert!(permitted(b"5 3\r\n1 1 E\nRF\n").is_err(), "mixed endings");
        assert!(permitted(b"5 3\rx\n").is_err(), "a stray carriage return");
        assert!(permitted(b"5 3\n1 1 E\nRF\n   ").is_err(), "Q4's shape");
        assert!(permitted(b"5 3\n1 1 E\nRF").is_ok());
        assert!(permitted(b"5 3\r\n1 1 E\r\nRF\r\n").is_ok());
    }
}
