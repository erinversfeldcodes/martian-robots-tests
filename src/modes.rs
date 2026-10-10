use std::path::Path;
use std::time::Duration;

use crate::contract::Contract;
use crate::expect::{Diagnostic, Expect, show};
use crate::mission::{Mission, Robot};
use crate::mutation;
use crate::properties;
use crate::rng::Rng;
use crate::run::observe;
use crate::shrink;
use crate::spelling::{self, Spelling};

pub struct Budget {
    /// Grader policy under Q3, so it is a flag rather than a constant.
    pub timeout: Duration,
    pub missions: u32,
    pub spellings: u32,
    pub properties: u32,
    pub rejections: u32,
    pub seed: u64,
}

pub struct SpellingRun {
    pub divergences: Vec<Divergence>,
    /// Valid missions the program would not answer at all. Not a divergence -
    /// nothing was compared - but not a pass either.
    pub refused: Vec<Vec<u8>>,
    /// Respellings actually compared against a canonical answer. Zero of these
    /// with a budget above zero means the mode reported on nothing.
    pub compared: usize,
}

pub struct Divergence {
    pub mission: Vec<u8>,
    pub against: Vec<u8>,
    pub expected: Vec<u8>,
    pub got: Vec<u8>,
}

/// Require a program to agree with itself.
pub fn spelling_differential(
    implementation: &Path,
    contract: &Contract,
    budget: &Budget,
) -> Result<SpellingRun, String> {
    let mut rng = Rng::from_seed(budget.seed);
    let mut divergences = Vec::new();
    let mut compared = 0;
    let mut refused = Vec::new();

    for _ in 0..budget.missions {
        let mission = Mission::draw(&mut rng, contract);
        let canonical = mission.canonical();
        let Some(expected) = answer(implementation, &canonical, budget.timeout)? else {
            refused.push(canonical);
            continue;
        };

        for _ in 1..budget.spellings {
            let rendered = spelling::render(&mission, &Spelling::draw(&mut rng, &contract.grammar));

            // The generator checks itself on every run, not only in its own
            // tests. A spelling that broke the grammar would turn this gate
            // into a rejection test - weaker, and still green - and one that
            // changed the mission would blame a program for the generator's
            // mistake.
            spelling::permitted(&rendered)
                .map_err(|why| format!("the generator emitted {why}: {}", show(&rendered)))?;
            if Mission::read_back(&rendered, &contract.grammar).as_ref() != Ok(&mission) {
                return Err(format!(
                    "the generator changed the mission it was rendering: {}",
                    show(&rendered)
                ));
            }

            compared += 1;
            let got = answer(implementation, &rendered, budget.timeout)?
                .unwrap_or_else(|| b"<refused a spelling of a mission it answered>".to_vec());
            if got != expected {
                divergences.push(Divergence {
                    mission: canonical.clone(),
                    against: rendered,
                    expected: expected.clone(),
                    got,
                });
            }
        }
    }

    Ok(SpellingRun {
        divergences,
        refused,
        compared,
    })
}

/// What the program said about one rendering, or `None` if it refused it.
///
/// A drawn mission is valid by construction, so a refusal is the program
/// answering a question these modes did not ask. It used to be folded into a
/// sentinel string and compared like any other answer, which meant a program
/// that refused *everything* agreed with itself perfectly and the spelling
/// mode reported no divergences. Refusals are now visible to each caller,
/// which decides what they mean.
fn answer(
    implementation: &Path,
    input: &[u8],
    timeout: Duration,
) -> Result<Option<Vec<u8>>, String> {
    let seen = observe(implementation, &[], input, timeout)?;
    Ok(seen.exited_zero().then_some(seen.stdout))
}

pub struct PropertyRun {
    pub names: Vec<&'static str>,
    pub fired: Vec<u32>,
    pub violations: Vec<String>,
    pub missions: u32,
}

/// Every predicate the properties mode reports on, in the order it reports
/// them. Exposed so a documentation test can check the README lists all of
/// them: two of these live here rather than in `properties`, and watching only
/// that module left them free to be renamed out from under the README.
pub fn predicate_names() -> Vec<&'static str> {
    properties::NAMES.iter().copied().chain(CROSS_RUN).collect()
}

const CROSS_RUN: [&str; 3] = [
    "the same input twice gives the same answer",
    "appending a robot does not change the robots before it",
    "running a robot's instructions in two halves ends where running them whole does",
];

