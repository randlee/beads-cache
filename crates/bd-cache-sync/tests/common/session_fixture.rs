//! Lifecycle test ownership is the kernel session, established before job launch.
//! Child-written PID receipts are diagnostics, never the cleanup authority.

use std::io;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustix::io::Errno;
use rustix::process::{Pid, Signal, WaitOptions, kill_process, kill_process_group, waitpid};
#[cfg(not(target_os = "linux"))]
use rustix::process::{getpgid, getsid};

use super::fixture_child::FixtureChild;

#[derive(Debug)]
pub(crate) struct SessionFixture {
    child: FixtureChild,
    session: Pid,
    cleanup_attempted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Member {
    pub(crate) pid: Pid,
    pub(crate) group: Pid,
    quiet: bool,
}

impl SessionFixture {
    pub(crate) fn spawn(test: &str, key: &str, mode: &str) -> io::Result<Self> {
        // Refuse unavailable process inventory before starting work we must own.
        process_ids()?;
        let mut child = FixtureChild::spawn(test, key, mode)?;
        let session = Pid::from_child(&child.process);
        // Transfer cleanup ownership; a failed session cleanup must not restart
        // the registry supervisor with a fresh budget during field destruction.
        child.cleanup_on_drop = false;
        Ok(Self {
            child,
            session,
            cleanup_attempted: false,
        })
    }

    pub(crate) fn finish(&mut self) -> io::Result<ExitStatus> {
        let status = self.child.process.wait()?;
        self.cleanup_using(&mut process_ids)?;
        Ok(status)
    }

    pub(crate) fn terminate(&mut self) -> io::Result<ExitStatus> {
        let _ = kill_process(self.session, Signal::KILL);
        let _ = self.child.process.kill();
        let status = self.child.process.wait()?;
        self.cleanup_using(&mut process_ids)?;
        Ok(status)
    }

    pub(crate) fn output(&self) -> io::Result<String> {
        self.child.output()
    }

    pub(crate) fn cleanup_receipt(&self) -> Arc<AtomicBool> {
        self.child.cleanup_receipt()
    }

    pub(crate) fn wait_for_output(&mut self, needle: &str) -> io::Result<bool> {
        loop {
            if self.child.output()?.contains(needle) {
                return Ok(true);
            }
            if self.child.process.try_wait()?.is_some() {
                return Ok(false);
            }
            std::thread::yield_now();
        }
    }

    pub(crate) fn owned_pids(&self) -> io::Result<Vec<u32>> {
        Ok(self
            .members()?
            .into_iter()
            .map(|member| member.pid.as_raw_pid().cast_unsigned())
            .collect())
    }

    pub(crate) fn session_id(&self) -> Pid {
        self.session
    }

    pub(crate) fn members(&self) -> io::Result<Vec<Member>> {
        Ok(session_members(self.session)?
            .into_iter()
            .filter(|member| member.pid != self.session)
            .collect())
    }

    pub(crate) fn wait_for_unregistered_launch(&mut self) -> io::Result<Vec<Member>> {
        loop {
            if self.child.process.try_wait()?.is_some() {
                return Err(io::Error::other(
                    "fixture exited before the unregistered launch",
                ));
            }
            let ready = self
                .child
                .scratch
                .path()
                .join("pre-registration.ready")
                .exists()
                && self.child.owned_pids()?.is_empty();
            if ready {
                let members = self.members()?;
                if members.len() >= 2 {
                    return Ok(members);
                }
            }
            std::thread::yield_now();
        }
    }

    fn cleanup(&mut self) -> io::Result<()> {
        self.cleanup_using(&mut process_ids)
    }

