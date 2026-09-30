use std::fmt::Write as _;

use crate::contract::Contract;
use crate::rng::Rng;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mission {
    pub max_x: u32,
    pub max_y: u32,
    pub robots: Vec<Robot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Robot {
    pub x: u32,
    pub y: u32,
    pub facing: char,
    pub instructions: String,
}

impl Mission {
    pub fn canonical(&self) -> Vec<u8> {
        let mut text = format!("{} {}\n", self.max_x, self.max_y);
        for robot in &self.robots {
            let _ = writeln!(
                text,
                "{} {} {}\n{}",
                robot.x, robot.y, robot.facing, robot.instructions
            );
        }
        text.into_bytes()
    }

    pub fn read_back(rendered: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(rendered).map_err(|error| error.to_string())?;
        // R19's guard, which this reader used to miss: a final line that is
        // nothing but a carriage return is not a line, and treating it as the
        // empty tail a real newline leaves behind made `...\n\r` read as
        // `...\n`. A trailing CR after actual content is a different thing -
        // half a CRLF the implicit ending completes - and stays valid.
        if text.rsplit('\n').next() == Some("\r") {
            return Err("a carriage return that ends nothing".to_string());
        }
        let lines: Vec<&str> = text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect();
        let lines = match lines.split_last() {
            Some((&"", rest)) => rest,
            _ => &lines[..],
        };

        let blank = |line: &str| line.trim_matches([' ', '\t']).is_empty();
        let mut at = 0;
        while at < lines.len() && blank(lines[at]) {
            at += 1;
        }
        let grid = lines.get(at).ok_or("no grid line")?;
        at += 1;
        let (max_x, max_y) = match tokens(grid)[..] {
            [x, y] => (number(x)?, number(y)?),
            _ => return Err(format!("grid line is not two numbers: {grid:?}")),
        };

        let mut robots = Vec::new();
        loop {
            while at < lines.len() && blank(lines[at]) {
                at += 1;
            }
            if at >= lines.len() {
                break;
            }
            let position = lines[at];
            let instructions = lines.get(at + 1).ok_or("no instruction line")?;
            at += 2;
            let (x, y, facing) = match tokens(position)[..] {
                [x, y, facing] if facing.chars().count() == 1 => (
                    number(x)?,
                    number(y)?,
                    facing.chars().next().expect("one character"),
                ),
                _ => return Err(format!("position line is malformed: {position:?}")),
            };
            robots.push(Robot {
                x,
                y,
                facing,
                instructions: instructions.trim_matches([' ', '\t']).to_string(),
            });
        }

        Ok(Self {
            max_x,
            max_y,
            robots,
        })
    }

    #[must_use]
    pub fn with_another_robot(&self, rng: &mut Rng, contract: &Contract) -> Self {
        let mut longer = self.clone();
        let mut robot = Robot::draw_on(rng, self.max_x, self.max_y, contract);
        robot.shape(rng, contract);
        longer.robots.push(robot);
        longer
    }

    pub fn every_robot_starts_on_the_grid(&self) -> bool {
        self.robots
            .iter()
            .all(|robot| robot.x <= self.max_x && robot.y <= self.max_y)
    }

    pub fn is_valid(&self, max_coordinate: u32, max_instructions: u32) -> bool {
        self.max_x <= max_coordinate
            && self.max_y <= max_coordinate
            && self.every_robot_starts_on_the_grid()
            && self.robots.iter().all(|robot| {
                matches!(robot.facing, 'N' | 'E' | 'S' | 'W')
                    && robot.instructions.len() <= max_instructions as usize
                    && robot
                        .instructions
                        .chars()
                        .all(|step| matches!(step, 'L' | 'R' | 'F'))
            })
    }

    pub fn draw_busy(rng: &mut Rng, contract: &Contract) -> Self {
        let max_x = axis(rng, contract.limits.max_coordinate);
        let max_y = axis(rng, contract.limits.max_coordinate);
        let robots = (0..=population(rng))
            .map(|_| {
                let mut robot = Robot::draw_on(rng, max_x, max_y, contract);
                let length = steps(rng, contract.limits.max_instructions);
                robot.instructions = (0..length)
                    .map(|_| *rng.pick(&contract.grammar.instructions))
                    .collect();
                robot
            })
            .collect();
        Self {
            max_x,
            max_y,
            robots,
        }
    }

    pub fn draw_shaped(rng: &mut Rng, contract: &Contract) -> Self {
        let mut mission = Self::draw(rng, contract);
        for robot in &mut mission.robots {
            robot.shape(rng, contract);
        }
        mission
    }

    pub fn draw(rng: &mut Rng, contract: &Contract) -> Self {
        let max_x = axis(rng, contract.limits.max_coordinate);
        let max_y = axis(rng, contract.limits.max_coordinate);
        let robots = (0..population(rng))
            .map(|_| Robot::draw_on(rng, max_x, max_y, contract))
            .collect();
        Self {
            max_x,
            max_y,
            robots,
        }
    }
}

impl Robot {
    fn draw_on(rng: &mut Rng, max_x: u32, max_y: u32, contract: &Contract) -> Self {
        let length = steps(rng, contract.limits.max_instructions);
        Self {
            x: rng.below(max_x + 1),
            y: rng.below(max_y + 1),
            // The letters come from the grammar: a vocabulary written down
            // here would be a second copy of one the contract already states.
            facing: *rng.pick(&contract.grammar.orientations),
            instructions: (0..length)
                .map(|_| *rng.pick(&contract.grammar.instructions))
                .collect(),
        }
    }

    fn shape(&mut self, rng: &mut Rng, contract: &Contract) {
        let length = self.instructions.len();
        // Forward is the instruction that moves, so it is the one the
        // degenerate shapes are built from; the rest turn.
        let forward = 'F';
        let turns: Vec<char> = contract
            .grammar
            .instructions
            .iter()
            .copied()
            .filter(|&step| step != forward)
            .collect();
        self.instructions = match rng.below(4) {
            0 => forward.to_string().repeat(length),
            1 => (0..length).map(|_| *rng.pick(&turns)).collect(),
            2 => String::new(),
            _ => self.instructions.clone(),
        };
    }
}

/// A grid axis, drawn in strata rather than uniformly.
///
/// Small worlds are where robots reach edges and leave scents for each other,
/// so most draws stay there. But a corpus that only ever stays there cannot
/// see a bound checked at one digit and not at two, or a coordinate held in
/// something too small for the limit the contract declares. Those are ordinary
/// bugs, and a generator capped below the limit is blind to all of them.
/// How many robots a mission carries.
///
/// Stratified for the same reason the axes are: small missions are the common
/// case and the catalogue already covers them, but they cannot reach what only
/// a crowd exposes. Three robots in the right order say nothing about the
/// ninth, and a program that answers from a map keyed by position, or sorts
/// before printing, agrees with everything until there is enough to disagree
/// about. The tail is drawn often enough to arrive inside a modest budget.
fn population(rng: &mut Rng) -> u32 {
    match rng.below(10) {
        0..=5 => rng.below(4),
        6..=8 => 4 + rng.below(5),
        _ => 9 + rng.below(8),
    }
}

fn axis(rng: &mut Rng, max_coordinate: u32) -> u32 {
    match rng.below(10) {
        0..=5 => rng.below(6).min(max_coordinate),
        6..=8 => rng.below(max_coordinate + 1),
        _ => max_coordinate,
    }
}

/// An instruction count, in the same three strata and for the same reason: a
/// program that stops after a fixed number of steps is invisible to a corpus
/// whose strings are always shorter than the limit.
fn steps(rng: &mut Rng, max_instructions: u32) -> usize {
    let drawn = match rng.below(10) {
        0..=5 => rng.below(8),
        6..=8 => rng.below(max_instructions + 1),
        _ => max_instructions,
    };
    drawn as usize
}

fn tokens(line: &str) -> Vec<&str> {
    line.split([' ', '\t'])
        .filter(|part| !part.is_empty())
        .collect()
}

fn number(token: &str) -> Result<u32, String> {
    token
        .parse()
        .map_err(|_| format!("not a number: {token:?}"))
}

#[cfg(test)]
mod tests {
    use super::{Mission, Robot};
    use crate::contract::Contract;
    use crate::rng::Rng;

    #[test]
    fn a_canonical_mission_reads_back_as_itself() {
        let mission = Mission {
            max_x: 5,
            max_y: 3,
            robots: vec![
                Robot {
                    x: 1,
                    y: 1,
                    facing: 'E',
                    instructions: "RFRFRFRF".to_string(),
                },
                Robot {
                    x: 0,
                    y: 3,
                    facing: 'W',
                    instructions: String::new(),
                },
            ],
        };
        assert_eq!(Mission::read_back(&mission.canonical()).unwrap(), mission);
    }

    #[test]
    fn a_mission_with_no_robots_reads_back() {
        let mission = Mission {
            max_x: 0,
            max_y: 0,
            robots: Vec::new(),
        };
        assert_eq!(Mission::read_back(b"0 0\n").unwrap(), mission);
    }

    #[test]
    fn blank_lines_are_separators_and_an_empty_instruction_line_is_not() {
        let read = Mission::read_back(b"\n \n5 3\n1 1 E\n\n\n0 3 W\nLF\n").unwrap();
        assert_eq!(read.robots.len(), 2);
        assert_eq!(read.robots[0].instructions, "");
        assert_eq!(read.robots[1].instructions, "LF");
    }

    #[test]
    fn every_drawn_mission_reads_back_as_itself() {
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(20_260_922);
        for _ in 0..500 {
            let mission = Mission::draw(&mut rng, &contract);
            assert_eq!(
                Mission::read_back(&mission.canonical()).unwrap(),
                mission,
                "{mission:?}"
            );
        }
    }

    #[test]
    fn the_corpus_reaches_the_limits_the_contract_declares() {
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(20_260_929);
        let (mut widest, mut longest, mut most) = (0, 0, 0);
        for _ in 0..400 {
            let mission = Mission::draw_busy(&mut rng, &contract);
            widest = widest.max(mission.max_x).max(mission.max_y);
            most = most.max(mission.robots.len());
            for robot in &mission.robots {
                longest = longest.max(robot.instructions.len());
            }
        }
        assert_eq!(
            widest, contract.limits.max_coordinate,
            "no mission reached the coordinate limit"
        );
        assert_eq!(
            longest, contract.limits.max_instructions as usize,
            "no robot reached the instruction limit"
        );
        assert!(most >= 4, "only {most} robot(s) in any mission");
    }

    #[test]
    fn small_worlds_stay_the_common_case() {
        // Scent only happens where robots reach edges, so widening the range
        // must not drown out the worlds where the interesting interactions
        // are.
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(5);
        let small = (0..400)
            .filter(|_| {
                let mission = Mission::draw(&mut rng, &contract);
                mission.max_x <= 5 && mission.max_y <= 5
            })
            .count();
        assert!(small > 100, "only {small} of 400 missions were small");
    }

    #[test]
    fn drawn_missions_stay_on_their_own_grid() {
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(7);
        for _ in 0..500 {
            let mission = Mission::draw(&mut rng, &contract);
            for robot in &mission.robots {
                assert!(robot.x <= mission.max_x && robot.y <= mission.max_y);
            }
        }
    }
}