/// Check the answers against statements that need no second implementation.
pub fn properties(
    implementation: &Path,
    contract: &Contract,
    budget: &Budget,
) -> Result<PropertyRun, String> {
    let mut rng = Rng::from_seed(budget.seed ^ 0x5072_6F70);
    let names = predicate_names();
    let mut fired = vec![0; names.len()];
    let mut violations = Vec::new();

    for round in 0..budget.properties {
        let mission = Mission::draw_shaped(&mut rng, contract);

        // Half of them arrive respelled. A mission is the same mission however
        // its whitespace and line endings are written, so every predicate
        // holds over a respelling as it does over the canonical form.
        //
        // The spelling mode already compares a respelling against the
        // program's own canonical answer, and it would see most readers that
        // mishandle a tab. What it does not do is ask the two cross-run
        // questions of a respelled input: whether an unusual spelling is
        // answered the same way twice, and whether appending a robot written
        // that way disturbs the robots before it. Those are where a reader
        // that keeps state across a parse, or scans further than it should,
        // shows up. The rest is a wider respelled corpus, which is worth
        // having for its own sake.
        let spelling = (round % 2 == 1).then(|| Spelling::draw(&mut rng, &contract.grammar));
        let input = write(&mission, spelling.as_ref(), contract)?;

        let Some(said) = answer(implementation, &input, budget.timeout)? else {
            violations.push(format!(
                "a valid mission was refused, so nothing can be said about it\n      on {}",
                show(&input)
            ));
            continue;
        };

        for (index, verdict) in properties::check(&mission, &said, contract)
            .into_iter()
            .enumerate()
        {
            match verdict {
                properties::Verdict::NotApplicable => {}
                properties::Verdict::Held => fired[index] += 1,
                properties::Verdict::Violated(why) => {
                    fired[index] += 1;
                    // The first violation is reduced before it is reported. A
                    // drawn mission is a dozen robots and several hundred
                    // bytes, which reproduces a violation without explaining
                    // it; only the first, because each candidate costs a
                    // process and the rest of the list says how widespread
                    // the problem is.
                    let reduced = if violations.is_empty() {
                        let smaller =
                            reduce(implementation, contract, &mission, index, budget.timeout)?;
                        format!("\n      reduced to {}", show(&smaller))
                    } else {
                        String::new()
                    };
                    violations.push(format!(
                        "{}: {why}\n      on {}{reduced}",
                        names[index],
                        show(&input)
                    ));
                }
            }
        }

        repeat_and_extend(
            implementation,
            contract,
            &mission,
            &said,
            &input,
            spelling.as_ref(),
            &mut rng,
            budget,
            &mut fired,
            &mut violations,
            &names,
        )?;
    }

    Ok(PropertyRun {
        names,
        fired,
        violations,
        missions: budget.properties,
    })
}

/// A mission as bytes, canonically or in the spelling given, with the
/// generator checking its own work either way: a rendering that broke the
/// grammar would turn an invariant into a rejection test, and one that changed
/// the mission would blame a program for the generator's mistake.
fn write(
    mission: &Mission,
    spelling: Option<&Spelling>,
    contract: &Contract,
) -> Result<Vec<u8>, String> {
    let Some(spelling) = spelling else {
        return Ok(mission.canonical());
    };
    let rendered = spelling::render(mission, spelling);
    spelling::permitted(&rendered)
        .map_err(|why| format!("the generator emitted {why}: {}", show(&rendered)))?;
    if Mission::read_back(&rendered, &contract.grammar).as_ref() != Ok(mission) {
        return Err(format!(
            "the generator changed the mission it was rendering: {}",
            show(&rendered)
        ));
    }
    Ok(rendered)
}

pub struct RejectionRun {
    pub run: u32,
    pub failures: Vec<String>,
}

