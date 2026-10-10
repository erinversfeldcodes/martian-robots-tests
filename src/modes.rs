use std::path::Path;
use std::time::Duration;

use crate::contract::Contract;
use crate::expect::{Diagnostic, Expect, show};
use crate::mission::Mission;
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

const CROSS_RUN: [&str; 2] = [
    "the same input twice gives the same answer",
    "appending a robot does not change the robots before it",
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

        for (index, verdict) in properties::check(&mission, &said).into_iter().enumerate() {
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

        let at = properties::NAMES.len();
        let again = answer(implementation, &input, budget.timeout)?.unwrap_or_default();
        fired[at] += 1;
        if again != said {
            violations.push(format!(
                "{}: {} then {}\n      on {}",
                names[at],
                show(&said),
                show(&again),
                show(&input)
            ));
        }

        // Only meaningful when there is a prefix to preserve: with no robots
        // the comparison is empty against empty, and counting it would
        // overstate what was judged.
        if mission.robots.is_empty() {
            continue;
        }
        let longer = mission.with_another_robot(&mut rng, contract);
        let longer_input = write(&longer, spelling.as_ref(), contract)?;
        if !longer.every_robot_starts_on_the_grid() {
            return Err(format!(
                "the generator built an invalid mission: {}",
                show(&longer.canonical())
            ));
        }
        {
            fired[at + 1] += 1;
            let extended =
                answer(implementation, &longer_input, budget.timeout)?.unwrap_or_default();
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
                    show(&said),
                    show(&kept),
                    show(&longer_input)
                ));
            }
        }
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
            properties::check(candidate, &said).get(predicate),
            Some(properties::Verdict::Violated(_))
        ))
    })?;
    Ok(smaller.mission.canonical())
}