    fn cleanup_using(
        &mut self,
        inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
    ) -> io::Result<()> {
        if self.cleanup_attempted {
            return if self.child.cleaned.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(io::Error::other("session cleanup was already attempted"))
            };
        }
        self.cleanup_attempted = true;
        let mut calls = 0;
        let mut failure = None;
        // Stop the root and acknowledge a stable stopped session before kill.
        // Kernel membership covers groups born before registration; no stopped
        // member can fork another child between this census and termination.
        match quiesce(self.session, &mut || {
            calls += 1;
            inventory()
        }) {
            Ok(members) => {
                for member in members {
                    if member.pid != self.session {
                        kill_member(self.session, member);
                    }
                }
            }
            Err(error) => failure = Some(error),
        }
        let _ = self.child.process.kill();
        let root_wait = self.child.process.wait();
        if let Err(error) = root_wait {
            failure = Some(error);
        }
        // The real root wait closes all in-flight fixture launches. Re-enumerate
        // the inherited session after that join, including reparented processes.
        let drain_result = if failure.is_some() {
            // A failed injected/real quiescence census cannot own cleanup.
            // Recover ownership through a fresh production inventory while
            // preserving the original failed outcome below.
            drain(self.session, &mut process_ids)
        } else {
            drain(self.session, &mut || {
                calls += 1;
                inventory()
            })
        };
        match drain_result {
            Ok(true) => {}
            Ok(false) => {
                failure = Some(io::Error::other("owned session still has unreaped members"));
            }
            Err(error) => failure = Some(error),
        }
        if let Some(error) = failure {
            eprintln!(
                "RQA10_CLEANUP session={} calls={calls} cleaned=false error={error}",
                self.session.as_raw_pid(),
            );
            return Err(error);
        }
        self.child.cleaned.store(true, Ordering::SeqCst);
        eprintln!(
            "session cleanup completed session={} inventory_calls={calls}",
            self.session.as_raw_pid()
        );
        Ok(())
    }
}

fn audit_members_using(
    session: Pid,
    inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<Member>> {
    members_before(session, inventory)
}

impl Drop for SessionFixture {
    fn drop(&mut self) {
        if !self.cleanup_attempted
            && !self.child.cleaned.load(Ordering::SeqCst)
            && let Err(error) = self.cleanup()
        {
            eprintln!("session fixture cleanup failed: {error}");
        }
    }
}

/// Own both the inventory process and its output; no pipe reader can outlive it.
#[derive(Debug)]
struct Inventory {
    process: Child,
    output: tempfile::NamedTempFile,
}

impl Inventory {
    fn spawn() -> io::Result<Self> {
        Self::spawn_command(Command::new("/bin/ps").args(["-axo", "pid=,stat="]))
    }

    fn spawn_command(command: &mut Command) -> io::Result<Self> {
        let output = tempfile::NamedTempFile::new()?;
        let process = command
            .stdin(Stdio::null())
            .stdout(output.reopen()?)
            .stderr(output.reopen()?)
            .spawn()?;
        Ok(Self { process, output })
    }

    fn finish(&mut self) -> io::Result<String> {
        let status = self.process.wait()?;
        let output = std::fs::read_to_string(self.output.path())?;
        if status.success() {
            Ok(output)
        } else {
            Err(io::Error::other(format!(
                "owned session inventory failed: {output}"
            )))
        }
    }

    fn cancel(&mut self) -> io::Result<()> {
        let _ = self.process.kill();
        self.process.wait()?;
        Ok(())
    }

    fn reap(&mut self) -> io::Result<()> {
        if self.process.try_wait()?.is_none() {
            self.process.kill()?;
        }
        self.process.wait()?;
        Ok(())
    }
}

impl Drop for Inventory {
    fn drop(&mut self) {
        if let Err(error) = self.reap() {
            eprintln!("inventory child cleanup failed: {error}");
        }
    }
}

fn process_ids() -> io::Result<Vec<(Pid, bool)>> {
    process_ids_using(Inventory::spawn)
}

fn process_ids_using(
    spawn: impl FnOnce() -> io::Result<Inventory>,
) -> io::Result<Vec<(Pid, bool)>> {
    // Inventory completion is the handshake: the caller waits for the actual
    // scan process and owns/reaps it before parsing its complete output.
    let output = spawn()?.finish()?;
    let pids: Vec<_> = output
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let raw = fields
                .next()
                .ok_or_else(|| io::Error::other("missing process PID"))?
                .parse::<i32>()
                .map_err(io::Error::other)?;
            let state = fields
                .next()
                .ok_or_else(|| io::Error::other("missing process state"))?;
            if raw <= 0 {
                return Err(io::Error::other("nonpositive process inventory PID"));
            }
            let pid = Pid::from_raw(raw)
                .ok_or_else(|| io::Error::other("invalid process inventory PID"))?;
            Ok((pid, state.starts_with(['T', 't', 'Z', 'X'])))
        })
        .collect::<io::Result<_>>()?;
    if !pids
        .iter()
        .any(|(pid, _)| *pid == rustix::process::getpid())
    {
        return Err(io::Error::other(
            "session inventory omitted its observing process",
        ));
    }
    Ok(pids)
}

