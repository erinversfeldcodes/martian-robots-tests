//! Implementations that are wrong in exactly one way about §2.2.
//!
//! The semantic cases in this suite all carry a claim: that a particular
//! misreading of the scent rule fails them. That claim is worth no more than
//! the argument behind it until something demonstrates it, and the probe
//! cannot — it is right on purpose. So this program is a gallery of near
//! misses, one per reading somebody could plausibly arrive at from the brief,
//! selected by the `MISBEHAVE` variable.
//!
//! Everything outside §2.2 is borrowed from the library: it reads a mission
//! with the suite's own reader and refuses what that refuses. That is not a
//! conforming diagnostic and it is not meant to be. What matters is that each
//! defect leaves every *other* semantic case passing, because a program that
//! failed all of them would prove nothing about which case catches what.

use std::fmt::Write as _;
use std::io::Read;
use std::process::ExitCode;

use martian_robots_verify::contract::Contract;
use martian_robots_verify::mission::Mission;

fn main() -> ExitCode {
    let Ok(contract) = Contract::load() else {
        eprintln!("the contract this program carries is broken");
        return ExitCode::from(1);
    };
    let defect = std::env::var("MISBEHAVE").unwrap_or_default();

    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        eprintln!("(R22) line 1: stdin could not be read");
        return ExitCode::from(1);
    }

    match Mission::read_back(&input) {
        Ok(mission)
            if mission.is_valid(
                contract.limits.max_coordinate,
                contract.limits.max_instructions,
            ) =>
        {
            print!("{}", run(&mission, &defect));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("(R12) line 1: this program only simulates, and refuses the rest");
            ExitCode::from(1)
        }
    }
}

/// §2.2, with one thing wrong.
fn run(mission: &Mission, defect: &str) -> String {
    // A scent is a cell. One defect remembers the heading with it, which is
    // why the heading is stored at all.
    let mut scented: Vec<(u32, u32, char)> = Vec::new();
    let mut answer = String::new();

    for robot in &mission.robots {
        let (mut x, mut y) = (robot.x, robot.y);
        let mut facing = robot.facing;
        let mut lost = false;
        let placed_on_a_scent = scented.iter().any(|&(sx, sy, _)| (sx, sy) == (x, y));

        for step in robot.instructions.chars() {
            match step {
                'L' => facing = turned(facing, 3),
                'R' => facing = turned(facing, 1),
                'F' => {
                    let (ahead_x, ahead_y) = ahead(x, y, facing);
                    let leaves = ahead_x < 0
                        || ahead_y < 0
                        || ahead_x > i64::from(mission.max_x)
                        || ahead_y > i64::from(mission.max_y);
                    if !leaves {
                        x = u32::try_from(ahead_x).unwrap_or(x);
                        y = u32::try_from(ahead_y).unwrap_or(y);
                        continue;
                    }
                    if protected(&mut scented, x, y, facing, placed_on_a_scent, defect) {
                        // The move is ignored. One defect ends the run here
                        // instead, which is the difference between skipping an
                        // instruction and abandoning the rest.
                        if defect == "stop-on-ignore" {
                            break;
                        }
                        continue;
                    }
                    if defect == "one-scent" {
                        scented.clear();
                    }
                    scented.push((x, y, facing));
                    lost = true;
                    // A lost robot is off the grid and has nothing left to do.
                    if defect != "keep-simulating" {
                        break;
                    }
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

    answer
}

/// Whether a world-leaving move is ignored. Every defect about scent rather
/// than about control flow lives here.
fn protected(
    scented: &mut Vec<(u32, u32, char)>,
    x: u32,
    y: u32,
    facing: char,
    placed_on_a_scent: bool,
    defect: &str,
) -> bool {
    let on_this_cell =
        |scented: &Vec<(u32, u32, char)>| scented.iter().any(|&(sx, sy, _)| (sx, sy) == (x, y));
    match defect {
        // The brief says "the same grid point", which reads as a heading to
        // anyone picturing the robot rather than the cell.
        "direction-scent" => scented.contains(&(x, y, facing)),
        // The scent is looked up once, for where the robot was placed.
        "scent-at-start" => placed_on_a_scent,
        // A warning spent on the robot it saves.
        "scent-consumed" => {
            let found = on_this_cell(scented);
            scented.retain(|&(sx, sy, _)| (sx, sy) != (x, y));
            found
        }
        _ => on_this_cell(scented),
    }
}

const COMPASS: [char; 4] = ['N', 'E', 'S', 'W'];

fn turned(facing: char, quarters: usize) -> char {
    let at = COMPASS
        .iter()
        .position(|&point| point == facing)
        .unwrap_or(0);
    COMPASS[(at + quarters) % 4]
}

fn ahead(x: u32, y: u32, facing: char) -> (i64, i64) {
    let (x, y) = (i64::from(x), i64::from(y));
    match facing {
        'N' => (x, y + 1),
        'E' => (x + 1, y),
        'S' => (x, y - 1),
        _ => (x - 1, y),
    }
}
