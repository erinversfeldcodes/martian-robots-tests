mod cases;
mod contract;
mod expect;
mod run;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use contract::Contract;
use expect::show;

const USAGE: &str = "\
Usage: martian-robots-verify --bin <path> [--quiet]
       martian-robots-verify --contract

  --bin <path>   the implementation under test
  --quiet        report only the result line, not each case
  --contract     write the contract to stdout, as one document
  -h, --help     print this message and exit

Exit codes:
  0  the implementation conforms
  1  the implementation does not conform
  2  the suite could not run (bad arguments, no such implementation, or
     an incoherent contract)
";

const COULD_NOT_RUN: u8 = 2;

enum Task {
    Grade {
        implementation: PathBuf,
        quiet: bool,
    },
    Show,
    Help,
}

impl Task {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut args = args;
        let mut implementation = None;
        let mut quiet = false;

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "-h" | "--help" => return Ok(Self::Help),
                "--contract" => return Ok(Self::Show),
                "--quiet" => quiet = true,
                "--bin" => {
                    let path = args.next().ok_or("--bin needs a path")?;
                    implementation = Some(PathBuf::from(path));
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self::Grade {
            implementation: implementation.ok_or("--bin is required")?,
            quiet,
        })
    }
}

fn main() -> ExitCode {
    let task = match Task::parse(std::env::args().skip(1)) {
        Ok(task) => task,
        Err(message) => return fail(&message),
    };

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
        Task::Grade {
            implementation,
            quiet,
        } => grade(&contract, &implementation, quiet),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("martian-robots-verify: {message}");
    eprint!("{USAGE}");
    ExitCode::from(COULD_NOT_RUN)
}

fn grade(contract: &Contract, implementation: &Path, quiet: bool) -> ExitCode {
    if !implementation.is_file() {
        return fail(&format!(
            "no such implementation: {}",
            implementation.display()
        ));
    }

    let catalogue = cases::catalogue(contract);
    let mut failed = 0_usize;

    for case in &catalogue {
        let seen = match run::observe(implementation, &case.arguments, &case.stdin, run::TIMEOUT) {
            Ok(seen) => seen,
            Err(message) => return fail(&message),
        };

        match case.expect.judge(&seen) {
            None => {
                if !quiet {
                    println!("ok   {}", case.name);
                }
            }
            Some(why) => {
                failed += 1;
                println!("FAIL {}", case.name);
                println!("      {why}");
                if !case.stdin.is_empty() {
                    println!("      stdin: {}", show(&case.stdin));
                }
                if case.enforces.is_empty() {
                    println!("      why: {}", case.note);
                } else {
                    println!("      enforces: {}", case.enforces.join(", "));
                }
            }
        }
    }

    let enforced = contract
        .ruled()
        .filter(|ruling| {
            catalogue
                .iter()
                .any(|case| case.enforces.iter().any(|id| id == &ruling.id))
        })
        .count();
    let ruled = contract.ruled().count();

    if !quiet {
        println!(
            "contract {}: {enforced} of {ruled} ruled question(s) enforced",
            contract.version
        );
    }
    println!(
        "result: {} case(s) run, {} passed, {failed} failed",
        catalogue.len(),
        catalogue.len() - failed
    );

    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
