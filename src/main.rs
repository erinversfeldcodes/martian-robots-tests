use martian_robots_verify::{cases, contract, expect, modes, rng, run};

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
  --differential <n> missions to compare with a second implementation
                 written from the same contract (default 60)
  --seed <n>     the seed the generated missions come from, so a failure
                 replays. Fixed by default, so two runs of the same code
                 grade the same corpus; `--seed random` explores a wider one
                 and prints the seed it chose
  --case <id>    run only the cases whose id starts with this, and none of
                 the generated modes. A group name on its own selects the
                 group: `--case scent` is every scent case
  --cases        list every case id and exit
  --timeout <s>  how long one run of the implementation may take (default 10).
                 Q3 leaves a hang to grader policy, so this is policy: a
                 loaded runner should not read as a conformance failure
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
        /// Run only the cases whose id starts with this, and no generated
        /// mode. A failure names an id, and this is how the id is used.
        only: Option<String>,
    },
    Show,
    List,
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
        let mut against_reference = 60;
        let mut seed = None;
        let mut only = None;
        let mut timeout = run::TIMEOUT;

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "-h" | "--help" => return Ok(Self::Help),
                "--contract" => return Ok(Self::Show),
                "--cases" => return Ok(Self::List),
                "--quiet" => quiet = true,
                "--bin" => {
                    let path = args.next().ok_or("--bin needs a path")?;
                    implementation = Some(PathBuf::from(path));
                }
                "--spelling" => missions = number(args.next(), "--spelling")?,
                "--properties" => checks = number(args.next(), "--properties")?,
                "--rejections" => breakages = number(args.next(), "--rejections")?,
                "--differential" => against_reference = number(args.next(), "--differential")?,
                "--seed" => {
                    let asked = args.next().ok_or("--seed needs a number or `random`")?;
                    seed = Some(if asked == "random" {
                        rng::Rng::seed_from_the_clock()
                    } else {
                        asked
                            .parse()
                            .map_err(|_| "--seed needs a number or `random`".to_string())?
                    });
                }
                "--case" => only = Some(args.next().ok_or("--case needs an id")?),
                "--timeout" => {
                    timeout = std::time::Duration::from_secs(number(args.next(), "--timeout")?);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        // Asking for one case and getting sixty generated missions as well
        // would make the flag useless for the thing it is for: running the
        // case a failure just named.
        if only.is_some() {
            missions = 0;
            checks = 0;
            breakages = 0;
            against_reference = 0;
        }

        Ok(Self::Grade {
            implementation: implementation.ok_or("--bin is required")?,
            quiet,
            budget: modes::Budget {
                missions: u32::try_from(missions).map_err(|_| "--spelling is too large")?,
                spellings: 4,
                properties: u32::try_from(checks).map_err(|_| "--properties is too large")?,
                rejections: u32::try_from(breakages).map_err(|_| "--rejections is too large")?,
                differential: u32::try_from(against_reference)
                    .map_err(|_| "--differential is too large")?,
                seed: seed.unwrap_or(rng::Rng::DEFAULT_SEED),
                timeout,
            },
            only,
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
        Task::List => {
            for case in cases::catalogue(&contract) {
                println!("{}", case.id);
            }
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
            only,
        } => grade(&contract, &implementation, quiet, &budget, only.as_deref()),
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
    only: Option<&str>,
) -> ExitCode {
    if !implementation.is_file() {
        return fail(&format!(
            "no such implementation: {}",
            implementation.display()
        ));
    }

    match catalogue(contract, implementation, quiet, only, budget.timeout) {
        Err(message) => fail(&message),
        // A selected run is asking about named cases, so the generated modes
        // are not run and their empty summaries are not printed.
        Ok(failed) if only.is_some() => {
            if failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Ok(failed) => match generated(contract, implementation, quiet, budget) {
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
fn catalogue(
    contract: &Contract,
    implementation: &Path,
    quiet: bool,
    only: Option<&str>,
    timeout: std::time::Duration,
) -> Result<usize, String> {
    let everything = cases::catalogue(contract);
    let catalogue: Vec<&cases::Case> = everything
        .iter()
        .filter(|case| only.is_none_or(|prefix| case.id.starts_with(prefix)))
        .collect();

    // A filter that matches nothing must not report a clean run. This is the
    // shape of every silently-empty test suite there has ever been.
    if let Some(prefix) = only
        && catalogue.is_empty()
    {
        return Err(format!(
            "no case id starts with {prefix:?}; --cases lists them"
        ));
    }
    let mut failed = 0;

    for case in &catalogue {
        let seen = run::observe(implementation, &case.arguments, &case.stdin, timeout)?;
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
                println!("      case: {} (--case {})", case.id, case.id);
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

    if !quiet && only.is_none() {
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
/// Anything that was asked to run and judged nothing did not pass — it did not
/// report. Printing `0 of 1 attempted … 0 failure(s)` and exiting 0 is the
/// shape of every silently empty test suite there has ever been, and it is the
/// one place this suite was not applying to itself the rule it applies to a
/// mistyped `--case`. A consumer who turns the budgets down to fit a CI minute
/// should be told the budget bought nothing, not handed a green run.
fn judged(what: &str, judgements: usize, requested: u32, advice: &str) -> Result<(), String> {
    if requested > 0 && judgements == 0 {
        return Err(format!(
            "the {what} generator was asked for {requested} and judged nothing: {advice}"
        ));
    }
    Ok(())
}

fn generated(
    contract: &Contract,
    implementation: &Path,
    quiet: bool,
    budget: &modes::Budget,
) -> Result<usize, String> {
    let spelling = modes::spelling_differential(implementation, contract, budget)?;
    for divergence in &spelling.divergences {
        println!("DIVERGES on a legal respelling");
        println!("      mission:  {}", show(&divergence.mission));
        println!("      spelled:  {}", show(&divergence.against));
        println!("      expected: {}", show(&divergence.expected));
        println!("      got:      {}", show(&divergence.got));
    }
    for refused in &spelling.refused {
        println!("REFUSED a valid mission, so no spelling of it could be compared");
        println!("      mission:  {}", show(refused));
    }
    println!(
        "spelling: {} mission(s) x {} rendering(s), seed {}, {} divergence(s), {} refused",
        budget.missions,
        budget.spellings,
        budget.seed,
        spelling.divergences.len(),
        spelling.refused.len()
    );

    judged(
        "spelling",
        spelling.compared,
        budget.missions,
        "every drawn mission was refused, so no respelling of one could be compared",
    )?;

    let properties = modes::properties(implementation, contract, budget)?;
    for violation in &properties.violations {
        println!("VIOLATED {violation}");
    }
    if !quiet {
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

    judged(
        "properties",
        properties.fired.iter().sum::<u32>() as usize,
        budget.properties,
        "no predicate evaluated, so nothing was stated about any answer - the \
         corpus is too small for any of them to apply",
    )?;

    let rejections = modes::rejections(implementation, contract, budget)?;
    for failure in &rejections.failures {
        println!("ACCEPTED {failure}");
    }
    println!(
        "rejections: {} of {} attempted, seed {}, {} failure(s)",
        rejections.run,
        budget.rejections,
        budget.seed,
        rejections.failures.len()
    );

    judged(
        "rejections",
        rejections.run as usize,
        budget.rejections,
        "no mutation could be built, so no input was refused or accepted - try \
         a larger budget or a different seed",
    )?;

    let disagreements = modes::differential(implementation, contract, budget)?;
    for disagreement in &disagreements {
        println!("DISAGREES with the reference");
        println!("      mission:  {}", show(&disagreement.mission));
        println!("      expected: {}", show(&disagreement.expected));
        println!("      got:      {}", show(&disagreement.got));
        if let Some(reduced) = &disagreement.reduced {
            println!(
                "      reduced in {} attempt(s){}:",
                reduced.attempts,
                if reduced.exhausted {
                    ", before the budget ran out"
                } else {
                    ""
                }
            );
            println!("      mission:  {}", show(&reduced.mission));
            println!("      expected: {}", show(&reduced.expected));
            println!("      got:      {}", show(&reduced.got));
        }
    }
    println!(
        "differential: {} mission(s), seed {}, {} disagreement(s)",
        budget.differential,
        budget.seed,
        disagreements.len()
    );

    Ok(spelling.divergences.len()
        + spelling.refused.len()
        + properties.violations.len()
        + rejections.failures.len()
        + disagreements.len())
}
