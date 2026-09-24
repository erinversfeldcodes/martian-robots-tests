use std::path::Path;

use crate::contract::Contract;
use crate::expect::{Diagnostic, Expect, show};
use crate::mission::Mission;
use crate::mutation;
use crate::properties;
use crate::reference;
use crate::rng::Rng;
use crate::run::{TIMEOUT, observe};
use crate::spelling::{self, Spelling};

pub struct Budget {
    pub missions: u32,
    pub spellings: u32,
    pub properties: u32,
    pub rejections: u32,
    pub differential: u32,
    pub seed: u64,
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
) -> Result<Vec<Divergence>, String> {
    let mut rng = Rng::from_seed(budget.seed);
    let mut divergences = Vec::new();

    for _ in 0..budget.missions {
        let mission = Mission::draw(&mut rng, contract);
        let canonical = mission.canonical();
        let expected = answer(implementation, &canonical)?;

        for _ in 1..budget.spellings {
            let rendered = spelling::render(&mission, &Spelling::draw(&mut rng, &contract.grammar));

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
        let mission = Mission::draw_shaped(&mut rng, contract);
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

        let longer = mission.with_another_robot(&mut rng, contract);
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

        let expect = Expect::Rejection(Diagnostic::at_line(mutation.line, &mutation.tags));
        let seen = observe(implementation, &[], &mutation.rendered, TIMEOUT)?;
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

pub struct Disagreement {
    pub mission: Vec<u8>,
    pub expected: Vec<u8>,
    pub got: Vec<u8>,
}

/// Compare answers with a second implementation written from the same
/// contract.
pub fn differential(
    implementation: &Path,
    contract: &Contract,
    budget: &Budget,
) -> Result<Vec<Disagreement>, String> {
    let mut rng = Rng::from_seed(budget.seed ^ 0x4469_6666);
    let mut disagreements = Vec::new();

    for _ in 0..budget.differential {
        let mission = Mission::draw_busy(&mut rng, contract);
        if !mission.is_valid(
            contract.limits.max_coordinate,
            contract.limits.max_instructions,
        ) {
            return Err(format!(
                "the generator built an invalid mission: {}",
                show(&mission.canonical())
            ));
        }

        let input = mission.canonical();
        let expected = reference::run(&mission);
        let got = answer(implementation, &input)?;
        if got != expected {
            disagreements.push(Disagreement {
                mission: input,
                expected,
                got,
            });
        }
    }

    Ok(disagreements)
}
