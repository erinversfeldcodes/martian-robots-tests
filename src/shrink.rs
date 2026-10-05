//! A failing mission, made small enough to read.
//!
//! A generated failure arrives as whatever the draw produced: a dozen robots
//! on a fifty-cell world, several with ninety-nine instructions. That
//! reproduces the bug and explains nothing. What a reader needs is the
//! smallest mission that still fails, because the parts that can be removed
//! without the failure going away are the parts the bug does not depend on.
//!
//! The reduction is greedy and deterministic: try the candidates in order of
//! how much they remove, keep the first that still fails, and start again.
//! Nothing here decides *whether* a mission fails — that answer always comes
//! from running the program again — so shrinking can narrow a report but never
//! manufacture one.
//!
//! Two disciplines make the result trustworthy. Every candidate is a valid
//! mission, checked before it is offered, so a reduction cannot turn a wrong
//! answer into a rejection and call it the same failure. And the caller's
//! predicate is asked about one specific failure rather than failure in
//! general, so a mission cannot shrink its way into a different bug.

use crate::contract::Contract;
use crate::mission::Mission;

/// How many times a program may be run to reduce one failure.
///
/// Every candidate costs a process, so this is a budget rather than a search.
/// A crowded mission offers around a hundred candidates per pass and several
/// passes are wanted, so this allows four of them: enough to take a dozen
/// robots down to the one that matters and trim its instructions to the step
/// the bug needs. Only one failure per run is reduced, so this is also the
/// whole cost.
pub const ATTEMPTS: usize = 400;

pub struct Reduced {
    pub mission: Mission,
    pub attempts: usize,
    /// Whether the budget ran out before every candidate had been tried. The
    /// report is then smaller than what was drawn, but nothing here shows it
    /// could not be smaller still.
    pub exhausted: bool,
}

/// Reduce `mission` to the smallest one `still_fails` accepts.
///
/// `still_fails` runs the program under test, so it returns `Err` for the
/// grader's own problems — a process that would not start — and those abort the
/// reduction rather than being read as "no longer fails".
pub fn shrink<F>(
    mission: &Mission,
    contract: &Contract,
    mut still_fails: F,
) -> Result<Reduced, String>
where
    F: FnMut(&Mission) -> Result<bool, String>,
{
    let mut smallest = mission.clone();
    let mut attempts = 0;
    let mut exhausted = false;

    'reducing: loop {
        for candidate in smaller(&smallest, contract) {
            if attempts >= ATTEMPTS {
                exhausted = true;
                break 'reducing;
            }
            attempts += 1;
            if still_fails(&candidate)? {
                smallest = candidate;
                continue 'reducing;
            }
        }
        break;
    }

    Ok(Reduced {
        mission: smallest,
        attempts,
        exhausted,
    })
}

/// Valid missions smaller than this one, biggest reduction first.
///
/// Order is what makes a greedy pass effective: dropping a robot removes more
/// than shortening one, and shortening by half removes more than shortening by
/// a step. A candidate that is not a valid mission is not offered at all.
fn smaller(mission: &Mission, contract: &Contract) -> Vec<Mission> {
    let mut candidates = Vec::new();

    // Half the robots, then one robot, then each robot in turn. The first two
    // are what turn a crowd into a pair in two attempts instead of ten.
    if mission.robots.len() > 2 {
        for keep in [mission.robots.len() / 2, 1] {
            let mut fewer = mission.clone();
            fewer.robots.truncate(keep);
            candidates.push(fewer);
        }
    }
    for at in 0..mission.robots.len() {
        let mut fewer = mission.clone();
        fewer.robots.remove(at);
        candidates.push(fewer);
    }

    // Shorter instructions, halved before trimmed. A bug that needs the tenth
    // step keeps it; one that needs only the first loses the other ninety-eight
    // in a handful of attempts.
    for at in 0..mission.robots.len() {
        let length = mission.robots[at].instructions.len();
        for keep in [length / 2, length.saturating_sub(1)] {
            if keep < length {
                let mut shorter = mission.clone();
                shorter.robots[at].instructions.truncate(keep);
                candidates.push(shorter);
            }
        }
    }

    // A smaller world, and robots nearer the origin. Both change what the
    // answer is, so they only survive when the failure did not depend on the
    // numbers being large.
    for (max_x, max_y) in [
        (mission.max_x / 2, mission.max_y / 2),
        (mission.max_x.saturating_sub(1), mission.max_y),
        (mission.max_x, mission.max_y.saturating_sub(1)),
    ] {
        if (max_x, max_y) != (mission.max_x, mission.max_y) {
            let mut nearer = mission.clone();
            nearer.max_x = max_x;
            nearer.max_y = max_y;
            candidates.push(nearer);
        }
    }
    for at in 0..mission.robots.len() {
        for (x, y) in [
            (mission.robots[at].x / 2, mission.robots[at].y / 2),
            (mission.robots[at].x.saturating_sub(1), mission.robots[at].y),
            (mission.robots[at].x, mission.robots[at].y.saturating_sub(1)),
        ] {
            if (x, y) != (mission.robots[at].x, mission.robots[at].y) {
                let mut nearer = mission.clone();
                nearer.robots[at].x = x;
                nearer.robots[at].y = y;
                candidates.push(nearer);
            }
        }
    }

    candidates.retain(|candidate| candidate.is_valid(contract));
    candidates
}

