//! Valid missions broken on purpose, one way at a time.
//!
//! The catalogue pins rejection at the shapes somebody enumerated. This breaks
//! generated missions by construction and checks the same obligations over a
//! much larger space: no stdout, a non-zero exit, and a diagnostic that names
//! the line the defect is on and cites a ruling that governs it.
//!
//! Where the expectation comes from matters. The line and the admissible tags
//! are derived from *how the mutation was built* — never from what the program
//! under test said — so a program cannot teach the suite to accept its own
//! answer. And the input is checked to be genuinely invalid before it is used,
//! because a mutation that quietly left a valid mission behind would turn a
//! rejection test into a much weaker success test that still reports green.

use crate::mission::Mission;
use crate::rng::Rng;

pub struct Mutation {
    pub kind: &'static str,
    pub rendered: Vec<u8>,
    /// The 1-based physical line the defect sits on. Canonical renderings put
    /// the grid line first and two lines per robot, so this is arithmetic.
    pub line: usize,
    pub tags: Vec<&'static str>,
}

/// How the mutation makes the input invalid, and therefore what must be true
/// of it before it is used to grade anybody.
enum Invalidity {
    /// The grammar rejects it: the suite's own reader must fail to read it.
    Grammar,
    /// It parses, but the contract's limits refuse it.
    Semantic,
}

struct Broken {
    kind: &'static str,
    at: usize,
    tags: Vec<&'static str>,
    invalidity: Invalidity,
}

pub fn mutate(
    rng: &mut Rng,
    mission: &Mission,
    max_coordinate: u32,
    max_instructions: u32,
) -> Option<Mutation> {
    let mut lines: Vec<String> = String::from_utf8(mission.canonical())
        .ok()?
        .lines()
        .map(ToString::to_string)
        .collect();
    if mission.robots.is_empty() {
        return None;
    }
    let robot = rng.below(u32::try_from(mission.robots.len()).ok()?) as usize;
    let broken = if rng.chance(2) {
        break_a_value(
            rng,
            mission,
            &mut lines,
            robot,
            max_coordinate,
            max_instructions,
        )?
    } else {
        break_the_shape(rng, mission, &mut lines, robot)
    };

    let rendered = format!("{}\n", lines.join("\n")).into_bytes();
    check(
        &rendered,
        &broken.invalidity,
        max_coordinate,
        max_instructions,
    )?;

    Some(Mutation {
        kind: broken.kind,
        rendered,
        line: broken.at + 1,
        tags: broken.tags,
    })
}

/// Break what a token means: a number past a limit, a letter outside a
/// vocabulary, a string longer than the contract allows. The line still has
/// the shape the grammar wants.
fn break_a_value(
    rng: &mut Rng,
    mission: &Mission,
    lines: &mut [String],
    robot: usize,
    max_coordinate: u32,
    max_instructions: u32,
) -> Option<Broken> {
    let position = 1 + robot * 2;
    let instructions = position + 1;
    let at_hand = &mission.robots[robot];

    Some(match rng.below(5) {
        // A coordinate over the limit on the grid line. Nothing else can
        // govern it: the robots move with the world.
        0 => {
            lines[0] = format!("{} {}", max_coordinate + 1 + rng.below(9), mission.max_y);
            Broken {
                kind: "a grid coordinate over the limit",
                at: 0,
                tags: vec!["R5"],
                invalidity: Invalidity::Semantic,
            }
        }
        // On a position line the same value is also off the world, so either
        // ruling governs (§2.5).
        1 => {
            lines[position] = format!(
                "{} {} {}",
                max_coordinate + 1 + rng.below(9),
                at_hand.y,
                at_hand.facing
            );
            Broken {
                kind: "a robot coordinate over the limit",
                at: position,
                tags: vec!["R5", "R1"],
                invalidity: Invalidity::Semantic,
            }
        }
        // Off the world while staying under the limit, which isolates R1 -
        // possible only where the world is smaller than the limit.
        2 => {
            if mission.max_x >= max_coordinate {
                return None;
            }
            let beyond = mission.max_x + 1 + rng.below(max_coordinate - mission.max_x);
            lines[position] = format!("{beyond} {} {}", at_hand.y, at_hand.facing);
            Broken {
                kind: "a start off the world",
                at: position,
                tags: vec!["R1"],
                invalidity: Invalidity::Semantic,
            }
        }
        3 => {
            let wrong = *rng.pick(&['Q', 'n', 'e', 'X']);
            lines[position] = format!("{} {} {wrong}", at_hand.x, at_hand.y);
            Broken {
                kind: "an orientation outside the vocabulary",
                at: position,
                tags: vec!["R7", "R12"],
                invalidity: Invalidity::Semantic,
            }
        }
        _ => {
            lines[instructions] = "L".repeat(max_instructions as usize + 1);
            Broken {
                kind: "an instruction string over the limit",
                at: instructions,
                tags: vec!["R6"],
                invalidity: Invalidity::Semantic,
            }
        }
    })
}

