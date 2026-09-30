use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How long a program gets before the grader gives up on it.
///
/// Q3 leaves a hang to grader policy, so this is a policy and not a contract —
/// which is exactly why it belongs on a flag. Ten seconds is generous for one
/// mission and mean on a loaded runner, and a conformance failure that is
/// really a busy machine is the worst kind of red.
pub const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq, Eq)]
pub enum Ending {
    Code(i32),
    /// Killed by a signal, which on Unix is how a panic-free crash shows up.
    Signal,
    /// Killed by the grader. Q3 leaves this to grader policy, so it is a
    /// failure here by choice rather than by contract.
    Timeout,
}

#[derive(Debug)]
pub struct Observation {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub ending: Ending,
    /// Whether the program took everything offered on stdin.
    ///
    /// R20 and R21 both say a flag leaves stdin unread, and this is the only
    /// way the outside of a process can tell: offer more than a pipe will
    /// hold, and a program that never reads cannot have taken it. A program
    /// that drains a megabyte to decide what `--help` means has read stdin,
    /// whatever it did with the bytes.
    pub drained_stdin: bool,
}

impl Observation {
    pub fn exited_zero(&self) -> bool {
        self.ending == Ending::Code(0)
    }

    pub fn exited_non_zero(&self) -> bool {
        matches!(self.ending, Ending::Code(code) if code != 0)
    }
}

/// Where a spawned process should write its coverage profile.
///
/// This suite is meant to be the only test oracle an implementation has: its
/// code is exercised by another process spawning it, so the only way it can
/// measure its own coverage is for the instrumentation to work *through* that
/// boundary. Inheriting the environment is most of it, and that happens by
/// itself — nothing here clears it.
///
/// What does not happen by itself is one profile per process. A run spawns
/// hundreds, and LLVM's instrumentation writes to the path it is given, so a
/// pattern with nothing process-specific in it has every process writing the
/// same file. The result is not an error: it is a coverage number built from
/// whichever process happened to write last, which is the same species of
/// quiet wrongness this suite exists to refuse. So a pattern that cannot
/// distinguish processes gains `%p`, and one that already can is left alone.
fn profile_pattern(current: Option<&str>) -> Option<String> {
    let current = current?;
    if current.is_empty() || current.contains("%p") || current.contains("%m") {
        return None;
    }
    Some(match current.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem}-%p.{extension}"),
        _ => format!("{current}-%p"),
    })
}

/// Run a program the way a shell would: bytes to stdin, bytes from stdout and
/// stderr, an exit code, and a deadline.
///
/// stdin is written from its own thread and its errors are dropped, because a
/// program that exits before reading everything is not misbehaving — a broken
/// pipe here is the program's verdict, not the grader's. stdout and stderr are
/// drained concurrently for the opposite reason: a program that writes more
/// than a pipe buffer holds would block forever against a grader that waits
/// before reading.
pub fn observe(
    program: &Path,
    arguments: &[OsString],
    stdin: &[u8],
    timeout: Duration,
) -> Result<Observation, String> {
    let mut spawning = Command::new(program);
    // The environment is inherited, which is what lets an instrumented
    // implementation record coverage from inside a run. Only the profile path
    // is touched, and only when it could not tell two processes apart.
    if let Some(pattern) = profile_pattern(std::env::var("LLVM_PROFILE_FILE").ok().as_deref()) {
        spawning.env("LLVM_PROFILE_FILE", pattern);
    }
    let mut child = spawning
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not run {}: {error}", program.display()))?;

    let mut pipe = child.stdin.take().ok_or("stdin was not piped")?;
    let input = stdin.to_vec();
    let writer = thread::spawn(move || pipe.write_all(&input).is_ok());

    let mut out = child.stdout.take().ok_or("stdout was not piped")?;
    let mut err = child.stderr.take().ok_or("stderr was not piped")?;
    let reader_out = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = out.read_to_end(&mut bytes);
        bytes
    });
    let reader_err = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = err.read_to_end(&mut bytes);
        bytes
    });

    let deadline = Instant::now() + timeout;
    let ending = loop {
        match child.try_wait() {
            Ok(Some(status)) => break ending_of(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Ending::Timeout;
                }
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => return Err(format!("waiting for {}: {error}", program.display())),
        }
    };

    let stdout = reader_out.join().unwrap_or_default();
    let stderr = reader_err.join().unwrap_or_default();
    let drained_stdin = writer.join().unwrap_or(false);

    Ok(Observation {
        stdout,
        stderr,
        ending,
        drained_stdin,
    })
}

