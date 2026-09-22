//! Statements about a program's own output that are true or false without
//! asking anybody else.
//!
//! Every differential check proves that two implementations agree. If this
//! suite and the program it grades share a wrong belief, the run is green and
//! nothing says otherwise — and since both were written from one document,
//! that correlation is real rather than hypothetical. These predicates have no
//! such dependency: each restates a sentence of the contract as something
//! observable in the answer itself.
//!
//! Two of them need no simulation at all. A robot whose instructions contain
//! no `F` cannot move, so its position is its start and it cannot be lost. And
//! since a loss scents the cell it happened on, and a world-leaving move from
//! a scented cell is ignored, no two robots can ever report a loss on the same
//! cell.

use crate::mission::{Mission, Robot};

pub const NAMES: [&str; 6] = [
    "one line per robot, in input order",
    "every line is canonical",
    "every reported position is on the grid",
    "a robot that cannot move reports where it started",
    "a robot that only moves forward stops where the world stops it",
    "no two robots are lost on the same cell",
];

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The mission says nothing about this property, which is not a pass.
    NotApplicable,
    Held,
    Violated(String),
}

#[derive(Debug, PartialEq, Eq)]
struct Reported {
    x: u32,
    y: u32,
    facing: char,
    lost: bool,
}

/// Judge every property against one answer. The order matches `NAMES`.
pub fn check(mission: &Mission, stdout: &[u8]) -> Vec<Verdict> {
    let reports = match parse(stdout) {
        Ok(reports) => reports,
        Err(why) => {
            // An unreadable answer is a violation of the shape properties and
            // says nothing about the rest.
            return vec![
                Verdict::Violated(why.clone()),
                Verdict::Violated(why),
                Verdict::NotApplicable,
                Verdict::NotApplicable,
                Verdict::NotApplicable,
                Verdict::NotApplicable,
            ];
        }
    };

    vec![
        one_line_per_robot(mission, &reports),
        Verdict::Held, // parsing strictly is the canonical-line property
        on_the_grid(mission, &reports),
        cannot_move(mission, &reports),
        only_forward(mission, &reports),
        no_two_losses_on_one_cell(&reports),
    ]
}

/// Strict by design: the shape of a line is contract (§2.3), so anything this
/// refuses is a violation rather than a parsing inconvenience.
fn parse(stdout: &[u8]) -> Result<Vec<Reported>, String> {
    let text = std::str::from_utf8(stdout).map_err(|_| "output is not UTF-8".to_string())?;
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let body = text
        .strip_suffix('\n')
        .ok_or("output does not end with a line ending")?;

    body.split('\n')
        .map(|line| {
            let (position, lost) = match line.strip_suffix(" LOST") {
                Some(position) => (position, true),
                None => (line, false),
            };
            let tokens: Vec<&str> = position.split(' ').collect();
            let [x, y, facing] = tokens[..] else {
                return Err(format!("line is not `x y orientation`: {line:?}"));
            };
            Ok(Reported {
                x: canonical_number(x)?,
                y: canonical_number(y)?,
                facing: match facing {
                    "N" | "E" | "S" | "W" => facing.chars().next().expect("one character"),
                    _ => return Err(format!("not an orientation: {facing:?}")),
                },
                lost,
            })
        })
        .collect()
}

fn canonical_number(token: &str) -> Result<u32, String> {
    if token.len() > 1 && token.starts_with('0') {
        return Err(format!("number is not canonical: {token:?}"));
    }
    token
        .parse()
        .map_err(|_| format!("not a number: {token:?}"))
}

fn one_line_per_robot(mission: &Mission, reports: &[Reported]) -> Verdict {
    if reports.len() == mission.robots.len() {
        Verdict::Held
    } else {
        Verdict::Violated(format!(
            "{} robot(s), {} line(s)",
            mission.robots.len(),
            reports.len()
        ))
    }
}

fn on_the_grid(mission: &Mission, reports: &[Reported]) -> Verdict {
    if reports.is_empty() {
        return Verdict::NotApplicable;
    }
    for report in reports {
        if report.x > mission.max_x || report.y > mission.max_y {
            return Verdict::Violated(format!(
                "({}, {}) is outside {} {}",
                report.x, report.y, mission.max_x, mission.max_y
            ));
        }
    }
    Verdict::Held
}

