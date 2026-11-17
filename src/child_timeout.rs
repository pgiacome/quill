//! Run a helper program with a time limit.
//!
//! `Command::output` and `Command::status` wait forever. A wedged D-Bus
//! portal, a file on a stalled network share or a hung ffmpeg would then pin
//! a thread (and whatever flag it holds) for the life of the process. Every
//! child that is waited on goes through here instead: when the limit passes
//! the child is killed and reaped, and the caller gets
//! [`io::ErrorKind::TimedOut`].
#![deny(clippy::indexing_slicing)]

use crate::locks::lock_or_recover;
use crate::media_tools::NoConsole;
use std::io::{self, Read};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// How often a running child is checked for exit.
const POLL: Duration = Duration::from_millis(10);
/// How long the output readers get to finish after the child is gone. A
/// grandchild that inherited the pipes can keep them open past the kill.
const READER_GRACE: Duration = Duration::from_millis(500);

/// Short probes (`ffprobe` on one file, `gsettings get`, `fc-list`).
pub const PROBE_LIMIT: Duration = Duration::from_secs(10);
/// Desktop services over D-Bus (`gdbus`, `xdg-mime`, `xdg-settings`).
pub const DESKTOP_QUERY_LIMIT: Duration = Duration::from_secs(5);
/// A notification helper that waits for a click (`notify-send --wait`): the
/// user may take their time, but not for ever.
pub const NOTIFICATION_WAIT_LIMIT: Duration = Duration::from_secs(5 * 60);

/// Time an ffmpeg job on `media_seconds` of media may take: a fixed 30 s for
/// start-up and slow disks, plus 4 s for each second of media (a slow
/// software encoder runs far below real time), at most ten minutes. An
/// unknown (zero or negative) duration gets the two-minute middle.
pub fn transcode_limit(media_seconds: f64) -> Duration {
    if !media_seconds.is_finite() || media_seconds <= 0.0 {
        return Duration::from_secs(120);
    }
    let seconds = (30.0 + media_seconds * 4.0).min(600.0);
    Duration::from_secs_f64(seconds)
}

/// `cmd.output()` with a limit: stdin is closed, stdout and stderr are
/// captured, and a child still running after `limit` is killed and reaped.
pub fn output_within(cmd: &mut Command, limit: Duration) -> io::Result<Output> {
    let mut child = cmd
        .no_console()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let readers = child
        .stdout
        .take()
        .map(read_all_in_thread)
        .transpose()
        .and_then(|stdout| {
            let stderr = child.stderr.take().map(read_all_in_thread).transpose()?;
            Ok((stdout, stderr))
        });
    let (stdout, stderr) = reap_on_error(&mut child, readers)?;
    let waited = wait_within(&mut child, limit);
    let status = reap_on_error(&mut child, waited)?;
    let ok = status.success();
    Ok(Output {
        status,
        stdout: collect(stdout, ok, READER_GRACE)?,
        stderr: collect(stderr, ok, READER_GRACE)?,
    })
}

/// What `program args` prints on stdout when it exits successfully within
/// `limit`; `None` for a missing program, a failure, a timeout or output
/// that is not text. For the desktop's one-line answers (`gsettings get`).
pub fn stdout_within(program: &str, args: &[&str], limit: Duration) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    let out = output_within(&mut cmd, limit).ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `cmd.status()` with a limit. Stdin is closed; stdout and stderr stay as
/// the caller set them. Those must not be `Stdio::piped()`: nothing here
/// drains a pipe, so a chatty child would block until the limit. Use
/// `Stdio::null()` or a log file (as the callers do), or [`output_within`]
/// to read what the child prints.
pub fn status_within(cmd: &mut Command, limit: Duration) -> io::Result<ExitStatus> {
    let mut child = cmd.no_console().stdin(Stdio::null()).spawn()?;
    let waited = wait_within(&mut child, limit);
    reap_on_error(&mut child, waited)
}

/// Wait for an already started `child` for at most `limit`. The child is
/// left running on a timeout: the caller kills and reaps it.
pub fn wait_within(child: &mut std::process::Child, limit: Duration) -> io::Result<ExitStatus> {
    wait_on(child, limit, &mut SystemClock)
}

/// A started child as [`wait_on`] and [`reap_on_error`] see it, so a test
/// can stand in a fake that never exits.
trait Running {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn kill(&mut self) -> io::Result<()>;
    fn wait(&mut self) -> io::Result<ExitStatus>;
}

impl Running for std::process::Child {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        std::process::Child::try_wait(self)
    }
    fn kill(&mut self) -> io::Result<()> {
        std::process::Child::kill(self)
    }
    fn wait(&mut self) -> io::Result<ExitStatus> {
        std::process::Child::wait(self)
    }
}

