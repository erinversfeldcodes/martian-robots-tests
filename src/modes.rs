use std::path::Path;

use crate::contract::Contract;
use crate::expect::show;
use crate::mission::Mission;
use crate::rng::Rng;
use crate::run::{TIMEOUT, observe};
use crate::spelling::{self, Spelling};

pub struct Budget {
    pub missions: u32,
    pub spellings: u32,
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
