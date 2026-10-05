//! A program built to conform, for the suite to be pointed at.
//!
//! Every other fixture is wrong on purpose and proves a check can go red. This
//! one is right on purpose and proves the checks are not red for a program
//! that has done nothing wrong — which is the failure a suite cannot see from
//! inside, because over-pinning looks exactly like a thorough gate until
//! somebody tries to satisfy it.
//!
//! So it is deliberately eccentric everywhere the contract is silent. It exits
//! 7 rather than 1, writes its diagnostics back-to-front with the ruling
//! first, says nothing resembling the word "usage" in its usage error, and
//! reports its version in a sentence. A case that fails this program has
//! pinned something the contract left free.
//!
//! It is not the implementation this contract is for. It parses and
//! diagnoses; the simulation is the suite's own reference, because a third
//! simulator would be a third chance to be wrong about §2.2.

use std::io::{Read, Write};
use std::process::ExitCode;

use martian_robots_verify::contract::Contract;
use martian_robots_verify::mission::{Mission, Robot};
use martian_robots_verify::reference;

/// Nothing in the contract names a non-zero code, so this one is not 1.
const REFUSED: u8 = 7;

fn main() -> ExitCode {
    let contract = match Contract::load() {
        Ok(contract) => contract,
        Err(message) => {
            eprintln!("the contract this probe carries is broken: {message}");
            return ExitCode::from(REFUSED);
        }
    };

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => simulate(&contract),
        [only] if only == "--help" || only == "-h" => {
            println!("martian robots: missions on stdin, answers on stdout.");
            ExitCode::SUCCESS
        }
        [only] if only == "--version" => {
            println!("this program implements contract {}.", contract.version);
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("that is not something this program knows how to be asked.");
            ExitCode::from(REFUSED)
        }
    }
}

fn simulate(contract: &Contract) -> ExitCode {
    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        eprintln!("(R22) stdin could not be read");
        return ExitCode::from(REFUSED);
    }

    match read(&input, contract) {
        Ok(mission) => {
            let _ = std::io::stdout().write_all(&reference::run(&mission));
            ExitCode::SUCCESS
        }
        Err(problems) => {
            for problem in &problems {
                // The ruling first and the line last, which no ruling forbids.
                match problem.line {
                    Some(line) => eprintln!(
                        "({}) trouble on line {line}: {}",
                        problem.rule, problem.what
                    ),
                    None => eprintln!("({}) trouble: {}", problem.rule, problem.what),
                }
            }
            ExitCode::from(REFUSED)
        }
    }
}

#[expect(
    clippy::naive_bytecount,
    reason = "counted once, on input already refused; a dependency to do it faster would cost more than it saves"
)]
fn bytecount(bytes: &[u8], wanted: u8) -> usize {
    bytes.iter().filter(|&&byte| byte == wanted).count()
}

struct Problem {
    rule: &'static str,
    line: Option<usize>,
    what: String,
}

/// Read a mission, collecting every line that breaks a rule on its own (R25).
fn read(input: &[u8], contract: &Contract) -> Result<Mission, Vec<Problem>> {
    let text = match std::str::from_utf8(input) {
        Ok(text) => text,
        Err(bad) => {
            let line = 1 + bytecount(&input[..bad.valid_up_to()], b'\n');
            return Err(vec![Problem {
                rule: "R22",
                line: Some(line),
                what: "this is not text".to_string(),
            }]);
        }
    };

    let lines = split(text)?;
    let separators = &contract.grammar.separators;
    let max = contract.limits.max_coordinate;
    let mut problems = Vec::new();

    let blank = |line: &str| line.trim_matches(|c| separators.contains(&c)).is_empty();
    let mut at = 0;
    while at < lines.len() && blank(lines[at]) {
        at += 1;
    }

    let Some(grid) = lines.get(at) else {
        return Err(vec![Problem {
            rule: "R12",
            line: None,
            what: "there is no grid line".to_string(),
        }]);
    };
    let grid_at = at + 1;
    at += 1;

    let mut world = (0, 0);
    match numbers(grid, separators, 2) {
        Ok(found) => {
            for value in &found {
                if *value > max {
                    problems.push(Problem {
                        rule: "R5",
                        line: Some(grid_at),
                        what: format!("{value} is past the limit"),
                    });
                }
            }
            world = (found[0], found[1]);
        }
        Err(what) => problems.push(Problem {
            rule: "R12",
            line: Some(grid_at),
            what,
        }),
    }

    let mut robots = Vec::new();
    read_robots(&lines, at, world, contract, &mut robots, &mut problems);

    if problems.is_empty() {
        Ok(Mission {
            max_x: world.0,
            max_y: world.1,
            robots,
        })
    } else {
        Err(problems)
    }
}