/// Where [`wait_on`] reads the time and sleeps between checks, so a test
/// can run the limit on a manual clock.
trait Clock {
    fn now(&self) -> Instant;
    fn sleep(&mut self, step: Duration);
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sleep(&mut self, step: Duration) {
        std::thread::sleep(step);
    }
}

fn wait_on(
    child: &mut impl Running,
    limit: Duration,
    clock: &mut impl Clock,
) -> io::Result<ExitStatus> {
    let deadline = clock.now().checked_add(limit);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if deadline.is_some_and(|deadline| clock.now() >= deadline) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the helper program did not finish in time",
            ));
        }
        clock.sleep(POLL);
    }
}

/// After a failed wait the child is killed and reaped, so no zombie or
/// runaway helper is left behind.
fn reap_on_error<T>(child: &mut impl Running, result: io::Result<T>) -> io::Result<T> {
    result.inspect_err(|_| {
        let _ = child.kill();
        let _ = child.wait();
    })
}

/// Whether `err` is a [`wait_within`] timeout.
pub fn is_timeout(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::TimedOut
}

/// A pipe being drained on a thread of its own.
struct Reader {
    /// Signalled when the pipe reached its end.
    done: mpsc::Receiver<()>,
    /// Everything read so far.
    bytes: Arc<Mutex<Vec<u8>>>,
}

/// Read `pipe` on a thread of its own, so a child that fills its pipe never
/// stalls waiting for us to read. A thread that cannot start is an error: the
/// output would be silently empty.
fn read_all_in_thread<R: Read + Send + 'static>(mut pipe: R) -> io::Result<Reader> {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&bytes);
    let (tx, done) = mpsc::channel();
    std::thread::Builder::new()
        .name("quill-child-output".into())
        .spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Some(read) = chunk.get(..n) {
                            lock_or_recover(&sink).extend_from_slice(read);
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = tx.send(());
        })?;
    Ok(Reader { done, bytes })
}

