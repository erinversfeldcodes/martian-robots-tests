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
        let max_coordinate = contract.limits.max_coordinate;
        let max_x = rng.below(6).min(max_coordinate);
        let max_y = rng.below(6).min(max_coordinate);
        let robots = (0..=rng.below(3))
            .map(|_| {
                let mut robot = Robot::draw_on(rng, max_x, max_y, contract);
                let length = rng.below(16) as usize;
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
        let max_coordinate = contract.limits.max_coordinate;
        let max_x = rng.below(6).min(max_coordinate);
        let max_y = rng.below(6).min(max_coordinate);
        let robots = (0..rng.below(4))
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
        let length = rng.below(7) as usize;
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
