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

const COULD_NOT_RUN: u8 = 2;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
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
