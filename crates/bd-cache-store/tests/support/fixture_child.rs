//! Owns an exactly filtered test process, its scratch files and registered children.

use std::io;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub(crate) struct FixtureChild {
    pub(crate) process: Child,
    output: tempfile::NamedTempFile,
    pub(crate) scratch: tempfile::TempDir,
    pub(crate) cleaned: Arc<AtomicBool>,
    pub(crate) started: Instant,
    pub(crate) cleanup_on_drop: bool,
    spawn_returned_after: Duration,
    owner_unix_ns: u64,
    cleanup_attempted: bool,
}

impl FixtureChild {
    pub(crate) fn spawn(test: &str, key: &str, mode: &str) -> io::Result<Self> {
        let output = tempfile::NamedTempFile::new()?;
        let scratch = tempfile::tempdir()?;
        let stdout = output.reopen()?;
        let stderr = stdout.try_clone()?;
        let started = Instant::now();
        // Diagnostic wall correlation only; the lifetime always uses started.
        let owner_unix_ns = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos(),
        )
        .map_err(io::Error::other)?;
        let process = Command::new(std::env::current_exe()?)
            .args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env(key, mode)
            .env("BD_CACHE_FIXTURE_ROOT", scratch.path())
            .env("BD_CACHE_FIXTURE_OWNER_UNIX_NS", owner_unix_ns.to_string())
            .env("TMPDIR", scratch.path())
            .env("TMP", scratch.path())
            .env("TEMP", scratch.path())
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()?;
        let spawn_returned_after = started.elapsed();
        Ok(Self {
            process,
            output,
            scratch,
            cleaned: Arc::default(),
            started,
            cleanup_on_drop: true,
            spawn_returned_after,
            owner_unix_ns,
            cleanup_attempted: false,
        })
    }

    pub(crate) fn finish(&mut self, lifetime: Duration) -> io::Result<Option<ExitStatus>> {
        self.finish_observing(lifetime, |_, _, _, _| Ok(()))
    }

    // The optional observer runs inline under the same original absolute clock.
    // Its error or unwind leaves this owner responsible for actual cleanup.
    pub(crate) fn finish_observing(
        &mut self,
        lifetime: Duration,
        mut observe: impl FnMut(&Self, bool, Duration, u64) -> io::Result<()>,
    ) -> io::Result<Option<ExitStatus>> {
        let deadline = self.started + lifetime;
        loop {
            if let Some(status) = self.process.try_wait()? {
                self.kill_and_reap()?;
                observe(self, true, self.spawn_returned_after, self.owner_unix_ns)?;
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                self.kill_and_reap()?;
                observe(self, true, self.spawn_returned_after, self.owner_unix_ns)?;
                return Ok(None);
            }
            observe(self, false, self.spawn_returned_after, self.owner_unix_ns)?;
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(crate) fn output(&self) -> io::Result<String> {
        std::fs::read_to_string(self.output.path())
    }

    pub(crate) fn cleanup_receipt(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cleaned)
    }

    pub(crate) fn wait_for_output(&self, needle: &str, lifetime: Duration) -> io::Result<bool> {
        let deadline = Instant::now() + lifetime;
        loop {
            if self.output()?.contains(needle) {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(crate) fn owned_pids(&self) -> io::Result<Vec<u32>> {
        let mut pids = Vec::new();
        for entry in std::fs::read_dir(self.scratch.path())? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("owned-child-") && name.ends_with(".pid") {
                let text = std::fs::read_to_string(entry.path())?;
                let pid = text.trim().parse::<u32>().map_err(io::Error::other)?;
                if pid <= 1 {
                    return Err(io::Error::other("invalid registered child PID"));
                }
                pids.push(pid);
            }
        }
        Ok(pids)
    }

    fn kill_and_reap(&mut self) -> io::Result<()> {
        if std::mem::replace(&mut self.cleanup_attempted, true) {
            return Err(io::Error::other("owned fixture cleanup already attempted"));
        }
        // An incomplete receipt stays incomplete; Drop cannot create a retry.
        self.cleanup_on_drop = false;
        let (pids, mut failure) = match self.owned_pids() {
            Ok(pids) => (pids, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        // Python supervisors create separate process groups. Kill those first
        // while their actual parent/runtime is alive and can perform its wait.
        for pid in &pids {
            if let Err(error) = kill_owned_group(*pid) {
                failure = Some(error);
            }
        }
        let _ = children_gone(&pids, Duration::from_millis(250));
        let _ = self.process.kill();
        if let Err(error) = self.process.wait() {
            failure = Some(error);
        }
        // Confirm descendant disappearance as well as reaping the test child.
        // This also covers a fixture that already exited during an assertion.
        match children_gone(&pids, Duration::from_millis(750)) {
            Ok(true) => {}
            Ok(false) => failure = Some(io::Error::other("registered child was not reaped")),
            Err(error) => failure = Some(error),
        }
        if let Some(error) = failure {
            return Err(error);
        }
        self.cleaned.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for FixtureChild {
    fn drop(&mut self) {
        if self.cleanup_on_drop
            && !self.cleaned.load(Ordering::SeqCst)
            && let Err(error) = self.kill_and_reap()
        {
            eprintln!("owned fixture cleanup failed: {error}");
        }
    }
}

fn children_gone(pids: &[u32], lifetime: Duration) -> io::Result<bool> {
    let deadline = Instant::now() + lifetime;
    loop {
        let mut gone = true;
        for pid in pids {
            gone &= !pid_exists(*pid)?;
        }
        if gone {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
pub(crate) fn pid_exists(pid: u32) -> io::Result<bool> {
    let output = Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()?;
    if output.status.success() {
        return Ok(true);
    }
    if String::from_utf8_lossy(&output.stderr)
        .to_lowercase()
        .contains("no such process")
    {
        return Ok(false);
    }
    Err(io::Error::other(format!(
        "cannot inspect owned child {pid}: {}",
        String::from_utf8_lossy(&output.stderr)
    )))
}

#[cfg(not(unix))]
pub(crate) fn pid_exists(_pid: u32) -> io::Result<bool> {
    Err(io::Error::other("descendant fixtures require Unix"))
}

#[cfg(unix)]
fn kill_owned_group(pid: u32) -> io::Result<()> {
    if !pid_exists(pid)? {
        return Ok(());
    }
    let group = Command::new("/bin/kill")
        .args(["-KILL", &format!("-{pid}")])
        .output()?;
    if group.status.success() || !pid_exists(pid)? {
        return Ok(());
    }
    let child = Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .output()?;
    if child.status.success() || !pid_exists(pid)? {
        return Ok(());
    }
    Err(io::Error::other(format!("cannot kill owned child {pid}")))
}

#[cfg(not(unix))]
fn kill_owned_group(_pid: u32) -> io::Result<()> {
    Err(io::Error::other("descendant fixtures require Unix"))
}

pub(crate) fn assert_passed(child: &mut FixtureChild) -> io::Result<()> {
    let status = child.finish(Duration::from_secs(5))?;
    let output = child.output()?;
    assert!(
        status.is_some_and(|status| status.success()),
        "owned fixture failed or exceeded five seconds: {output}"
    );
    assert!(
        output.contains("1 passed"),
        "exact fixture did not run: {output}"
    );
    Ok(())
}
