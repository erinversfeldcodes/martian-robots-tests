use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
}

impl Observation {
    pub fn exited_zero(&self) -> bool {
        self.ending == Ending::Code(0)
    }

    pub fn exited_non_zero(&self) -> bool {
        matches!(self.ending, Ending::Code(code) if code != 0)
    }
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
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not run {}: {error}", program.display()))?;

    let mut pipe = child.stdin.take().ok_or("stdin was not piped")?;
    let input = stdin.to_vec();
    let writer = thread::spawn(move || {
        let _ = pipe.write_all(&input);
    });

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
    let _ = writer.join();

    Ok(Observation {
        stdout,
        stderr,
        ending,
    })
}

fn ending_of(status: std::process::ExitStatus) -> Ending {
    status.code().map_or(Ending::Signal, Ending::Code)
}

#[cfg(test)]
mod tests {
    use super::{Ending, TIMEOUT, observe};
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
