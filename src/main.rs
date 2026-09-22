//! The conformance suite for the Martian Robots CLI.
//!
//! The contract lives here, beside the suite that enforces it, because the
//! two are one artifact: a rule nobody checks is a suggestion, and a check
//! that cites no rule is a preference. An implementation depends on this
//! repository; this repository depends on nothing.
//!
//! Today it holds the contract and can show it. Grading arrives next, and
//! every case it gains will cite the ruling it enforces.

mod contract;

use std::process::ExitCode;

const USAGE: &str = "\
Usage: martian-robots-verify --contract

  --contract     write the contract to stdout, as one document
  -h, --help     print this message and exit

Exit codes:
  0  success
  2  the suite could not run (bad arguments, or an incoherent contract)
";

/// The suite could not run, which will stay distinct from a candidate that
/// ran and failed once there is something to grade.
const COULD_NOT_RUN: u8 = 2;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        // Loading validates: an incoherent contract fails here rather than
        // halfway through grading something against it.
        Some("--contract") => match contract::Contract::load().and_then(|c| c.render()) {
            Ok(document) => {
                print!("{document}");
                ExitCode::SUCCESS
            }
            Err(message) => fail(&message),
        },
        Some(other) => fail(&format!("unknown argument: {other}")),
        None => fail("nothing to do"),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("martian-robots-verify: {message}");
    eprint!("{USAGE}");
    ExitCode::from(COULD_NOT_RUN)
}
