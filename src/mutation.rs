use crate::contract::Contract;
use crate::mission::Mission;
use crate::rng::Rng;

pub struct Mutation {
    pub kind: &'static str,
    pub rendered: Vec<u8>,
    pub expectation: Expectation,
}

/// What the diagnostic owes, derived from how the mutation was built.
pub enum Expectation {
    /// The 1-based physical line the defect sits on, and the rulings any one
    /// of which governs it.
    At(usize, Vec<&'static str>),
    /// Two framing rules can disagree about which line carries the defect,
    /// and §2.5 does not settle it, so only the ruling is required.
    Tagged(Vec<&'static str>),
    /// Independent defects on separate lines, each governed by a ruling of
    /// its own. R25 asks for every violation found in one pass, and no
    /// single-defect mutation can ask for that.
    Several(Vec<(usize, Vec<&'static str>)>),
}

/// How the mutation makes the input invalid, and therefore what must be true
/// of it before it is used to grade anybody.
enum Invalidity {
    /// The grammar refuses it: `Mission::read_back` cannot read it against the
    /// grammar the contract publishes.
    ///
    /// The two vocabulary kinds were `Semantic` until the reader was made
    /// faithful to the grammar. They were always grammar defects — a letter
    /// outside `L R F` does not match the `instruction` production — and the
    /// label was following the reader rather than the contract. Saying
    /// `Grammar` is also the stricter claim: it demands the input be
    /// unreadable, where `Semantic` accepts either outcome.
    Grammar,
    Semantic,
    /// Refused, and which mechanism refuses it is incidental. A character that
    /// is not a separator makes a number unreadable where a number belongs and
    /// becomes an unknown instruction where instructions belong; the claim
    /// being made is the same one either way.
    Either,
}

struct Broken {
    kind: &'static str,
    at: usize,
    tags: Vec<&'static str>,
    invalidity: Invalidity,
}

pub fn mutate(rng: &mut Rng, mission: &Mission, contract: &Contract) -> Option<Mutation> {
    let max_coordinate = contract.limits.max_coordinate;
    let max_instructions = contract.limits.max_instructions;
    let mut lines: Vec<String> = String::from_utf8(mission.canonical())
        .ok()?
        .lines()
        .map(ToString::to_string)
        .collect();
    if mission.robots.is_empty() {
        return None;
    }
    let robot = rng.below(u32::try_from(mission.robots.len()).ok()?) as usize;

    // A quarter of the time, put a character where a separator belongs that
    // the grammar does not admit as one. This is the largest family of
    // realistic parser bugs: every mainstream runtime's idea of "whitespace"
    // is wider than `ws`, so a program built on one accepts input the contract
    // refuses, and no other family here notices.
    if rng.chance(4) {
        return break_the_whitespace(rng, mission, contract);
    }

    // Bytes that are not text, from their own taxonomy.
    if rng.chance(6) {
        return break_the_encoding(rng, mission, contract);
    }

    // Two independent defects, when there is more than one robot to put them
    // on. Every other family breaks one thing, so nothing else in this mode
    // asks a program to find a second.
    if mission.robots.len() > 1 && rng.chance(5) {
        return break_two_blocks(rng, mission, contract);
    }

    // A third of the time, break the shape of the input rather than the
    // content of one line. §2.4 says a missing or extra line shifts the blocks
    // after it and they are diagnosed where they fall, and nothing else here
    // produces one.
    if rng.chance(3) {
        return break_the_framing(rng, mission, contract);
    }

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
        break_the_shape(rng, mission, &mut lines, robot, contract)
    };

    let rendered = format!("{}\n", lines.join("\n")).into_bytes();
    check(&rendered, &broken.invalidity, contract)?;

    Some(Mutation {
        kind: broken.kind,
        rendered,
        expectation: Expectation::At(broken.at + 1, broken.tags),
    })
}

/// Put a character the grammar does not admit where a separator belongs.
///
/// The discipline that makes this claim anything is one character: every input
/// differs from a valid mission by exactly the character injected, and putting
/// a space there restores a valid mission. That is asserted before the input is
/// used, so a failure means the program accepted *this character* rather than
/// something else that happened to be wrong with the line.
///
/// The positions are the ones where a space would be legal, and the one that
/// catches the most parsers is inside an otherwise-legal run: a program that
/// trims and splits stops at the offender and blames the innocent space beside
/// it.
fn break_the_whitespace(rng: &mut Rng, mission: &Mission, contract: &Contract) -> Option<Mutation> {
    let foreign = *rng.pick(&contract.grammar.not_separators());
    let space = *contract.grammar.separators.first()?;
    let text = String::from_utf8(mission.canonical()).ok()?;
    let lines: Vec<String> = text.lines().map(ToString::to_string).collect();

    // Any line but the grid line belongs to a robot, and a mission with no
    // robots has only the grid line to work with.
    let at = rng.below(u32::try_from(lines.len()).ok()?) as usize;
    let line = &lines[at];

    let (where_at, broken) = match rng.below(4) {
        0 => ("leading", format!("{foreign}{line}")),
        1 => ("trailing", format!("{line}{foreign}")),
        // Replace a separator, which only a line that has one can do.
        2 => {
            let separator = line.find(space)?;
            (
                "where a separator belongs",
                format!(
                    "{}{foreign}{}",
                    &line[..separator],
                    &line[separator + space.len_utf8()..]
                ),
            )
        }
        // Hidden inside a run of real separators.
        _ => {
            let separator = line.find(space)?;
            (
                "inside a run of separators",
                format!(
                    "{}{space}{foreign}{space}{}",
                    &line[..separator],
                    &line[separator + space.len_utf8()..]
                ),
            )
        }
    };

    // The name carries all three axes, because which cell of the matrix broke
    // is the finding: a program that rejects a leading one and accepts one
    // hidden in a run has a trim-then-split reader, and nothing else says so.
    let line_type = match at {
        0 => "the grid line",
        _ if at % 2 == 1 => "a position line",
        _ => "an instruction line",
    };
    let kind = match (where_at, line_type) {
        ("leading", "the grid line") => "a foreign separator, leading, on the grid line",
        ("leading", "a position line") => "a foreign separator, leading, on a position line",
        ("leading", _) => "a foreign separator, leading, on an instruction line",
        ("trailing", "the grid line") => "a foreign separator, trailing, on the grid line",
        ("trailing", "a position line") => "a foreign separator, trailing, on a position line",
        ("trailing", _) => "a foreign separator, trailing, on an instruction line",
        ("where a separator belongs", "the grid line") => {
            "a foreign separator, where a separator belongs, on the grid line"
        }
        ("where a separator belongs", _) => {
            "a foreign separator, where a separator belongs, on a position line"
        }
        (_, "the grid line") => "a foreign separator, inside a run, on the grid line",
        _ => "a foreign separator, inside a run, on a position line",
    };

    let mut mutated = lines.clone();
    mutated[at] = broken;
    let rendered = format!("{}\n", mutated.join("\n")).into_bytes();

    // One character: put a space where the offender is and the mission comes
    // back. Without this the case could be failing for any other reason.
    let mut restored = lines.clone();
    restored[at] = mutated[at].replace(foreign, &space.to_string());
    let restored = format!("{}\n", restored.join("\n")).into_bytes();
    if !Mission::read_back(&restored, &contract.grammar)
        .is_ok_and(|mission| mission.is_valid(contract))
    {
        return None;
    }

    check(&rendered, &Invalidity::Either, contract)?;

    // Q6 leaves the characterisation open: a bad separator, or a character
    // outside the vocabulary. Any governing ruling satisfies §2.5 - but R7's
    // question is scoped to an instruction string or an orientation, and the
    // grid line has neither, so there it cannot be read as governing at all.
    // The hand-written form-feed case on the grid line already says so.
    let tags = if line_type == "the grid line" {
        vec!["R4", "R12"]
    } else {
        vec!["R4", "R12", "R7"]
    };

    Some(Mutation {
        kind,
        rendered,
        expectation: Expectation::At(at + 1, tags),
    })
}

/// Bytes that are not text, drawn from a taxonomy of how decoders fail rather
/// than from one value somebody picked.
///
/// The classes are chosen for what a lenient decoder does with them, and they
/// are not equally dangerous. An overlong encoding is the one that matters: a
/// decoder that reads `C0 A0` as a space turns input the contract refuses into
/// a mission it accepts, and nothing downstream can tell that happened. A
/// surrogate and a code point past the last one are what a decoder written
/// against UTF-16, or against an old table, lets through. A truncation is what
/// a reader that splits a buffer mid-character produces, and a lone
/// continuation byte is what arrives when somebody concatenated two halves in
/// the wrong order.
///
/// Every sequence here is invalid whatever it sits next to, because a
/// canonical mission is ASCII and ASCII is not a continuation byte — but the
/// invalidity is proved per input all the same.
fn break_the_encoding(rng: &mut Rng, mission: &Mission, contract: &Contract) -> Option<Mutation> {
    const CLASSES: [(&str, &[u8]); 10] = [
        ("an overlong encoding of a space", &[0xc0, 0xa0]),
        ("an overlong encoding of a digit", &[0xc0, 0xb0]),
        (
            "an overlong three-byte encoding of a space",
            &[0xe0, 0x80, 0xa0],
        ),
        ("a UTF-16 surrogate", &[0xed, 0xa0, 0x80]),
        ("a code point past the last one", &[0xf4, 0x90, 0x80, 0x80]),
        ("a two-byte character cut short", &[0xc3]),
        ("a three-byte character cut short", &[0xe2, 0x82]),
        ("a four-byte character cut short", &[0xf0, 0x9f, 0x92]),
        ("a byte a Latin-1 decoder reads as a letter", &[0xe9]),
        ("a byte that cannot begin a character", &[0x80]),
    ];

    let text = String::from_utf8(mission.canonical()).ok()?;
    let lines: Vec<String> = text.lines().map(ToString::to_string).collect();
    let at = rng.below(u32::try_from(lines.len()).ok()?) as usize;
    let (kind, bytes) = *rng.pick(&CLASSES);

    // Inside the line rather than at either end of it, so no framing rule can
    // be read as governing instead of R22.
    let line = lines[at].as_bytes();
    let into = rng.below(u32::try_from(line.len()).ok()? + 1) as usize;
    let mut broken = Vec::new();
    broken.extend_from_slice(&line[..into]);
    broken.extend_from_slice(bytes);
    broken.extend_from_slice(&line[into..]);

    let mut rendered = Vec::new();
    for (number, other) in lines.iter().enumerate() {
        if number == at {
            rendered.extend_from_slice(&broken);
        } else {
            rendered.extend_from_slice(other.as_bytes());
        }
        rendered.push(b'\n');
    }

    check(&rendered, &Invalidity::Grammar, contract)?;
    Some(Mutation {
        kind,
        rendered,
        expectation: Expectation::At(at + 1, vec!["R22"]),
    })
}

/// Two violations, on two different robots, each of which stands on its own.
///
/// R25's rationale excuses a violation that can only be judged against another
/// invalid line, so both defects here are ones that need nothing else: a
/// coordinate past the declared limit is past it whatever the world says, and
/// a letter outside the instruction vocabulary is outside it whatever else is
/// wrong. The two are governed by different rulings on purpose — one tag
/// cannot answer for both, so a program that reports its first problem and
/// stops is caught by the tags as well as by the line numbers.
fn break_two_blocks(rng: &mut Rng, mission: &Mission, contract: &Contract) -> Option<Mutation> {
    let text = String::from_utf8(mission.canonical()).ok()?;
    let mut lines: Vec<String> = text.lines().map(ToString::to_string).collect();

    // Two different robots, so the defects cannot land on one line.
    let count = u32::try_from(mission.robots.len()).ok()?;
    let first = rng.below(count) as usize;
    let second = (first + 1 + rng.below(count - 1) as usize) % mission.robots.len();

    let over = contract.limits.max_coordinate + 1;
    let position = 1 + first * 2;
    let robot = &mission.robots[first];
    lines[position] = format!("{over} {} {}", robot.y, robot.facing);

    // An orientation letter, which the grammar guarantees is not an
    // instruction: the two vocabularies are disjoint, and reading the letter
    // out of the contract beats writing down one that happens to be wrong.
    let instructions = 2 + second * 2;
    let outside = *rng.pick(&contract.grammar.orientations);
    lines[instructions] = format!("{}{outside}", lines[instructions]);

    let rendered = format!("{}\n", lines.join("\n")).into_bytes();
    check(&rendered, &Invalidity::Grammar, contract)?;

    Some(Mutation {
        kind: "two independent problems, on two robots",
        rendered,
        expectation: Expectation::Several(vec![
            (position + 1, vec!["R5", "R1"]),
            (instructions + 1, vec!["R7", "R12"]),
        ]),
    })
}

/// Break the framing: a line removed, a line inserted, an ending that ends
/// nothing.
///
/// This is where a parser that resynchronises invents a line number, and where
/// R13, R18 and R19 decide whether one invisible byte flips a rejection into
/// an accepted mission. The expectation is still derived from the edit:
/// deleting line k means the content that followed it now sits *at* k, so that
/// is the line a diagnostic should name.
fn break_the_framing(rng: &mut Rng, mission: &Mission, contract: &Contract) -> Option<Mutation> {
    let canonical = mission.canonical();
    let text = String::from_utf8(canonical.clone()).ok()?;
    let lines: Vec<String> = text.lines().map(ToString::to_string).collect();
    let robots = mission.robots.len();
    let robot = rng.below(u32::try_from(robots).ok()?) as usize;
    let rejoin = |lines: &[String]| format!("{}\n", lines.join("\n")).into_bytes();

    let (kind, expectation, invalidity, rendered) = match rng.below(5) {
        // Stop after a position line. R18 is what keeps this a rejection:
        // without it the implicit ending would invent the blank line that
        // makes this a robot with no instructions.
        0 => {
            let keep = 2 + robot * 2;
            (
                "input that stops after a position line",
                Expectation::At(keep, vec!["R13"]),
                Invalidity::Grammar,
                rejoin(&lines[..keep]),
            )
        }
        // A carriage return after a terminated line, which R19's guard refuses
        // to complete into one. Which line carries it is contestable.
        1 => {
            let mut rendered = canonical.clone();
            rendered.push(b'\r');
            (
                "a carriage return that would make a line out of nothing",
                Expectation::Tagged(vec!["R19", "R13"]),
                Invalidity::Grammar,
                rendered,
            )
        }
        // A carriage return inside a line, ending nothing.
        2 => {
            let at = 1 + rng.below(u32::try_from(lines.len()).ok()? - 1) as usize;
            let mut broken = lines.clone();
            broken[at] = format!("\r{}", broken[at]);
            (
                "a carriage return that ends nothing",
                Expectation::At(at + 1, vec!["R19", "R11", "R12"]),
                Invalidity::Grammar,
                rejoin(&broken),
            )
        }
        // Remove an instruction line from the middle: the position line that
        // followed it is now read as instructions, where it now falls.
        3 => {
            if robot + 1 >= robots {
                return None;
            }
            let drop = 2 + robot * 2;
            let mut kept = lines.clone();
            kept.remove(drop);
            (
                "an instruction line removed from the middle",
                // Removing a line shifts every block after it, so the last
                // robot is left without an instruction line: R13 governs as
                // well, and §2.5 asks for one governing tag rather than a
                // particular one.
                Expectation::At(drop + 1, vec!["R7", "R12", "R13"]),
                Invalidity::Semantic,
                rejoin(&kept),
            )
        }
        // Insert a blank between a position line and its instructions: the
        // blank is the instruction line (R2), and the instructions that
        // followed are read as a position line where they now fall.
        _ => {
            let put = 2 + robot * 2;
            let mut kept = lines.clone();
            kept.insert(put, String::new());
            (
                "a blank line inserted before an instruction line",
                // The same shift, and the same consequence: what was an
                // instruction line is now a position line, and the block that
                // was last has nothing left to say.
                Expectation::At(put + 2, vec!["R12", "R7", "R13"]),
                Invalidity::Grammar,
                rejoin(&kept),
            )
        }
    };

    check(&rendered, &invalidity, contract)?;

    Some(Mutation {
        kind,
        rendered,
        expectation,
    })
}

/// Break what a token means
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
        0 => {
            lines[0] = format!("{} {}", max_coordinate + 1 + rng.below(9), mission.max_y);
            Broken {
                kind: "a grid coordinate over the limit",
                at: 0,
                tags: vec!["R5"],
                invalidity: Invalidity::Semantic,
            }
        }
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
                invalidity: Invalidity::Grammar,
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

/// Break the shape of a line
fn break_the_shape(
    rng: &mut Rng,
    mission: &Mission,
    lines: &mut [String],
    robot: usize,
    contract: &Contract,
) -> Broken {
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
                invalidity: Invalidity::Grammar,
            }
        }
        _ => {
            // Drawn from what the grammar does *not* admit, so admitting a
            // character in `ws` stops it being injected here.
            let foreign = *rng.pick(&contract.grammar.not_separators());
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

fn check(rendered: &[u8], invalidity: &Invalidity, contract: &Contract) -> Option<()> {
    let refused = match (invalidity, Mission::read_back(rendered, &contract.grammar)) {
        (Invalidity::Grammar, read) => read.is_err(),
        (Invalidity::Semantic | Invalidity::Either, Ok(mission)) => !mission.is_valid(contract),
        (Invalidity::Semantic | Invalidity::Either, Err(_)) => true,
    };
    refused.then_some(())
}

#[cfg(test)]
mod tests {
    use super::{Expectation, mutate};
    use crate::contract::Contract;
    use crate::mission::Mission;
    use crate::rng::Rng;

    #[test]
    fn every_mutation_leaves_input_the_contract_refuses() {
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(20_260_922);
        let mut built = 0;
        for _ in 0..2_000 {
            let mission = Mission::draw(&mut rng, &contract);
            if let Some(mutation) = mutate(&mut rng, &mission, &contract) {
                built += 1;
                let valid = Mission::read_back(&mutation.rendered, &contract.grammar)
                    .is_ok_and(|read| read.is_valid(&contract));
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
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(4);
        for _ in 0..2_000 {
            let mission = Mission::draw(&mut rng, &contract);
            let canonical = String::from_utf8(mission.canonical()).unwrap();
            let Some(mutation) = mutate(&mut rng, &mission, &contract) else {
                continue;
            };
            let before: Vec<&str> = canonical.lines().collect();
            // A mutation that injects a byte which is not text has no line
            // comparison to make; the rule that built it names the line.
            let Ok(after) = String::from_utf8(mutation.rendered.clone()) else {
                continue;
            };
            let after: Vec<&str> = after.lines().collect();
            // A framing mutation changes how many lines there are, so a
            // positional comparison says nothing about it; those are checked
            // by the rule that built them.
            let Expectation::At(line, _) = &mutation.expectation else {
                continue;
            };
            if before.len() != after.len() {
                continue;
            }
            let changed: Vec<usize> = before
                .iter()
                .zip(&after)
                .enumerate()
                .filter(|(_, (was, is))| was != is)
                .map(|(at, _)| at + 1)
                .collect();
            assert_eq!(
                changed,
                vec![*line],
                "{} says line {line} but changed {changed:?}",
                mutation.kind
            );
        }
    }

    #[test]
    fn every_kind_of_mutation_gets_built() {
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(88);
        let mut kinds = std::collections::BTreeSet::new();
        for _ in 0..4_000 {
            let mission = Mission::draw(&mut rng, &contract);
            if let Some(mutation) = mutate(&mut rng, &mission, &contract) {
                kinds.insert(mutation.kind);
            }
        }
        assert_eq!(kinds.len(), 35, "only built {kinds:?}");
    }

    #[test]
    fn no_mutation_offers_a_ruling_that_cannot_govern_where_it_landed() {
        // R7's question is scoped to an instruction string or an orientation.
        // A grid line carries neither, so offering R7 there lets a program
        // answer with a ruling that does not govern and still pass - which is
        // the same defect as demanding the wrong ruling, in the other
        // direction.
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(101);
        let mut checked = 0;
        for _ in 0..4_000 {
            let mission = Mission::draw(&mut rng, &contract);
            let Some(mutation) = mutate(&mut rng, &mission, &contract) else {
                continue;
            };
            if !mutation.kind.ends_with("the grid line") {
                continue;
            }
            checked += 1;
            let tags: Vec<&&str> = match &mutation.expectation {
                Expectation::At(_, tags) | Expectation::Tagged(tags) => tags.iter().collect(),
                Expectation::Several(demands) => {
                    demands.iter().flat_map(|(_, tags)| tags).collect()
                }
            };
            assert!(
                !tags.contains(&&"R7"),
                "{:?} offers R7 on a line with no vocabulary token",
                mutation.kind
            );
        }
        assert!(checked > 0, "no mutation landed on the grid line");
    }
}
