//! The conformance suite for the Martian Robots CLI.
//!
//! The contract lives here, beside the suite that enforces it, because the two
//! are one artifact: a rule nobody checks is a suggestion, and a check that
//! cites no rule is a preference. An implementation depends on this
//! repository; this repository depends on nothing.
//!
//! The suite grades a candidate the only way a user can reach it — by running
//! the binary and reading stdout, stderr and the exit code. It never links
//! against an implementation, never imports its types, and never asks it a
//! question a shell could not ask.
//!
//! This revision establishes the interface and nothing else: it accepts a
//! candidate, checks the suite can actually run against it, runs the catalogue
//! — which is empty — and reports. That ordering is deliberate. An empty suite
//! that reports honestly is worth more than a full one nobody can invoke, and
//! every case added from here will cite the ruling it enforces.

mod contract;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use contract::Contract;

const USAGE: &str = "\
Usage: martian-robots-verify --bin <path> [--quiet]
       martian-robots-verify --contract

  --bin <path>   the candidate program to grade
  --quiet        report only the result line, not each case
  --contract     write the contract to stdout, as one document
  -h, --help     print this message and exit

Exit codes:
  0  the candidate conforms
  1  the candidate does not conform
  2  the suite could not run (bad arguments, no such candidate, or an
     incoherent contract)
";

/// The suite could not run, which is distinct from a candidate that ran and
/// failed. A caller that cannot tell those apart cannot tell a broken
/// invocation from a broken implementation.
const COULD_NOT_RUN: u8 = 2;

/// What the runner was asked to do.
enum Task {
    /// Grade a candidate.
    Grade { candidate: PathBuf, quiet: bool },
    /// Write the contract to stdout.
    Show,
    /// Print usage. Asked for, so it is a success.
    Help,
}

impl Task {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut args = args;
        let mut candidate = None;
        let mut quiet = false;

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "-h" | "--help" => return Ok(Self::Help),
                "--contract" => return Ok(Self::Show),
                "--quiet" => quiet = true,
                "--bin" => {
                    let path = args.next().ok_or("--bin needs a path")?;
                    candidate = Some(PathBuf::from(path));
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self::Grade {
            candidate: candidate.ok_or("--bin is required")?,
            quiet,
        })
    }
}

fn main() -> ExitCode {
    let task = match Task::parse(std::env::args().skip(1)) {
        Ok(task) => task,
        Err(message) => return fail(&message),
    };

    // Loaded before anything else, and for every task: a suite whose contract
    // does not parse has no business reporting on somebody else's program.
    let contract = match Contract::load() {
        Ok(contract) => contract,
        Err(message) => return fail(&message),
    };

    match task {
        Task::Help => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Task::Show => match contract.render() {
            Ok(document) => {
                print!("{document}");
                ExitCode::SUCCESS
            }
            Err(message) => fail(&message),
        },
        Task::Grade { candidate, quiet } => grade(&contract, &candidate, quiet),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("martian-robots-verify: {message}");
    eprint!("{USAGE}");
    ExitCode::from(COULD_NOT_RUN)
}

/// Run the catalogue against the candidate and report what happened.
fn grade(contract: &Contract, candidate: &Path, quiet: bool) -> ExitCode {
    // Checked before grading rather than discovered case by case: a candidate
    // that is not there produces a hundred identical failures that all mean
    // "you pointed me at nothing", which is a usage error wearing a
    // non-conformance costume.
    if !candidate.is_file() {
        return fail(&format!("no such candidate: {}", candidate.display()));
    }

    let (run, failed) = (0_usize, 0_usize);

    if !quiet {
        // Which contract is being enforced is the first thing a reader of the
        // output needs, because it is the thing that makes the verdict mean
        // anything.
        println!(
            "contract {}: {} ruled question(s) to enforce",
            contract.version,
            contract.ruled().count()
        );
        if run == 0 {
            println!(
                "no cases yet: this runner can grade a candidate but does not yet say anything about it"
            );
        }
    }
    println!(
        "result: {run} case(s) run, {} passed, {failed} failed",
        run - failed
    );

    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