/// A robot whose instructions contain no `F` never moves, so its position is
/// its start, it keeps turning through whatever the turns say, and it cannot
/// be lost. No delta table required.
fn cannot_move(mission: &Mission, reports: &[Reported]) -> Verdict {
    let mut applied = false;
    for (robot, report) in mission.robots.iter().zip(reports) {
        if robot.instructions.contains('F') {
            continue;
        }
        applied = true;
        let turned = turned(robot);
        if report.lost || report.x != robot.x || report.y != robot.y || report.facing != turned {
            return Verdict::Violated(format!(
                "a robot at ({}, {}) facing {} with {:?} reported ({}, {}) facing {}{}",
                robot.x,
                robot.y,
                robot.facing,
                robot.instructions,
                report.x,
                report.y,
                report.facing,
                if report.lost { " LOST" } else { "" }
            ));
        }
    }
    if applied {
        Verdict::Held
    } else {
        Verdict::NotApplicable
    }
}

/// The first robot runs on a world with no scents in it, so an instruction
/// string of nothing but `F` has exactly one answer: it walks until the world
/// stops it, and is lost the step after that.
fn only_forward(mission: &Mission, reports: &[Reported]) -> Verdict {
    let (Some(robot), Some(report)) = (mission.robots.first(), reports.first()) else {
        return Verdict::NotApplicable;
    };
    if robot.instructions.is_empty() || robot.instructions.chars().any(|step| step != 'F') {
        return Verdict::NotApplicable;
    }

    let steps = u32::try_from(robot.instructions.len()).unwrap_or(u32::MAX);
    let room = match robot.facing {
        'N' => mission.max_y - robot.y,
        'S' => robot.y,
        'E' => mission.max_x - robot.x,
        _ => robot.x,
    };
    let travelled = steps.min(room);
    let (x, y) = match robot.facing {
        'N' => (robot.x, robot.y + travelled),
        'S' => (robot.x, robot.y - travelled),
        'E' => (robot.x + travelled, robot.y),
        _ => (robot.x - travelled, robot.y),
    };
    let expected = Reported {
        x,
        y,
        facing: robot.facing,
        lost: steps > room,
    };

    if *report == expected {
        Verdict::Held
    } else {
        Verdict::Violated(format!(
            "{} {} {} with {} step(s) forward on {} {}: expected {expected:?}, got {report:?}",
            robot.x, robot.y, robot.facing, steps, mission.max_x, mission.max_y
        ))
    }
}

/// A loss scents the cell it happened on, and a world-leaving move from a
/// scented cell is ignored. So a second loss on the same cell is impossible,
/// whatever the missions or the movement code.
fn no_two_losses_on_one_cell(reports: &[Reported]) -> Verdict {
    let mut lost: Vec<(u32, u32)> = Vec::new();
    for report in reports.iter().filter(|report| report.lost) {
        if lost.contains(&(report.x, report.y)) {
            return Verdict::Violated(format!("two robots lost on ({}, {})", report.x, report.y));
        }
        lost.push((report.x, report.y));
    }
    if lost.is_empty() {
        Verdict::NotApplicable
    } else {
        Verdict::Held
    }
}

fn turned(robot: &Robot) -> char {
    const COMPASS: [char; 4] = ['N', 'E', 'S', 'W'];
    let mut at = COMPASS
        .iter()
        .position(|&point| point == robot.facing)
        .unwrap_or(0);
    for step in robot.instructions.chars() {
        at = match step {
            'R' => (at + 1) % 4,
            'L' => (at + 3) % 4,
            _ => at,
        };
    }
    COMPASS[at]
}

#[cfg(test)]
mod tests {
    use super::{NAMES, Verdict, check};
    use crate::mission::{Mission, Robot};

    fn mission(max_x: u32, max_y: u32, robots: &[(u32, u32, char, &str)]) -> Mission {
        Mission {
            max_x,
            max_y,
            robots: robots
                .iter()
                .map(|&(x, y, facing, instructions)| Robot {
                    x,
                    y,
                    facing,
                    instructions: instructions.to_string(),
                })
                .collect(),
        }
    }