/// What a reader read. A reader still open `grace` ([`READER_GRACE`]) after
/// the child exited means a grandchild kept the pipe (a daemon the helper started).
/// When the child itself succeeded the bytes read so far are its answer; after
/// a failed exit the cut output cannot be trusted and that is a timeout.
fn collect(reader: Option<Reader>, child_succeeded: bool, grace: Duration) -> io::Result<Vec<u8>> {
    let Some(reader) = reader else {
        return Ok(Vec::new());
    };
    let ended = reader.done.recv_timeout(grace).is_ok();
    if !ended && !child_succeeded {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the helper program's output did not end in time",
        ));
    }
    Ok(std::mem::take(&mut *lock_or_recover(&reader.bytes)))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script]);
        cmd
    }

    /// No limit: these children finish on their own, so passing never
    /// depends on the clock.
    const NO_LIMIT: Duration = Duration::MAX;

    #[test]
    fn output_is_captured() {
        let out = output_within(&mut sh("echo hi; echo oops >&2"), NO_LIMIT).unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout, b"hi\n");
        assert_eq!(out.stderr, b"oops\n");
    }

    #[test]
    fn large_output_does_not_stall_the_child() {
        let out = output_within(&mut sh("head -c 1000000 /dev/zero"), NO_LIMIT).unwrap();
        assert_eq!(out.stdout.len(), 1_000_000);
    }

    #[test]
    fn a_chatty_child_with_its_output_sent_away_does_not_stall_status_within() {
        let mut cmd = sh("head -c 1000000 /dev/zero; head -c 1000000 /dev/zero >&2");
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        let status = status_within(&mut cmd, NO_LIMIT).unwrap();
        assert!(status.success());
    }

    #[test]
    fn a_child_past_a_zero_limit_is_a_timeout_for_both_waits() {
        // `sleep 30` cannot have exited by the first check, and a zero
        // limit has passed by then, so this is decided without waiting.
        let err = output_within(&mut sh("sleep 30"), Duration::ZERO).unwrap_err();
        assert!(is_timeout(&err));
        let err = status_within(&mut sh("sleep 30"), Duration::ZERO).unwrap_err();
        assert!(is_timeout(&err));
    }

    use std::os::unix::process::ExitStatusExt;

    /// A child that exits after `exits_after` checks (never when `None`)
    /// and records what was done to it.
    #[derive(Default)]
    struct FakeChild {
        exits_after: Option<u32>,
        checks: u32,
        killed: bool,
        reaped: bool,
    }

    impl Running for FakeChild {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            self.checks += 1;
            let exited = self.killed || self.exits_after.is_some_and(|n| self.checks > n);
            Ok(exited.then(|| ExitStatus::from_raw(0)))
        }
        fn kill(&mut self) -> io::Result<()> {
            self.killed = true;
            Ok(())
        }
        fn wait(&mut self) -> io::Result<ExitStatus> {
            self.reaped = true;
            Ok(ExitStatus::from_raw(9))
        }
    }

    /// Time moves only when the wait sleeps.
    struct ManualClock {
        now: Instant,
        slept: Duration,
    }

    impl ManualClock {
        fn new() -> Self {
            Self {
                now: Instant::now(),
                slept: Duration::ZERO,
            }
        }
    }

    impl Clock for ManualClock {
        fn now(&self) -> Instant {
            self.now
        }
        fn sleep(&mut self, step: Duration) {
            self.now += step;
            self.slept += step;
        }
    }

    #[test]
    fn a_hung_child_times_out_once_the_limit_has_passed_and_is_killed_and_reaped() {
        let mut child = FakeChild::default();
        let mut clock = ManualClock::new();
        let waited = wait_on(&mut child, Duration::from_millis(200), &mut clock);
        assert!(waited.as_ref().is_err_and(is_timeout));
        assert_eq!(clock.slept, Duration::from_millis(200));
        assert!(!child.killed, "wait_within leaves the kill to its caller");
        let err = reap_on_error(&mut child, waited).unwrap_err();
        assert!(is_timeout(&err));
        assert!(child.killed && child.reaped);
    }

    #[test]
    fn a_child_that_exits_before_the_limit_is_not_killed() {
        let mut child = FakeChild {
            exits_after: Some(3),
            ..FakeChild::default()
        };
        let mut clock = ManualClock::new();
        let waited = wait_on(&mut child, Duration::from_secs(1), &mut clock);
        let status = reap_on_error(&mut child, waited).unwrap();
        assert!(status.success());
        assert_eq!(clock.slept, POLL * 3);
        assert!(!child.killed && !child.reaped);
    }

    #[test]
    fn an_unbounded_limit_never_times_out() {
        let mut child = FakeChild {
            exits_after: Some(1000),
            ..FakeChild::default()
        };
        let waited = wait_on(&mut child, Duration::MAX, &mut ManualClock::new());
        assert!(waited.unwrap().success());
    }

    /// A reader holding `bytes` whose pipe has ended (`ended`) or is still
    /// held open by a grandchild. The sender is returned so a held pipe
    /// stays open (not disconnected) for the whole test.
    fn reader(bytes: &[u8], ended: bool) -> (Reader, mpsc::Sender<()>) {
        let (tx, done) = mpsc::channel();
        if ended {
            tx.send(()).unwrap();
        }
        let reader = Reader {
            done,
            bytes: Arc::new(Mutex::new(bytes.to_vec())),
        };
        (reader, tx)
    }

    #[test]
    fn a_successful_child_whose_grandchild_holds_the_pipe_still_returns_its_output() {
        // A daemon the helper autolaunched inherited stdout and keeps it
        // open after the helper exited 0.
        let (held, _open) = reader(b"hi\n", false);
        assert_eq!(collect(Some(held), true, Duration::ZERO).unwrap(), b"hi\n");
    }

    #[test]
    fn output_cut_off_after_a_failed_exit_is_a_timeout() {
        let (held, _open) = reader(b"hi\n", false);
        let err = collect(Some(held), false, Duration::ZERO).unwrap_err();
        assert!(is_timeout(&err));
    }

    #[test]
    fn output_that_ended_is_kept_whatever_the_exit() {
        let (ended, _tx) = reader(b"partial", true);
        assert_eq!(
            collect(Some(ended), false, Duration::ZERO).unwrap(),
            b"partial"
        );
        assert!(collect(None, false, Duration::ZERO).unwrap().is_empty());
    }

    #[test]
    fn a_missing_program_is_a_spawn_error_not_a_timeout() {
        let err =
            output_within(&mut Command::new("/definitely/not/a/program"), NO_LIMIT).unwrap_err();
        assert!(!is_timeout(&err));
    }

    #[test]
    fn transcode_limit_scales_with_the_media_and_is_capped() {
        assert_eq!(transcode_limit(0.0), Duration::from_secs(120));
        assert_eq!(transcode_limit(f64::NAN), Duration::from_secs(120));
        assert_eq!(transcode_limit(10.0), Duration::from_secs(70));
        assert_eq!(transcode_limit(100_000.0), Duration::from_secs(600));
    }
}