/// Break the shape of a line: a token too many, a token too few, a character
/// the grammar has no place for.
fn break_the_shape(rng: &mut Rng, mission: &Mission, lines: &mut [String], robot: usize) -> Broken {
    let position = 1 + robot * 2;
    let instructions = position + 1;
    let at_hand = &mission.robots[robot];

    match rng.below(4) {
        0 => {
            lines[position] = format!("{} X", lines[position]);
            Broken {
                kind: "an extra token on a position line",
                at: position,
                tags: vec!["R12"],
                invalidity: Invalidity::Grammar,
            }
        }
        1 => {
            lines[position] = format!("{} {}", at_hand.x, at_hand.y);
            Broken {
                kind: "a position line with no orientation",
                at: position,
                tags: vec!["R12"],
                invalidity: Invalidity::Grammar,
            }
        }
        2 => {
            let wrong = *rng.pick(&['X', 'r', 'f', '1']);
            let mut line = lines[instructions].clone();
            let at = rng.below(u32::try_from(line.len()).unwrap_or(0) + 1) as usize;
            line.insert(at, wrong);
            lines[instructions] = line;
            Broken {
                kind: "an instruction outside the vocabulary",
                at: instructions,
                tags: vec!["R7", "R12"],
                // The line still has the shape the grammar wants - one token
                // of letters - so what refuses it is the vocabulary, not the
                // grammar.
                invalidity: Invalidity::Semantic,
            }
        }
        // A separator the grammar does not have. Q6 leaves open whether a
        // diagnostic calls this a bad separator or an unknown character, so
        // both readings satisfy the obligation.
        _ => {
            let foreign = *rng.pick(&['\u{c}', '\u{b}', '\u{a0}', '\u{3000}']);
            lines[position] = format!("{}{foreign}{} {}", at_hand.x, at_hand.y, at_hand.facing);
            Broken {
                kind: "a separator the grammar does not have",
                at: position,
                tags: vec!["R4", "R12", "R7"],
                invalidity: Invalidity::Grammar,
            }
        }
    }
}

/// A mutation that left valid input behind would quietly turn a rejection
/// test into a success test. `None` here means the generator is wrong, not
/// the program.
fn check(
    rendered: &[u8],
    invalidity: &Invalidity,
    max_coordinate: u32,
    max_instructions: u32,
) -> Option<()> {
    // A semantic mutation the reader cannot parse is still invalid input,
    // just for a reason this mutation did not intend.
    let refused = match (invalidity, Mission::read_back(rendered)) {
        (Invalidity::Grammar, read) => read.is_err(),
        (Invalidity::Semantic, Ok(mission)) => !mission.is_valid(max_coordinate, max_instructions),
        (Invalidity::Semantic, Err(_)) => true,
    };
    refused.then_some(())
}

#[cfg(test)]
mod tests {
    use super::mutate;
    use crate::mission::Mission;
    use crate::rng::Rng;

    #[test]
    fn every_mutation_leaves_input_the_contract_refuses() {
        let mut rng = Rng::from_seed(20_260_922);
        let mut built = 0;
        for _ in 0..2_000 {
            let mission = Mission::draw(&mut rng, 50);
            if let Some(mutation) = mutate(&mut rng, &mission, 50, 99) {
                built += 1;
                let valid =
                    Mission::read_back(&mutation.rendered).is_ok_and(|read| read.is_valid(50, 99));
                assert!(
                    !valid,
                    "{} left valid input: {:?}",
                    mutation.kind,
                    String::from_utf8_lossy(&mutation.rendered)
                );
            }
        }
        assert!(built > 500, "only {built} mutations were built");
    }

    #[test]
    fn the_line_named_is_the_line_changed() {
        let mut rng = Rng::from_seed(4);
        for _ in 0..2_000 {
            let mission = Mission::draw(&mut rng, 50);
            let canonical = String::from_utf8(mission.canonical()).unwrap();
            let Some(mutation) = mutate(&mut rng, &mission, 50, 99) else {
                continue;
            };
            let before: Vec<&str> = canonical.lines().collect();
            let after = String::from_utf8(mutation.rendered.clone()).unwrap();
            let after: Vec<&str> = after.lines().collect();
            let changed: Vec<usize> = before
                .iter()
                .zip(&after)
                .enumerate()
                .filter(|(_, (was, is))| was != is)
                .map(|(at, _)| at + 1)
                .collect();
            assert_eq!(
                changed,
                vec![mutation.line],
                "{} says line {} but changed {:?}",
                mutation.kind,
                mutation.line,
                changed
            );
        }
    }

    #[test]
    fn every_kind_of_mutation_gets_built() {
        let mut rng = Rng::from_seed(88);
        let mut kinds = std::collections::BTreeSet::new();
        for _ in 0..4_000 {
            let mission = Mission::draw(&mut rng, 50);
            if let Some(mutation) = mutate(&mut rng, &mission, 50, 99) {
                kinds.insert(mutation.kind);
            }
        }
        assert_eq!(kinds.len(), 9, "only built {kinds:?}");
    }
}