#[cfg(test)]
mod tests {
    use super::{ATTEMPTS, shrink, smaller};
    use crate::contract::Contract;
    use crate::mission::{Mission, Robot};
    use crate::rng::Rng;

    fn robot(x: u32, y: u32, facing: char, instructions: &str) -> Robot {
        Robot {
            x,
            y,
            facing,
            instructions: instructions.to_string(),
        }
    }

    #[test]
    fn a_crowd_reduces_to_the_one_robot_that_matters() {
        let contract = Contract::load().unwrap();
        let mut mission = Mission {
            max_x: 40,
            max_y: 40,
            robots: (0..12)
                .map(|at| robot(at, at, 'N', &"LR".repeat(40)))
                .collect(),
        };
        mission.robots[7].facing = 'S';

        // The failure is "somewhere in here there is a robot facing south".
        let reduced = shrink(&mission, &contract, |candidate| {
            Ok(candidate.robots.iter().any(|robot| robot.facing == 'S'))
        })
        .unwrap();

        assert_eq!(reduced.mission.robots.len(), 1);
        assert_eq!(reduced.mission.robots[0].facing, 'S');
        assert_eq!(reduced.mission.robots[0].instructions, "");
        assert!(!reduced.exhausted, "{} attempt(s)", reduced.attempts);
    }

    #[test]
    fn a_failure_that_needs_the_whole_mission_keeps_it() {
        let contract = Contract::load().unwrap();
        let mission = Mission {
            max_x: 5,
            max_y: 3,
            robots: vec![robot(1, 1, 'E', "RFRFRFRF"), robot(0, 3, 'W', "LL")],
        };
        let reduced = shrink(&mission, &contract, |candidate| Ok(*candidate == mission)).unwrap();
        assert_eq!(reduced.mission, mission);
    }

    #[test]
    fn a_mission_that_does_not_fail_is_returned_as_it_stands() {
        let contract = Contract::load().unwrap();
        let mission = Mission {
            max_x: 5,
            max_y: 3,
            robots: vec![robot(1, 1, 'E', "RF")],
        };
        let reduced = shrink(&mission, &contract, |_| Ok(false)).unwrap();
        assert_eq!(reduced.mission, mission);
    }

    #[test]
    fn a_predicate_that_always_fails_stops_at_the_budget() {
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(31);
        let mission = Mission::draw_busy(&mut rng, &contract);
        let reduced = shrink(&mission, &contract, |_| Ok(true)).unwrap();
        assert!(
            reduced.attempts <= ATTEMPTS,
            "{} attempt(s)",
            reduced.attempts
        );
    }

    #[test]
    fn the_graders_own_problems_abort_the_reduction() {
        let contract = Contract::load().unwrap();
        let mission = Mission {
            max_x: 5,
            max_y: 3,
            robots: vec![robot(1, 1, 'E', "RFRF")],
        };
        let failed = shrink(&mission, &contract, |_| Err("no such program".to_string()));
        assert_eq!(failed.err(), Some("no such program".to_string()));
    }

    #[test]
    fn every_candidate_offered_is_a_valid_mission() {
        // A reduction that walked a robot off its world, or shrank the world
        // out from under one, would turn a wrong answer into a rejection and
        // report the two as the same failure.
        let contract = Contract::load().unwrap();
        let mut rng = Rng::from_seed(37);
        for _ in 0..200 {
            let mission = Mission::draw_busy(&mut rng, &contract);
            for candidate in smaller(&mission, &contract) {
                assert!(
                    candidate.is_valid(&contract),
                    "{}",
                    String::from_utf8_lossy(&candidate.canonical())
                );
            }
        }
    }

    #[test]
    fn a_smallest_mission_offers_nothing() {
        let contract = Contract::load().unwrap();
        let mission = Mission {
            max_x: 0,
            max_y: 0,
            robots: Vec::new(),
        };
        assert!(smaller(&mission, &contract).is_empty());
    }
}
