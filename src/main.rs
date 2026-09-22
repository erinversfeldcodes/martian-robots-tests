mod contract;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use contract::Contract;

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

    let (run, failed) = (0_usize, 0_usize);

    if !quiet {
        println!(
            "contract {}: {} ruled question(s) to enforce",
            contract.version,
            contract.ruled().count()
        );
        if run == 0 {
            println!(
                "no cases yet: this runner can grade an implementation but says nothing about it"
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
