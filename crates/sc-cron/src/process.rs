//! Runs one child process so that a kill reaches everything it started.
//!
//! This is the process half of the Python hook runner (REQ-CRON-0013,
//! ADR-CRON-0008). The child is spawned as the leader of a Unix process
//! group, or inside a Windows job object, so that killing it also kills any
//! grandchild it started: a hook that forks a worker cannot outlive its
//! timeout (NFR-BDC-0004). The group or job is killed
//!
//! - when the caller's `expiry` future completes before the child is done, or
//! - when this future is dropped before the child is reaped, for instance
//!   because the worker aborted the run at its deadline.
//!
//! Standard output and standard error are each kept up to a cap; the rest is
//! read and discarded, so a child that floods a pipe is never blocked on it
//! and never grows this process's memory. Input is written concurrently with
//! reading, so a large input to a child that does not read it cannot stall
//! the supervisor.
//!
//! The group and job calls come from `process-wrap`, which keeps this crate
//! free of `unsafe`.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{ExitStatus, Stdio};

use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::ChildStdin;

#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;

/// How much of a pipe is read per call.
const READ_CHUNK: usize = 8 * 1024;

/// What to run: a program, its arguments, its working directory and its input.
#[derive(Debug)]
pub(crate) struct ChildSpec<'a> {
    pub(crate) program: &'a Path,
    pub(crate) args: Vec<OsString>,
    pub(crate) cwd: &'a Path,
    pub(crate) input: &'a [u8],
}

/// Up to a cap of one pipe's output, and whether more arrived.
#[derive(Debug, Default)]
pub(crate) struct Capped {
    pub(crate) bytes: Vec<u8>,
    pub(crate) truncated: bool,
}

impl Capped {
    fn push(&mut self, data: &[u8], cap: usize) {
        let room = cap.saturating_sub(self.bytes.len());
        let (kept, dropped) = data.split_at(data.len().min(room));
        self.bytes.extend_from_slice(kept);
        self.truncated |= !dropped.is_empty();
    }
}

/// What a supervised child left behind.
#[derive(Debug)]
pub(crate) struct Captured {
    /// The exit status, or `None` when the child was killed at `expiry`.
    pub(crate) status: Option<ExitStatus>,
    pub(crate) stdout: Capped,
    pub(crate) stderr: Capped,
}

/// Kills the child's group or job on drop unless the child was reaped.
struct Tree {
    child: Box<dyn ChildWrapper>,
    reaped: bool,
}

impl Drop for Tree {
    fn drop(&mut self) {
        if !self.reaped {
            // Already gone is fine; a kill that fails here cannot be retried.
            let _ = self.child.start_kill();
        }
    }
}