pub fn rejections(
    implementation: &Path,
    contract: &Contract,
    budget: &Budget,
) -> Result<RejectionRun, String> {
    let mut rng = Rng::from_seed(budget.seed ^ 0x5265_6A65);
    let mut failures = Vec::new();
    let mut run = 0;

    for _ in 0..budget.rejections {
        let mission = Mission::draw(&mut rng, contract);
        let Some(mutation) = mutation::mutate(&mut rng, &mission, contract) else {
            continue;
        };
        run += 1;

        let expect = Expect::Rejection(match &mutation.expectation {
            mutation::Expectation::At(line, tags) => Diagnostic::at_line(*line, tags),
            mutation::Expectation::Tagged(tags) => Diagnostic::tagged(tags),
            mutation::Expectation::Several(demands) => {
                let demands: Vec<(usize, &[&str])> = demands
                    .iter()
                    .map(|(line, tags)| (*line, tags.as_slice()))
                    .collect();
                Diagnostic::at_lines(&demands)
            }
        });
        let seen = observe(implementation, &[], &mutation.rendered, budget.timeout)?;
        if let Some(why) = expect.judge(&seen) {
            failures.push(format!(
                "{}: {why}\n      on {}",
                mutation.kind,
                show(&mutation.rendered)
            ));
        }
    }

    Ok(RejectionRun { run, failures })
}