// Linux permits SID/PGID zero for foreign kernel tasks. rustix's typed getsid
// assumes strictly positive success, so never call it on an all-host Linux PID.
// Read the kernel identity as raw integers and validate before constructing Pid.
#[cfg(target_os = "linux")]
fn identity(pid: Pid) -> io::Result<Option<(Pid, Pid)>> {
    match std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_pid())) {
        Ok(stat) => parse_linux_identity(&stat),
        Err(error)
            if error.raw_os_error() == Some(Errno::SRCH.raw_os_error())
                || matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

#[cfg(any(target_os = "linux", test))]
fn parse_linux_identity(stat: &str) -> io::Result<Option<(Pid, Pid)>> {
    // comm may itself contain spaces or ')'; fields after its final ')' are
    // state, ppid, pgrp, session. No foreign raw SID reaches typed-PID APIs.
    let (_, tail) = stat
        .rsplit_once(')')
        .ok_or_else(|| io::Error::other("invalid process stat comm"))?;
    let mut fields = tail.split_whitespace().skip(2);
    let group = fields
        .next()
        .ok_or_else(|| io::Error::other("missing process group"))?
        .parse::<i32>()
        .map_err(io::Error::other)?;
    let session = fields
        .next()
        .ok_or_else(|| io::Error::other("missing process session"))?
        .parse::<i32>()
        .map_err(io::Error::other)?;
    if session <= 0 || group <= 0 {
        return Ok(None);
    }
    Ok(Pid::from_raw(session).zip(Pid::from_raw(group)))
}

#[cfg(not(target_os = "linux"))]
fn identity(pid: Pid) -> io::Result<Option<(Pid, Pid)>> {
    match getsid(Some(pid)) {
        Ok(session) => match getpgid(Some(pid)) {
            Ok(group) => Ok(Some((session, group))),
            Err(Errno::SRCH) => Ok(None),
            Err(error) => Err(error.into()),
        },
        Err(Errno::SRCH | Errno::PERM | Errno::ACCESS) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn session_members(session: Pid) -> io::Result<Vec<Member>> {
    members_before(session, &mut process_ids)
}

fn members_before(
    session: Pid,
    inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<Member>> {
    let mut members = Vec::new();
    for (pid, quiet) in inventory()? {
        if let Some((owner, group)) = identity(pid)?
            && owner == session
        {
            members.push(Member { pid, group, quiet });
        }
    }
    Ok(members)
}

fn drain(
    session: Pid,
    inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<bool> {
    loop {
        let members = members_before(session, inventory)?;
        if members.is_empty() {
            return Ok(true);
        }
        for member in members {
            kill_member(session, member);
        }
    }
}

fn kill_member(session: Pid, member: Member) {
    if identity(member.pid).ok().flatten() == Some((session, member.group)) {
        if member.group != session {
            let _ = kill_process_group(member.group, Signal::KILL);
        }
        let _ = kill_process(member.pid, Signal::KILL);
    }
}

fn quiesce(
    session: Pid,
    inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<Member>> {
    if identity(session)?.is_some() {
        let _ = kill_process(session, Signal::STOP);
        // The fixture root is our child. Wait for the kernel's stopped-state
        // notification instead of sampling it after a delay.
        let _ = waitpid(Some(session), WaitOptions::UNTRACED)?;
    }
    let mut scan_failed = false;
    loop {
        let members = match members_before(session, inventory) {
            Ok(members) => members,
            Err(error) => {
                if scan_failed {
                    return Err(error);
                }
                scan_failed = true;
                continue;
            }
        };
        if members.iter().all(|member| member.quiet) {
            return Ok(members);
        }
        for member in &members {
            if !member.quiet && identity(member.pid).ok().flatten() == Some((session, member.group))
            {
                if member.group != session {
                    let _ = kill_process_group(member.group, Signal::STOP);
                }
                let _ = kill_process(member.pid, Signal::STOP);
            }
        }
    }
}

pub(crate) fn assert_passed(child: &mut SessionFixture) -> io::Result<()> {
    let status = child.finish()?;
    let output = child.output()?;
    assert!(status.success(), "session fixture failed: {output}");
    assert!(
        output.contains("1 passed"),
        "exact fixture did not run: {output}"
    );
    Ok(())
}

#[test]
fn foreign_nonpositive_session_identity_is_excluded_before_typed_pid_use() -> io::Result<()> {
    for (group, session) in [(0, 0), (1, 0), (0, 1), (-1, 4), (4, -1)] {
        assert_eq!(
            parse_linux_identity(&format!("2 (foreign ) task) S 0 {group} {session} 0"))?,
            None
        );
    }
    assert_eq!(
        parse_linux_identity("12 (ordinary ) task) S 1 12 12 0")?,
        Some((Pid::from_raw(12).unwrap(), Pid::from_raw(12).unwrap()))
    );
    assert!(parse_linux_identity("invalid").is_err());
    Ok(())
}

#[test]
fn stopped_inventory_is_reaped_on_explicit_cancel_and_unwind() -> io::Result<()> {
    for unwind in [false, true] {
        let mut inventory = stopped_inventory()?;
        let pid = Pid::from_child(&inventory.process);
        if unwind {
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _owner = inventory;
                panic!("injected inventory assertion unwind");
            }));
            assert!(failure.is_err());
        } else {
            inventory.cancel()?;
        }
        assert_eq!(rustix::process::test_kill_process(pid), Err(Errno::SRCH));
    }
    Ok(())
}

/// The child acknowledges its actual stop through waitpid before the test
/// releases it. This makes the held-inventory path a controlled handshake.
fn stopped_inventory() -> io::Result<Inventory> {
    let inventory = Inventory::spawn_command(
        Command::new("/bin/sh").args(["-c", "kill -STOP $$; exec /bin/ps -axo pid=,stat="]),
    )?;
    let pid = Pid::from_child(&inventory.process);
    let (_, status) = waitpid(Some(pid), WaitOptions::UNTRACED)?
        .ok_or_else(|| io::Error::other("stopped inventory returned no wait status"))?;
    if status.stopped() {
        Ok(inventory)
    } else {
        Err(io::Error::other(
            "inventory exited before its injected stop",
        ))
    }
}

#[test]
fn inventory_waits_for_scan_completion_handshake_and_reaps_its_command() -> io::Result<()> {
    let inventory = stopped_inventory()?;
    let pid = Pid::from_child(&inventory.process);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let scan = std::thread::scope(|scope| {
        let resume_thread = scope.spawn(move || {
            release_rx.recv().expect("scan requested release");
            kill_process(pid, Signal::CONT).expect("resume held inventory");
        });
        let pids = process_ids_using(|| {
            release_tx.send(()).expect("resume thread is waiting");
            Ok(inventory)
        })?;
        resume_thread
            .join()
            .map_err(|panic| io::Error::other(format!("inventory releaser panicked: {panic:?}")))?;
        Ok::<_, io::Error>(pids)
    })?;
    assert!(
        scan.iter()
            .any(|(pid, _)| *pid == rustix::process::getpid())
    );
    assert_eq!(rustix::process::test_kill_process(pid), Err(Errno::SRCH));
    Ok(())
}

const INVENTORY_FIXTURE_MODE: &str = "BD_CACHE_INVENTORY_FIXTURE_MODE";
const INVENTORY_FIXTURE_TEST: &str =
    "session_fixture::cleanup_inventory_recovery_preserves_the_original_fixture_failure";

#[test]
fn cleanup_inventory_recovery_preserves_the_original_fixture_failure() -> io::Result<()> {
    if let Ok(mode) = std::env::var(INVENTORY_FIXTURE_MODE) {
        rustix::process::setsid()?;
        eprintln!("inventory fixture session established");
        if mode == "wait" {
            loop {
                std::thread::park();
            }
        }
        panic!("original fixture failure must remain a failure");
    }
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "fail")?;
    assert!(child.wait_for_output("inventory fixture session established")?);
    let session = child.session;
    let mut scans = 0;
    // Inject one transient inventory error, then recover through the real
    // process inventory while preserving the fixture's own failed status.
    let status = child.child.process.wait()?;
    let result = child.cleanup_using(&mut || {
        scans += 1;
        if scans == 1 {
            Err(io::Error::other("injected transient inventory failure"))
        } else {
            process_ids()
        }
    });
    result?;
    assert!(!status.success());
    assert!(
        child
            .output()?
            .contains("original fixture failure must remain a failure")
    );
    assert!(child.child.cleaned.load(Ordering::SeqCst));
    assert!(
        scans >= 3,
        "recovery must include a retry and a final census"
    );
    assert!(audit_members_using(session, &mut process_ids)?.is_empty());
    assert_eq!(
        rustix::process::test_kill_process(session),
        Err(Errno::SRCH)
    );
    eprintln!(
        "FTQ-011 transient census recovered via {scans} scans; original fixture failed/reaped"
    );
    Ok(())
}

#[test]
fn incomplete_inventory_never_marks_cleanup_success_or_retries_the_attempt() -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "wait")?;
    assert!(child.wait_for_output("inventory fixture session established")?);
    let session = child.session;
    let receipt = child.cleanup_receipt();
    let mut scans = 0;
    let error = child
        .cleanup_using(&mut || {
            scans += 1;
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "injected incomplete inventory result",
            ))
        })
        .expect_err("incomplete required censuses must not prove successful cleanup");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(!receipt.load(Ordering::SeqCst));
    let attempted = child.cleanup_attempted;
    assert!(
        child.cleanup().is_err(),
        "a second cleanup must not gain a fresh budget"
    );
    assert_eq!(child.cleanup_attempted, attempted);
    drop(child);
    assert!(!receipt.load(Ordering::SeqCst));
    assert_eq!(
        rustix::process::test_kill_process(session),
        Err(Errno::SRCH)
    );
    assert_eq!(
        scans, 2,
        "the controlled failure requires two incomplete censuses"
    );
    // This separate audit is evidence only; it cannot retroactively set receipt.
    assert!(session_members(session)?.is_empty());
    assert!(!receipt.load(Ordering::SeqCst));
    eprintln!("FTQ-011 injected incomplete census scans={scans}; failed cleanup retained");
    Ok(())
}

