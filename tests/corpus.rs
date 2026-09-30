//! What a seed names, written down.
//!
//! A seed is advertised as a replay handle: a divergence prints one, and the
//! same seed is meant to bring the same inputs back. That holds only while the
//! generators stand still. Widen a draw range or add a mutation kind — both of
//! which happened within an hour of each other — and every historical seed
//! silently names different inputs, so a bug report citing one stops
//! reproducing and nobody finds out.
//!
//! These tests fix a small corpus per generator against one seed. They are not
//! about the inputs being *good*; other tests assert that the corpora are
//! valid, reach the contract's limits, and stay out of its open questions. A
//! change that improves a generator is expected to fail these, and the diff is
//! the point: it makes replacing the meaning of every seed something somebody
//! reviewed rather than something that happened.
//!
//! Each test holds two things. A list of descriptors, short enough to read in
//! a diff and specific enough to say what moved. And a digest over the exact
//! bytes, which catches everything a descriptor elides.

use std::fmt::Write as _;

use martian_robots_verify::contract::Contract;
use martian_robots_verify::mission::Mission;
use martian_robots_verify::mutation;
use martian_robots_verify::rng::Rng;
use martian_robots_verify::spelling::{self, Spelling};

const SEED: u64 = 20_260_930;

/// FNV-1a. Not a security property — a short, stable name for a byte string,
/// written out rather than depended on so the suite keeps no dependency for
/// it.
fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn describe(mission: &Mission) -> String {
    let mut described = format!("{}x{}", mission.max_x, mission.max_y);
    for robot in &mission.robots {
        let _ = write!(described, " {}{}", robot.facing, robot.instructions.len());
    }
    described
}

#[test]
fn the_missions_a_seed_names() {
    let contract = Contract::load().unwrap();
    let mut rng = Rng::from_seed(SEED);
    let mut described = Vec::new();
    let mut bytes = Vec::new();
    for _ in 0..12 {
        let mission = Mission::draw(&mut rng, &contract);
        described.push(describe(&mission));
        bytes.extend_from_slice(&mission.canonical());
    }

    assert_eq!(
        described, EXPECTED_MISSIONS,
        "the missions a seed names moved"
    );
    assert_eq!(digest(&bytes), EXPECTED_MISSION_DIGEST);
}

#[test]
fn the_spellings_a_seed_names() {
    let contract = Contract::load().unwrap();
    let mut rng = Rng::from_seed(SEED);
    let mission = Mission::read_back(b"5 3\n1 1 E\nRF\n").unwrap();
    let mut bytes = Vec::new();
    for _ in 0..8 {
        bytes.extend_from_slice(&spelling::render(
            &mission,
            &Spelling::draw(&mut rng, &contract.grammar),
        ));
    }

    assert_eq!(digest(&bytes), EXPECTED_SPELLING_DIGEST);
}

#[test]
fn the_mutations_a_seed_names() {
    let contract = Contract::load().unwrap();
    let mut rng = Rng::from_seed(SEED);
    let mut kinds = Vec::new();
    let mut bytes = Vec::new();
    while kinds.len() < 10 {
        let mission = Mission::draw(&mut rng, &contract);
        if let Some(mutation) = mutation::mutate(&mut rng, &mission, &contract) {
            kinds.push(mutation.kind);
            bytes.extend_from_slice(&mutation.rendered);
        }
    }

    assert_eq!(
        kinds, EXPECTED_MUTATIONS,
        "the mutations a seed names moved"
    );
    assert_eq!(digest(&bytes), EXPECTED_MUTATION_DIGEST);
}

const EXPECTED_MISSIONS: [&str; 12] = [
    "2x2 E94 S26 W99",
    "4x1 W24 N99 E46",
    "3x1 N99 W3",
    "5x43 E3",
    "39x3",
    "2x50 S99 E0",
    "18x3 W39",
    "22x45",
    "50x1 S6 S7 N5",
    "5x49 E85",
    "50x3 N64 E39 E99",
    "38x2 S6 N3 W85",
];
const EXPECTED_MISSION_DIGEST: &str = "4f3c4f2b65bcc2ce";
const EXPECTED_SPELLING_DIGEST: &str = "d1c614aec33820a9";
const EXPECTED_MUTATIONS: [&str; 10] = [
    "an instruction string over the limit",
    "an extra token on a position line",
    "an instruction string over the limit",
    "a start off the world",
    "a position line with no orientation",
    "a start off the world",
    "a start off the world",
    "an extra token on a position line",
    "a robot coordinate over the limit",
    "a carriage return that would make a line out of nothing",
];
const EXPECTED_MUTATION_DIGEST: &str = "64a72dbe41ed1d00";