/// Spawns `spec` in its own group or job and waits for it, up to `expiry`.
///
/// Returns the child's status and capped output once it and its output
/// pipes are done, or, if `expiry` completes first, kills the whole group or
/// job, reaps it, and returns the output read so far with no status.
///
/// # Errors
///
/// The spawn failed, or waiting for the child failed (the group is killed
/// before the error is returned).
pub(crate) async fn supervise(
    spec: &ChildSpec<'_>,
    cap: usize,
    expiry: impl Future<Output = ()>,
) -> io::Result<Captured> {
    let mut command = CommandWrap::with_new(spec.program, |command| {
        command
            .args(&spec.args)
            .current_dir(spec.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    });
    command.wrap(KillOnDrop);
    #[cfg(unix)]
    command.wrap(ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(JobObject);
    let mut tree = Tree {
        child: command.spawn()?,
        reaped: false,
    };
    let stdin = tree.child.stdin().take();
    let stdout = tree.child.stdout().take();
    let stderr = tree.child.stderr().take();
    let (mut out, mut err) = (Capped::default(), Capped::default());

    let finished = {
        let io = async {
            let (status, (), (), ()) = tokio::join!(
                tree.child.wait(),
                feed(stdin, spec.input),
                drain(stdout, &mut out, cap),
                drain(stderr, &mut err, cap),
            );
            status
        };
        tokio::pin!(io);
        tokio::select! {
            biased;
            status = &mut io => Some(status),
            () = expiry => None,
        }
    };
    let status = if let Some(status) = finished {
        let status = status?;
        tree.reaped = true;
        Some(status)
    } else {
        if let Err(error) = tree.child.start_kill() {
            tracing::debug!(subsystem = "sc_cron", action = "hook_kill", %error,
                "kill at expiry found nothing to kill: {error}");
        }
        // The group is dead; this reaps it.
        let _ = tree.child.wait().await;
        tree.reaped = true;
        None
    };
    Ok(Captured {
        status,
        stdout: out,
        stderr: err,
    })
}

/// Writes `input` to the child and closes its stdin.
///
/// A child that exits or closes stdin without reading it breaks the pipe;
/// its exit status is the answer then, so the write error is not one.
async fn feed(stdin: Option<ChildStdin>, input: &[u8]) {
    let Some(mut stdin) = stdin else { return };
    let _ = stdin.write_all(input).await;
    let _ = stdin.shutdown().await;
}

/// Reads `pipe` to its end, keeping the first `cap` bytes and dropping the rest.
async fn drain<R: AsyncRead + Unpin>(pipe: Option<R>, sink: &mut Capped, cap: usize) {
    let Some(mut pipe) = pipe else { return };
    // On the heap: two of these futures are alive at once, and an array of
    // this size would put their state past what a task should carry.
    let mut chunk = vec![0_u8; READ_CHUNK];
    loop {
        match pipe.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(read) => sink.push(chunk.get(..read).unwrap_or_default(), cap),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_keeps_the_first_cap_bytes_and_flags_the_rest() {
        let mut capped = Capped::default();
        capped.push(b"abc", 4);
        assert_eq!(
            (capped.bytes.as_slice(), capped.truncated),
            (&b"abc"[..], false)
        );
        capped.push(b"defg", 4);
        assert_eq!(
            (capped.bytes.as_slice(), capped.truncated),
            (&b"abcd"[..], true)
        );
        capped.push(b"h", 4);
        assert_eq!(capped.bytes, b"abcd");
    }

    #[test]
    fn capped_at_exactly_the_cap_is_not_truncated() {
        let mut capped = Capped::default();
        capped.push(b"abcd", 4);
        assert_eq!(
            (capped.bytes.as_slice(), capped.truncated),
            (&b"abcd"[..], false)
        );
    }

    /// A shell script run as the child, in a directory of its own.
    #[cfg(unix)]
    mod unix {
        use std::fs;
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        use std::process::Command;
        use std::sync::mpsc;
        use std::time::Duration;

        use super::*;

        /// Starts a background process in the child's group that holds the
        /// write end of the named pipe `life`, signals the named pipe `ready`
        /// and sleeps. Nothing forks after the signal, so every member of the
        /// group exists when the test hears it.
        const TREE: &str = r#"#!/bin/sh
cd "$1"
exec 3> life
( echo up > ready; exec sleep 3600 ) &
wait
"#;

        struct Tree {
            dir: tempfile::TempDir,
            script: PathBuf,
        }

        impl Tree {
            fn new() -> Self {
                let dir = tempfile::tempdir().expect("tempdir");
                let script = dir.path().join("tree.sh");
                fs::write(&script, TREE).expect("script");
                fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");
                for pipe in ["ready", "life"] {
                    let made = Command::new("mkfifo").arg(dir.path().join(pipe)).status();
                    assert!(made.expect("mkfifo").success());
                }
                Self { dir, script }
            }

            fn spec(&self) -> ChildSpec<'_> {
                ChildSpec {
                    program: &self.script,
                    args: vec![self.dir.path().into()],
                    cwd: self.dir.path(),
                    input: b"",
                }
            }

            /// Reads `pipe` to its end on a thread of its own: the value
            /// arrives when every writer has closed it.
            fn read(&self, pipe: &str) -> mpsc::Receiver<()> {
                let path = self.dir.path().join(pipe);
                let (sender, receiver) = mpsc::channel();
                std::thread::spawn(move || {
                    let mut sink = Vec::new();
                    if let Ok(mut pipe) = fs::File::open(path) {
                        let _ = pipe.read_to_end(&mut sink);
                    }
                    let _ = sender.send(());
                });
                receiver
            }
        }

        async fn wait(receiver: mpsc::Receiver<()>, what: &str) {
            let heard =
                tokio::task::spawn_blocking(move || receiver.recv_timeout(Duration::from_secs(10)))
                    .await
                    .expect("join");
            assert!(heard.is_ok(), "timed out waiting for {what}");
        }

        #[tokio::test]
        async fn expiry_kills_the_child_and_everything_it_started() {
            let tree = Tree::new();
            let (ready, life) = (tree.read("ready"), tree.read("life"));
            let (expire, expired) = tokio::sync::oneshot::channel::<()>();
            let spec = tree.spec();
            let run = supervise(&spec, 1024, async {
                let _ = expired.await;
            });
            let drive = async {
                wait(ready, "the tree to start").await;
                expire.send(()).expect("supervisor is waiting");
                wait(life, "the whole tree to die").await;
            };
            let (captured, ()) = tokio::join!(run, drive);
            assert!(
                captured.expect("supervise").status.is_none(),
                "killed, no status"
            );
        }

        #[tokio::test]
        async fn dropping_the_supervisor_kills_the_child_and_everything_it_started() {
            let tree = Tree::new();
            let (ready, life) = (tree.read("ready"), tree.read("life"));
            let spec = tree.spec();
            let run = supervise(&spec, 1024, std::future::pending());
            tokio::select! {
                _ = run => panic!("the tree never ends by itself"),
                () = wait(ready, "the tree to start") => {}
            }
            // `run` is dropped; the group dies with it.
            wait(life, "the whole tree to die").await;
        }

        #[tokio::test]
        async fn a_child_that_exits_reports_its_status_and_output() {
            let dir = tempfile::tempdir().expect("tempdir");
            let spec = ChildSpec {
                program: Path::new("/bin/sh"),
                args: vec!["-c".into(), "cat; echo out; echo err >&2; exit 7".into()],
                cwd: dir.path(),
                input: b"in\n",
            };
            let captured = supervise(&spec, 1024, std::future::pending())
                .await
                .expect("supervise");
            assert_eq!(captured.status.and_then(|status| status.code()), Some(7));
            assert_eq!(captured.stdout.bytes, b"in\nout\n");
            assert_eq!(captured.stderr.bytes, b"err\n");
            assert!(!captured.stdout.truncated && !captured.stderr.truncated);
        }
    }
}