#[test]
fn cleanup_and_post_audit_timeouts_retain_distinct_owned_outcomes() -> io::Result<()> {
    for post_audit in [false, true] {
        stage_timeout_control(post_audit)?;
    }
    Ok(())
}

fn stage_timeout_control(post_audit: bool) -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "fail")?;
    assert!(child.wait_for_output("inventory fixture session established")?);
    let session = child.session;
    let status = child.child.process.wait()?;
    assert!(
        !status.success(),
        "the fixture's panic must remain a failure"
    );
    let mut scans = 0;
    let result = child.cleanup_using(&mut || {
        scans += 1;
        if !post_audit && scans <= 2 {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "injected cleanup timeout",
            ))
        } else {
            process_ids()
        }
    });
    if post_audit {
        result?;
        assert!(child.child.cleaned.load(Ordering::SeqCst));
        let error = audit_members_using(session, &mut || {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "injected audit timeout",
            ))
        })
        .err()
        .ok_or_else(|| io::Error::other("held independent audit unexpectedly succeeded"))?;
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        // Audit failure is not silently accepted as an empty census and does
        // not rewrite the already completed cleanup's separate receipt.
        assert!(child.child.cleaned.load(Ordering::SeqCst));
    } else {
        assert_eq!(
            result
                .err()
                .ok_or_else(|| io::Error::other("injected cleanup failure unexpectedly succeeded"))?
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(!child.child.cleaned.load(Ordering::SeqCst));
        let original_attempt = child.cleanup_attempted;
        assert!(child.cleanup().is_err());
        assert_eq!(child.cleanup_attempted, original_attempt);
    }
    assert!(
        child
            .output()?
            .contains("original fixture failure must remain a failure")
    );
    assert_eq!(
        rustix::process::test_kill_process(session),
        Err(Errno::SRCH)
    );
    // A fresh independent audit establishes actual emptiness without changing
    // the failed cleanup/audit observation or granting another cleanup attempt.
    assert!(session_members(session)?.is_empty());
    assert_eq!(child.child.cleaned.load(Ordering::SeqCst), post_audit);
    eprintln!(
        "RQA10_CONTROL post_audit={post_audit} actual_root_wait=true complete_independent_census=true cleanup_receipt={post_audit} original_fixture_failed=true"
    );
    Ok(())
}
