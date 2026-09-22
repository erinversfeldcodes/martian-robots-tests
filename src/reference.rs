//! A second implementation, written from the contract.
//!
//! Everything else in this suite checks a program against itself or against a
//! statement that needs no simulation. None of that catches a plain wrong
//! answer to a mixed instruction string: a robot that ends one cell east of
//! where it should agrees with itself across every respelling and satisfies
//! every invariant. Only another implementation can say otherwise.
//!
//! What this proves is agreement, and that is worth saying plainly. A
//! divergence is a defect in the program under test *or* a place where the
//! contract admits two readings and the two sides took different ones. Both
//! are findings; neither is automatically the program's fault.

use std::fmt::Write as _;

use crate::mission::Mission;

/// Run a mission the way §2.2 describes it. The mission must be valid — the
/// generators only produce valid ones for this mode — so nothing here
/// diagnoses anything.
pub fn run(mission: &Mission) -> Vec<u8> {
    let mut scented: Vec<(u32, u32)> = Vec::new();
    let mut answer = String::new();

    for robot in &mission.robots {
        let (mut x, mut y) = (robot.x, robot.y);
        let mut facing = robot.facing;
        let mut lost = false;

        for step in robot.instructions.chars() {
            match step {
                // Three quarter-turns right is one left, which keeps the
                // arithmetic unsigned.
                'L' => facing = turned(facing, 3),
                'R' => facing = turned(facing, 1),
                'F' => {
                    let (ahead_x, ahead_y) = ahead(x, y, facing);
                    let leaves = ahead_x < 0
                        || ahead_y < 0
                        || ahead_x > i64::from(mission.max_x)
                        || ahead_y > i64::from(mission.max_y);
                    if leaves {
                        // R9: the scent is a property of the cell, not of the
                        // direction the robot that left it was facing.
                        if scented.contains(&(x, y)) {
                            continue;
                        }
                        scented.push((x, y));
                        lost = true;
                        break;
                    }
                    x = u32::try_from(ahead_x).unwrap_or(x);
                    y = u32::try_from(ahead_y).unwrap_or(y);
                }
                _ => {}
            }
        }

        let _ = writeln!(
            answer,
            "{x} {y} {facing}{}",
            if lost { " LOST" } else { "" }
        );
    }

    answer.into_bytes()
}

const COMPASS: [char; 4] = ['N', 'E', 'S', 'W'];

fn turned(facing: char, quarters: usize) -> char {
    let at = COMPASS
        .iter()
        .position(|&point| point == facing)
        .unwrap_or(0);
    COMPASS[(at + quarters) % 4]
}

/// North is the direction from `(x, y)` to `(x, y + 1)`, which is the only
/// thing in the brief that fixes the compass.
fn ahead(x: u32, y: u32, facing: char) -> (i64, i64) {
    let (x, y) = (i64::from(x), i64::from(y));
    match facing {
        'N' => (x, y + 1),
        'E' => (x + 1, y),
        'S' => (x, y - 1),
        _ => (x - 1, y),
    }
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::contract::{SAMPLE_INPUT, SAMPLE_OUTPUT};
    use crate::mission::Mission;

    #[test]
    fn the_briefs_sample_comes_out_as_the_brief_says() {
        // The only external truth available to this suite: input and output
        // both published, by somebody who wrote neither of these
        // implementations.
        let mission = Mission::read_back(SAMPLE_INPUT).expect("the sample is a valid mission");
        assert_eq!(run(&mission), SAMPLE_OUTPUT);
    }

    fn answer(input: &str) -> String {
        let mission = Mission::read_back(input.as_bytes()).expect("a valid mission");
        String::from_utf8(run(&mission)).expect("text")
    }

    #[test]
    fn a_robot_with_nothing_to_do_reports_where_it_stands() {
        assert_eq!(answer("5 3\n1 1 E\n\n"), "1 1 E\n");
    }

    #[test]
    fn turning_does_not_move_anything() {
        assert_eq!(answer("5 3\n1 1 N\nLL\n"), "1 1 S\n");
        assert_eq!(answer("5 3\n1 1 N\nRRR\n"), "1 1 W\n");
    }

    #[test]
    fn a_robot_that_walks_off_is_lost_where_it_last_stood() {
        assert_eq!(answer("1 1\n1 1 N\nF\n"), "1 1 N LOST\n");
    }

    #[test]
    fn a_scent_blocks_a_departure_by_any_edge() {
        // The corner case the brief's own sample cannot distinguish.
        assert_eq!(answer("1 1\n1 1 N\nF\n1 1 E\nF\n"), "1 1 N LOST\n1 1 E\n");
    }

    #[test]
    fn a_scent_is_not_used_up() {
        assert_eq!(
            answer("1 1\n1 1 N\nF\n1 1 N\nF\n1 1 N\nF\n"),
            "1 1 N LOST\n1 1 N\n1 1 N\n"
        );
    }

    #[test]
    fn a_scent_marks_the_cell_and_not_the_robot() {
        assert_eq!(
            answer("2 0\n0 0 W\nF\n0 0 E\nFFF\n"),
            "0 0 W LOST\n2 0 E LOST\n"
        );
    }

    #[test]
    fn an_ignored_move_does_not_end_the_run() {
        // One instruction is ignored, not the rest of the string: the robot
        // survives the scented edge, turns, and walks inland.
        assert_eq!(answer("2 1\n2 1 N\nF\n2 1 N\nFLF\n"), "2 1 N LOST\n1 1 W\n");
    }

    #[test]
    fn a_mission_with_no_robots_says_nothing() {
        assert_eq!(answer("5 3\n"), "");
    }
}
