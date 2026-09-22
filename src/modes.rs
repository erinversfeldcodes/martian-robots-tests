use std::path::Path;

use crate::contract::Contract;
use crate::expect::show;
use crate::mission::Mission;
use crate::properties;
use crate::rng::Rng;
use crate::run::{TIMEOUT, observe};
use crate::spelling::{self, Spelling};

pub struct Budget {
    pub missions: u32,
    pub spellings: u32,
    pub properties: u32,
    pub seed: u64,
}

pub struct Divergence {
    pub mission: Vec<u8>,
    pub against: Vec<u8>,
    pub expected: Vec<u8>,
    pub got: Vec<u8>,
}

/// Require a program to agree with itself.
///
/// The contract rules a mission's framing meaningless: whitespace runs, line
/// endings, the final end-of-line, blank separators, leading zeros. So every
/// legal spelling of one mission must produce the same bytes. No reference
/// implementation is needed to ask that question — which makes this the one
/// generated mode that still bites when the suite and the program under test
/// share a wrong belief.
pub fn spelling_differential(
    implementation: &Path,
    contract: &Contract,
    budget: &Budget,
) -> Result<Vec<Divergence>, String> {
    let mut rng = Rng::from_seed(budget.seed);
    let mut divergences = Vec::new();

    for _ in 0..budget.missions {
        let mission = Mission::draw(&mut rng, contract.limits.max_coordinate);
        let canonical = mission.canonical();
        let expected = answer(implementation, &canonical)?;

        for _ in 1..budget.spellings {
            let rendered = spelling::render(&mission, &Spelling::draw(&mut rng));

            // The generator checks itself on every run, not only in its own
            // tests. A spelling that broke the grammar would turn this gate
            // into a rejection test - weaker, and still green - and one that
            // changed the mission would blame a program for the generator's
            // mistake.
            spelling::permitted(&rendered)
                .map_err(|why| format!("the generator emitted {why}: {}", show(&rendered)))?;
            if Mission::read_back(&rendered).as_ref() != Ok(&mission) {
                return Err(format!(
                    "the generator changed the mission it was rendering: {}",
                    show(&rendered)
                ));
            }

            let got = answer(implementation, &rendered)?;
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

    Ok(divergences)
}

/// What the program says about one rendering. A drawn mission is valid by
/// construction, so anything but a clean exit is the program's answer to a
/// question this mode did not mean to ask, and is reported as a divergence
/// rather than silently compared.
fn answer(implementation: &Path, input: &[u8]) -> Result<Vec<u8>, String> {
    let seen = observe(implementation, &[], input, TIMEOUT)?;
    if seen.exited_zero() {
        Ok(seen.stdout)
    } else {
        Ok(format!("<refused a valid mission: {:?}>", seen.ending).into_bytes())
    }
}

pub struct PropertyRun {
    pub names: Vec<&'static str>,
    /// How many missions each property actually evaluated. A predicate that
    /// never fires reads exactly like one that always holds, which is how a
    /// mode can report green while proving nothing.
    pub fired: Vec<u32>,
    pub violations: Vec<String>,
    pub missions: u32,
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
    let names: Vec<&'static str> = properties::NAMES.iter().copied().chain(CROSS_RUN).collect();
    let mut fired = vec![0; names.len()];
    let mut violations = Vec::new();

    for _ in 0..budget.properties {
        let mission = Mission::draw_shaped(&mut rng, contract.limits.max_coordinate);
        let input = mission.canonical();
        let said = answer(implementation, &input)?;

        for (index, verdict) in properties::check(&mission, &said).into_iter().enumerate() {
            match verdict {
                properties::Verdict::NotApplicable => {}
                properties::Verdict::Held => fired[index] += 1,
                properties::Verdict::Violated(why) => {
                    fired[index] += 1;
                    violations.push(format!(
                        "{}: {why}\n      on {}",
                        names[index],
                        show(&input)
                    ));
                }
            }
        }

        let at = properties::NAMES.len();
        let again = answer(implementation, &input)?;
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

        let longer = mission.with_another_robot(&mut rng);
        if !longer.every_robot_starts_on_the_grid() {
            return Err(format!(
                "the generator built an invalid mission: {}",
                show(&longer.canonical())
            ));
        }
        {
            fired[at + 1] += 1;
            let extended = answer(implementation, &longer.canonical())?;
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
                    show(&longer.canonical())
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