/// The two cross-run statements that predate the decomposition one: the same
/// input twice gives the same answer, and appending a robot leaves the robots
/// before it alone.
#[expect(clippy::too_many_arguments, reason = "one call site, all of it needed")]
fn repeat_and_extend(
    implementation: &Path,
    contract: &Contract,
    mission: &Mission,
    said: &[u8],
    input: &[u8],
    spelling: Option<&Spelling>,
    rng: &mut Rng,
    budget: &Budget,
    fired: &mut [u32],
    violations: &mut Vec<String>,
    names: &[&'static str],
) -> Result<(), String> {
    let at = properties::NAMES.len();
    let again = answer(implementation, input, budget.timeout)?.unwrap_or_default();
    fired[at] += 1;
    if again != said {
        violations.push(format!(
            "{}: {} then {}\n      on {}",
            names[at],
            show(said),
            show(&again),
            show(input)
        ));
    }

    decompose(
        implementation,
        contract,
        mission,
        rng,
        budget.timeout,
        at + 2,
        fired,
        violations,
        names,
    )?;

    // Only meaningful when there is a prefix to preserve: with no robots the
    // comparison is empty against empty, and counting it would overstate what
    // was judged.
    if mission.robots.is_empty() {
        return Ok(());
    }
    let longer = mission.with_another_robot(rng, contract);
    let longer_input = write(&longer, spelling, contract)?;
    if !longer.every_robot_starts_on_the_grid() {
        return Err(format!(
            "the generator built an invalid mission: {}",
            show(&longer.canonical())
        ));
    }
    {
        fired[at + 1] += 1;
        let extended = answer(implementation, &longer_input, budget.timeout)?.unwrap_or_default();
        let before = said.split_inclusive(|&byte| byte == b'\n').count();
        let kept: Vec<u8> = extended
            .split_inclusive(|&byte| byte == b'\n')
            .take(before)
            .flatten()
            .copied()
            .collect();
        if kept != said {
            violations.push(format!(
                "{}: {} became {}\n      on {}",
                names[at + 1],
                show(said),
                show(&kept),
                show(&longer_input)
            ));
        }
    }

    Ok(())
}

/// Run one robot's instructions whole, then in two halves, and require the
/// same ending.
///
/// Nothing in the single-answer battery constrains what a *mixed* instruction
/// string means. Six of the eight predicates say nothing about movement at
/// all, and the two that do exclude any string containing both a turn and an
/// `F`. So a program that is wrong only about where a robot ends up — the
/// ordinary off-by-one, or a bug that bites past some instruction length —
/// agrees with itself across every respelling, satisfies every invariant, and
/// passes.
///
/// This needs no second implementation to say otherwise. Split a single
/// robot's instructions at a point, run the first half, place a robot where
/// that answer reported, run the second half: the two must agree, because the
/// state at the seam is the same state. It holds only when the first half does
/// not lose the robot — a loss ends the run and leaves a scent the split
/// cannot reproduce — so that case is skipped rather than judged.
///
/// A length-dependent defect fails this immediately: the whole string is
/// longer than either half.
#[expect(clippy::too_many_arguments, reason = "one call site, all of it needed")]
fn decompose(
    implementation: &Path,
    contract: &Contract,
    mission: &Mission,
    rng: &mut Rng,
    timeout: Duration,
    index: usize,
    fired: &mut [u32],
    violations: &mut Vec<String>,
    names: &[&'static str],
) -> Result<(), String> {
    // Its own draw, not the one the single-answer predicates run against.
    // That corpus deliberately degenerates most instruction strings to all
    // turns or all `F`, because that is what makes the two meaning-predicates
    // apply — and it is exactly the wrong shape here, where the whole point
    // is a string mixing both. `draw_busy` draws uniformly, which is what the
    // removed differential mode used and what nothing used after it went.
    //
    // One robot, so no scent from anybody else can reach it.
    let drawn = Mission::draw_busy(rng, contract);
    let Some(robot) = drawn.robots.first() else {
        return Ok(());
    };
    if robot.instructions.len() < 2 {
        return Ok(());
    }
    let alone = Mission {
        max_x: drawn.max_x,
        max_y: drawn.max_y,
        robots: vec![robot.clone()],
    };
    let _ = mission;

    let at = 1 + rng.below(u32::try_from(robot.instructions.len() - 1).unwrap_or(1)) as usize;
    let (first, second) = robot.instructions.split_at(at);

    let whole = answer(implementation, &alone.canonical(), timeout)?;
    let Some(whole) = whole else {
        return Ok(());
    };

    let mut half = alone.clone();
    half.robots[0].instructions = first.to_string();
    let Some(after) = answer(implementation, &half.canonical(), timeout)? else {
        return Ok(());
    };

    // A lost robot ends its run, and leaves a scent the split cannot carry.
    let Some(resumed) = resume(&alone, &after, second) else {
        return Ok(());
    };
    let Some(rest) = answer(implementation, &resumed.canonical(), timeout)? else {
        return Ok(());
    };

    fired[index] += 1;
    if rest == whole {
        return Ok(());
    }

    // The instruction string is what matters here and it can be ninety-nine
    // characters long, so the report is reduced the way a violation is: the
    // shortest string whose halves still disagree with the whole.
    let smaller = shrink::shrink(&alone, contract, |candidate| {
        let Some(robot) = candidate.robots.first() else {
            return Ok(false);
        };
        if robot.instructions.len() < 2 {
            return Ok(false);
        }
        let split = robot.instructions.len() / 2;
        let (first, second) = robot.instructions.split_at(split);
        let Some(whole) = answer(implementation, &candidate.canonical(), timeout)? else {
            return Ok(false);
        };
        let mut half = candidate.clone();
        half.robots[0].instructions = first.to_string();
        let Some(after) = answer(implementation, &half.canonical(), timeout)? else {
            return Ok(false);
        };
        let Some(resumed) = resume(candidate, &after, second) else {
            return Ok(false);
        };
        let Some(rest) = answer(implementation, &resumed.canonical(), timeout)? else {
            return Ok(false);
        };
        Ok(rest != whole)
    })?;

    violations.push(format!(
        "{}: {:?} then {:?} gave {}, and {:?} whole gave {}\n      on {}\n      reduced to {}",
        names[index],
        first,
        second,
        show(&rest),
        robot.instructions,
        show(&whole),
        show(&alone.canonical()),
        show(&smaller.mission.canonical())
    ));
    Ok(())
}

/// The mission that continues from where an answer left off, or `None` when
/// the answer says the robot is gone or does not parse.
fn resume(mission: &Mission, after: &[u8], rest: &str) -> Option<Mission> {
    let line = std::str::from_utf8(after).ok()?.lines().next()?;
    if line.ends_with(" LOST") {
        return None;
    }
    let [x, y, facing] = line.split(' ').collect::<Vec<&str>>()[..] else {
        return None;
    };
    let mut resumed = mission.clone();
    resumed.robots[0] = Robot {
        x: x.parse().ok()?,
        y: y.parse().ok()?,
        facing: facing.chars().next()?,
        instructions: rest.to_string(),
    };
    Some(resumed)
}

/// Reduce a mission that violates a predicate to the smallest one that still
/// violates the same predicate.
///
/// A drawn mission is a dozen robots and several hundred bytes, which
/// reproduces a violation without explaining it. Only the first violation in a
/// run is reduced, because each candidate costs a process.
fn reduce(
    implementation: &Path,
    contract: &Contract,
    mission: &Mission,
    predicate: usize,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let smaller = shrink::shrink(mission, contract, |candidate| {
        let Some(said) = answer(implementation, &candidate.canonical(), timeout)? else {
            // A refused valid mission is its own failure, not this one.
            return Ok(false);
        };
        Ok(matches!(
            properties::check(candidate, &said, contract).get(predicate),
            Some(properties::Verdict::Violated(_))
        ))
    })?;
    Ok(smaller.mission.canonical())
}
