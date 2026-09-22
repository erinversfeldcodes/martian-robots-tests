mod cases;
mod contract;
mod expect;
mod mission;
mod modes;
mod mutation;
mod properties;
mod rng;
mod run;
mod spelling;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use contract::Contract;
use expect::show;

const USAGE: &str = "\
Usage: martian-robots-verify --bin <path> [--quiet]
       martian-robots-verify --contract

  --bin <path>   the implementation under test
  --quiet        report only failures and the result lines
  --spelling <n> missions to render several legal ways (default 60)
  --properties <n> missions to check invariants against (default 40)
  --rejections <n> missions to break on purpose (default 40)
  --seed <n>     the seed the generated missions come from, so a failure
                 replays; a run without one picks and prints its own
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
        budget: modes::Budget,
    },
    Show,
    Help,
}

impl Task {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut args = args;
        let mut implementation = None;
        let mut quiet = false;
        let mut missions = 60;
        let mut checks = 40;
        let mut breakages = 40;
        let mut seed = None;

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "-h" | "--help" => return Ok(Self::Help),
                "--contract" => return Ok(Self::Show),
                "--quiet" => quiet = true,
                "--bin" => {
                    let path = args.next().ok_or("--bin needs a path")?;
                    implementation = Some(PathBuf::from(path));
                }
                "--spelling" => missions = number(args.next(), "--spelling")?,
                "--properties" => checks = number(args.next(), "--properties")?,
                "--rejections" => breakages = number(args.next(), "--rejections")?,
                "--seed" => seed = Some(number(args.next(), "--seed")?),
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self::Grade {
            implementation: implementation.ok_or("--bin is required")?,
            quiet,
            budget: modes::Budget {
                missions: u32::try_from(missions).map_err(|_| "--spelling is too large")?,
                spellings: 4,
                properties: u32::try_from(checks).map_err(|_| "--properties is too large")?,
                rejections: u32::try_from(breakages).map_err(|_| "--rejections is too large")?,
                seed: seed.unwrap_or_else(rng::Rng::seed_from_the_clock),
            },
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
            budget,
        } => grade(&contract, &implementation, quiet, &budget),
    }
}

fn number(argument: Option<String>, flag: &str) -> Result<u64, String> {
    argument
        .ok_or_else(|| format!("{flag} needs a number"))?
        .parse()
        .map_err(|_| format!("{flag} needs a number"))
}

fn fail(message: &str) -> ExitCode {
    eprintln!("martian-robots-verify: {message}");
    eprint!("{USAGE}");
    ExitCode::from(COULD_NOT_RUN)
}

fn grade(
    contract: &Contract,
    implementation: &Path,
    quiet: bool,
    budget: &modes::Budget,
) -> ExitCode {
    if !implementation.is_file() {
        return fail(&format!(
            "no such implementation: {}",
            implementation.display()
        ));
    }

    match catalogue(contract, implementation, quiet) {
        Err(message) => fail(&message),
        Ok(failed) => match generated(contract, implementation, quiet, budget) {
            // A generator that cannot trust its own output has nothing to say
            // about anybody else's, so this is a suite failure rather than a
            // verdict on the implementation.
            Err(message) => fail(&message),
            Ok(complaints) => {
                if failed == 0 && complaints == 0 {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
        },
    }
}

/// The cases, and how much of the contract they reach.
fn catalogue(contract: &Contract, implementation: &Path, quiet: bool) -> Result<usize, String> {
    let catalogue = cases::catalogue(contract);
    let mut failed = 0;

    for case in &catalogue {
        let seen = run::observe(implementation, &case.arguments, &case.stdin, run::TIMEOUT)?;
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

    if !quiet {
        let enforced = contract
            .ruled()
            .filter(|ruling| {
                catalogue
                    .iter()
                    .any(|case| case.enforces.iter().any(|id| id == &ruling.id))
            })
            .count();
        println!(
            "contract {}: {enforced} of {} ruled question(s) enforced",
            contract.version,
            contract.ruled().count()
        );
    }
    println!(
        "result: {} case(s) run, {} passed, {failed} failed",
        catalogue.len(),
        catalogue.len() - failed
    );
    Ok(failed)
}

/// The generated modes, which reach what an enumerated catalogue cannot.
fn generated(
    contract: &Contract,
    implementation: &Path,
    quiet: bool,
    budget: &modes::Budget,
) -> Result<usize, String> {
    let divergences = modes::spelling_differential(implementation, contract, budget)?;
    for divergence in &divergences {
        println!("DIVERGES on a legal respelling");
        println!("      mission:  {}", show(&divergence.mission));
        println!("      spelled:  {}", show(&divergence.against));
        println!("      expected: {}", show(&divergence.expected));
        println!("      got:      {}", show(&divergence.got));
    }
    println!(
        "spelling: {} mission(s) x {} rendering(s), seed {}, {} divergence(s)",
        budget.missions,
        budget.spellings,
        budget.seed,
        divergences.len()
    );

    let properties = modes::properties(implementation, contract, budget)?;
    for violation in &properties.violations {
        println!("VIOLATED {violation}");
    }
    if !quiet {
        // Firing counts, not just failures: a predicate that never evaluated
        // reads exactly like one that always held.
        for (name, fired) in properties.names.iter().zip(&properties.fired) {
            println!("      {fired:>4} {name}");
        }
    }
    println!(
        "properties: {} mission(s), seed {}, {} violation(s)",
        properties.missions,
        budget.seed,
        properties.violations.len()
    );

    let rejections = modes::rejections(implementation, contract, budget)?;
    for failure in &rejections.failures {
        println!("ACCEPTED {failure}");
    }
    println!(
        "rejections: {} mutation(s), seed {}, {} failure(s)",
        rejections.run,
        budget.seed,
        rejections.failures.len()
    );

    Ok(divergences.len() + properties.violations.len() + rejections.failures.len())
}