fn read_robots(
    lines: &[&str],
    from: usize,
    world: (u32, u32),
    contract: &Contract,
    robots: &mut Vec<Robot>,
    problems: &mut Vec<Problem>,
) {
    let separators = &contract.grammar.separators;
    let blank = |line: &str| line.trim_matches(|c| separators.contains(&c)).is_empty();
    let max = contract.limits.max_coordinate;
    let mut at = from;

    loop {
        while at < lines.len() && blank(lines[at]) {
            at += 1;
        }
        if at >= lines.len() {
            break;
        }

        let position = lines[at];
        let position_at = at + 1;
        let Some(instructions) = lines.get(at + 1) else {
            problems.push(Problem {
                rule: "R13",
                line: Some(position_at),
                what: "this robot never says what to do".to_string(),
            });
            break;
        };
        at += 2;

        let robot = robot(position, instructions, separators, contract);
        match robot {
            Ok(robot) => {
                for (value, name) in [(robot.x, "x"), (robot.y, "y")] {
                    if value > max {
                        problems.push(Problem {
                            rule: "R5",
                            line: Some(position_at),
                            what: format!("{name} of {value} is past the limit"),
                        });
                    }
                }
                if robot.x > world.0 || robot.y > world.1 {
                    problems.push(Problem {
                        rule: "R1",
                        line: Some(position_at),
                        what: "this robot starts nowhere".to_string(),
                    });
                }
                if robot.instructions.len() > contract.limits.max_instructions as usize {
                    problems.push(Problem {
                        rule: "R6",
                        line: Some(position_at + 1),
                        what: "too much to do".to_string(),
                    });
                }
                robots.push(robot);
            }
            Err((rule, line, what)) => problems.push(Problem {
                rule,
                line: Some(if line == 0 {
                    position_at
                } else {
                    position_at + 1
                }),
                what,
            }),
        }
    }
}

/// Lines, with R11's endings, R14's implicit final one and R19's guard.
fn split(text: &str) -> Result<Vec<&str>, Vec<Problem>> {
    let mut lines = Vec::new();
    let mut rest = text;
    let mut number = 1;

    while !rest.is_empty() {
        if let Some(at) = rest.find('\n') {
            let line = &rest[..at];
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.contains('\r') {
                return Err(vec![Problem {
                    rule: "R19",
                    line: Some(number),
                    what: "a carriage return that ends nothing".to_string(),
                }]);
            }
            lines.push(line);
            rest = &rest[at + 1..];
            number += 1;
        } else {
            // R14: end of input ends a line, but only a line that is there.
            // R19 lets a trailing carriage return complete it, and R18 stops
            // either from inventing one.
            let line = rest.strip_suffix('\r').unwrap_or(rest);
            if line.contains('\r') {
                return Err(vec![Problem {
                    rule: "R19",
                    line: Some(number),
                    what: "a carriage return that ends nothing".to_string(),
                }]);
            }
            if line.is_empty() {
                return Err(vec![Problem {
                    rule: "R19",
                    line: Some(number),
                    what: "a carriage return cannot make a line out of nothing".to_string(),
                }]);
            }
            lines.push(line);
            rest = "";
        }
    }

    Ok(lines)
}

fn robot(
    position: &str,
    instructions: &str,
    separators: &[char],
    contract: &Contract,
) -> Result<Robot, (&'static str, usize, String)> {
    let tokens = split_on(position, separators);
    let [x, y, facing] = tokens.as_slice() else {
        return Err((
            "R12",
            0,
            format!("{} token(s) where three belong", tokens.len()),
        ));
    };

    let number = |token: &str| -> Result<u32, (&'static str, usize, String)> {
        if token.is_empty() || !token.chars().all(|c| c.is_ascii_digit()) {
            return Err(("R12", 0, format!("{token:?} is not a number")));
        }
        token
            .parse()
            .map_err(|_| ("R5", 0, format!("{token:?} is past anything")))
    };

    let facing = {
        let mut characters = facing.chars();
        match (characters.next(), characters.next()) {
            (Some(only), None) if contract.grammar.orientations.contains(&only) => only,
            _ => return Err(("R7", 0, format!("{facing:?} is not a way to face"))),
        }
    };

    let trimmed = instructions.trim_matches(|c| separators.contains(&c));
    if let Some(wrong) = trimmed
        .chars()
        .find(|step| !contract.grammar.instructions.contains(step))
    {
        return Err(("R7", 1, format!("{wrong:?} is not something to do")));
    }

    Ok(Robot {
        x: number(x)?,
        y: number(y)?,
        facing,
        instructions: trimmed.to_string(),
    })
}

fn numbers(line: &str, separators: &[char], want: usize) -> Result<Vec<u32>, String> {
    let tokens = split_on(line, separators);
    if tokens.len() != want {
        return Err(format!("{} token(s) where {want} belong", tokens.len()));
    }
    tokens
        .iter()
        .map(|token| {
            if token.is_empty() || !token.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("{token:?} is not a number"));
            }
            token
                .parse()
                .map_err(|_| format!("{token:?} is past anything"))
        })
        .collect()
}

fn split_on<'a>(line: &'a str, separators: &[char]) -> Vec<&'a str> {
    line.split(|c| separators.contains(&c))
        .filter(|part| !part.is_empty())
        .collect()
}