    fn violations(mission: &Mission, stdout: &[u8]) -> Vec<String> {
        check(mission, stdout)
            .into_iter()
            .zip(NAMES)
            .filter_map(|(verdict, name)| match verdict {
                Verdict::Violated(_) => Some(name.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_correct_answer_violates_nothing() {
        let mission = mission(5, 3, &[(1, 1, 'E', "RFRFRFRF"), (0, 3, 'W', "LL")]);
        assert!(violations(&mission, b"1 1 E\n0 3 E\n").is_empty());
    }

    #[test]
    fn a_missing_line_is_caught() {
        let mission = mission(5, 3, &[(1, 1, 'E', "RFRFRFRF"), (0, 3, 'W', "LL")]);
        assert_eq!(
            violations(&mission, b"1 1 E\n"),
            ["one line per robot, in input order"]
        );
    }

    #[test]
    fn a_non_canonical_number_is_caught() {
        let mission = mission(5, 3, &[(1, 1, 'E', "")]);
        let caught = violations(&mission, b"01 1 E\n");
        assert!(
            caught.contains(&"every line is canonical".to_string()),
            "{caught:?}"
        );
    }

    #[test]
    fn a_missing_final_line_ending_is_caught() {
        let mission = mission(5, 3, &[(1, 1, 'E', "")]);
        assert!(!violations(&mission, b"1 1 E").is_empty());
    }

    #[test]
    fn a_position_off_the_grid_is_caught() {
        let mission = mission(5, 3, &[(1, 1, 'E', "")]);
        let caught = violations(&mission, b"9 9 E\n");
        assert!(
            caught.contains(&"every reported position is on the grid".to_string()),
            "{caught:?}"
        );
    }

    #[test]
    fn a_robot_that_cannot_move_and_moved_is_caught() {
        let mission = mission(5, 3, &[(1, 1, 'N', "LR")]);
        let caught = violations(&mission, b"1 2 N\n");
        assert!(
            caught.contains(&"a robot that cannot move reports where it started".to_string()),
            "{caught:?}"
        );
    }

    #[test]
    fn a_robot_that_cannot_move_keeps_turning() {
        let mission = mission(5, 3, &[(1, 1, 'N', "RR")]);
        assert!(violations(&mission, b"1 1 S\n").is_empty());
        assert!(!violations(&mission, b"1 1 N\n").is_empty());
    }

    #[test]
    fn walking_off_the_edge_is_pinned_without_simulating_anything() {
        // Three steps north from y=1 on a world three high: two steps to the
        // edge, lost on the third.
        let mission = mission(5, 3, &[(1, 1, 'N', "FFF")]);
        assert!(violations(&mission, b"1 3 N LOST\n").is_empty());
        let caught = violations(&mission, b"1 3 N\n");
        assert!(
            caught.contains(
                &"a robot that only moves forward stops where the world stops it".to_string()
            ),
            "{caught:?}"
        );
    }

    #[test]
    fn staying_on_the_grid_is_pinned_the_same_way() {
        let mission = mission(5, 3, &[(1, 1, 'E', "FF")]);
        assert!(violations(&mission, b"3 1 E\n").is_empty());
        assert!(!violations(&mission, b"4 1 E\n").is_empty());
    }

    #[test]
    fn two_losses_on_one_cell_are_impossible() {
        let mission = mission(1, 1, &[(1, 1, 'N', "F"), (1, 1, 'N', "F")]);
        let caught = violations(&mission, b"1 1 N LOST\n1 1 N LOST\n");
        assert!(
            caught.contains(&"no two robots are lost on the same cell".to_string()),
            "{caught:?}"
        );
    }

    #[test]
    fn a_property_a_mission_says_nothing_about_does_not_count_as_passing() {
        let mission = mission(5, 3, &[(1, 1, 'E', "RFRFRFRF")]);
        let verdicts = check(&mission, b"1 1 E\n");
        assert_eq!(verdicts[5], Verdict::NotApplicable, "nothing was lost");
        assert_eq!(verdicts[3], Verdict::NotApplicable, "the robot could move");
    }
}
