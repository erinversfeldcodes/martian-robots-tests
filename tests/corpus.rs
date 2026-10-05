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

/// The same seed a bare run of the suite uses, so the corpus written down
/// here is the corpus a reader gets by default.
const SEED: u64 = Rng::DEFAULT_SEED;

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
    let mission = Mission::read_back(b"5 3\n1 1 E\nRF\n", &contract.grammar).unwrap();
    let mut described = Vec::new();
    let mut bytes = Vec::new();
    for _ in 0..8 {
        let spelling = Spelling::draw(&mut rng, &contract.grammar);
        described.push(spelling.describe());
        bytes.extend_from_slice(&spelling::render(&mission, &spelling));
    }

    assert_eq!(
        described, EXPECTED_SPELLINGS,
        "the spellings a seed names moved"
    );
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
    "2x2 W0 W0 E2 E7 W7 E4 E87",
    "5x1 E36",
    "2x5 N1 W7 N21 S0 N4 W3 W2",
    "50x38 N7 W48 N99 W4 S3 W48 N95 S6 W5 W4",
    "2x50 W1 E1 N4 E35",
    "46x39 E6 S6 S7",
    "3x25 N52 S0 S2 E3 S1 W99 N4 E4",
    "0x3 E1 E99 W95 S0 E14",
    "5x0 W3",
    "4x3 E5 W5 E4 N99 E70",
    "50x12 N8 N2 S7 E4 W2 S4 S5 S68 S7",
    "1x2 W80 S99 N7 S84 E5 S97 E82 N5 E99 S5 E0 N4",
];
const EXPECTED_MISSION_DIGEST: &str = "2721281322b62b2b";
const EXPECTED_SPELLINGS: [&str; 8] = [
    "lf/eol/pre1/runsst,ts,s,s/lead4/trail4/zeros1/sep2/emptyblank",
    "crlf/eol/pre2/runsst,t,t,st/lead3/trail3/zeros1/sep1/wsblank",
    "lf/no-eol/pre2/runsst,st,t,t/lead3/trail1/zeros2/sep2/emptyblank",
    "lf/eol/pre1/runsst,ss,tt,s/lead3/trail4/zeros2/sep2/emptyblank",
    "lf/eol/pre2/runss,t,t,ts/lead2/trail4/zeros3/sep2/emptyblank",
    "lf/no-eol/pre2/runsst,st,ts,tt/lead3/trail2/zeros1/sep2/wsblank",
    "crlf/eol/pre2/runsts,ss,ts,ts/lead4/trail4/zeros3/sep1/wsblank",
    "crlf/eol/pre0/runsst,s,t,ts/lead4/trail4/zeros3/sep1/emptyblank",
];
const EXPECTED_SPELLING_DIGEST: &str = "d1c614aec33820a9";
const EXPECTED_MUTATIONS: [&str; 10] = [
    "an instruction outside the vocabulary",
    "a carriage return that ends nothing",
    "a start off the world",
    "a byte that cannot begin a character",
    "a position line with no orientation",
    "a separator the grammar does not have",
    "a position line with no orientation",
    "a code point past the last one",
    "a separator the grammar does not have",
    "a foreign separator, leading, on an instruction line",
];
const EXPECTED_MUTATION_DIGEST: &str = "1989f21088b4d2ea";