fn ending_of(status: std::process::ExitStatus) -> Ending {
    status.code().map_or(Ending::Signal, Ending::Code)
}

#[cfg(test)]
mod tests {
    use super::{Ending, TIMEOUT, observe, profile_pattern};
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::Duration;

    fn shell(script: &str) -> (PathBuf, Vec<OsString>) {
        (
            PathBuf::from("/bin/sh"),
            vec![OsString::from("-c"), OsString::from(script)],
        )
    }

    #[test]
    fn stdout_stderr_and_the_exit_code_are_all_observed() {
        let (program, arguments) = shell("printf out; printf err >&2; exit 3");
        let seen = observe(&program, &arguments, b"", TIMEOUT).unwrap();
        assert_eq!(seen.stdout, b"out");
        assert_eq!(seen.stderr, b"err");
        assert_eq!(seen.ending, Ending::Code(3));
    }

    #[test]
    fn the_environment_reaches_the_program() {
        // Nothing here clears the environment, and an instrumented
        // implementation depends on that: this suite is its only test oracle,
        // so the only place its coverage can be recorded is inside a process
        // this function spawned. A stray `env_clear` would leave a consumer
        // with an empty profile and no reason to suspect one.
        let (program, arguments) = shell("printf '%s' \"$PATH\"");
        let seen = observe(&program, &arguments, b"", TIMEOUT).unwrap();
        let inherited = String::from_utf8(seen.stdout).expect("a path");
        assert_eq!(
            inherited,
            std::env::var("PATH").expect("a PATH to inherit"),
            "the spawned program did not inherit the environment"
        );
    }

    #[test]
    fn a_profile_path_that_cannot_tell_two_processes_apart_gains_a_process_id() {
        // A run spawns hundreds of processes. Without this, they all write one
        // file and the coverage number comes from whichever finished last.
        assert_eq!(
            profile_pattern(Some("cov.profraw")).as_deref(),
            Some("cov-%p.profraw")
        );
        assert_eq!(
            profile_pattern(Some("target/coverage/martian.profraw")).as_deref(),
            Some("target/coverage/martian-%p.profraw")
        );
        assert_eq!(
            profile_pattern(Some("profile")).as_deref(),
            Some("profile-%p")
        );
    }

    #[test]
    fn a_profile_path_that_already_distinguishes_processes_is_left_alone() {
        // `%p` and `%m` are LLVM's own patterns for this, and a consumer who
        // wrote one knows what they want. Rewriting it would move their files.
        assert_eq!(profile_pattern(Some("cov-%p.profraw")), None);
        assert_eq!(profile_pattern(Some("cov-%m.profraw")), None);
        assert_eq!(profile_pattern(Some("%m-%p.profraw")), None);
        assert_eq!(profile_pattern(None), None);
        assert_eq!(profile_pattern(Some("")), None);
    }

    #[test]
    fn stdin_reaches_the_program_as_bytes() {
        let (program, arguments) = shell("cat");
        let seen = observe(&program, &arguments, &[0x00, 0xff, b'\n'], TIMEOUT).unwrap();
        assert_eq!(seen.stdout, vec![0x00, 0xff, b'\n']);
    }

    #[test]
    fn a_program_that_ignores_stdin_does_not_hang_the_grader() {
        let (program, arguments) = shell("exit 0");
        let large = vec![b'x'; 1 << 20];
        let seen = observe(&program, &arguments, &large, TIMEOUT).unwrap();
        assert_eq!(seen.ending, Ending::Code(0));
    }

    #[test]
    fn a_program_that_floods_a_pipe_does_not_hang_the_grader() {
        let (program, arguments) = shell("yes x | head -c 1000000");
        let seen = observe(&program, &arguments, b"", TIMEOUT).unwrap();
        assert_eq!(seen.stdout.len(), 1_000_000);
        assert_eq!(seen.ending, Ending::Code(0));
    }

    #[test]
    fn a_program_that_never_ends_is_killed_and_reaped() {
        let (program, arguments) = shell("sleep 30");
        let seen = observe(&program, &arguments, b"", Duration::from_millis(100)).unwrap();
        assert_eq!(seen.ending, Ending::Timeout);
    }

    #[test]
    fn a_signalled_program_is_not_mistaken_for_an_exit_code() {
        let (program, arguments) = shell("kill -9 $$");
        let seen = observe(&program, &arguments, b"", TIMEOUT).unwrap();
        assert_eq!(seen.ending, Ending::Signal);
    }
}
