//! Lifecycle test ownership is the kernel session, established before job launch.
//! Child-written PID receipts are diagnostics, never the cleanup authority.

use std::io;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::io::Errno;
use rustix::process::{Pid, Signal, kill_process, kill_process_group};
#[cfg(not(target_os = "linux"))]
use rustix::process::{getpgid, getsid};

use super::fixture_child::FixtureChild;

#[derive(Debug)]
pub(crate) struct SessionFixture {
    child: FixtureChild,
    session: Pid,
    cleanup_started: Option<Instant>,
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
        process_ids(Instant::now() + Duration::from_millis(250))?;
        let mut child = FixtureChild::spawn(test, key, mode)?;
        let session = Pid::from_child(&child.process);
        // Transfer cleanup ownership; a failed session cleanup must not restart
        // the registry supervisor with a fresh budget during field destruction.
        child.cleanup_on_drop = false;
        Ok(Self {
            child,
            session,
            cleanup_started: None,
        })
    }

    pub(crate) fn finish(&mut self, lifetime: Duration) -> io::Result<Option<ExitStatus>> {
        let deadline = self.child.started + lifetime;
        loop {
            if let Some(status) = self.child.process.try_wait()? {
                self.cleanup()?;
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                self.cleanup()?;
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(crate) fn output(&self) -> io::Result<String> {
        self.child.output()
    }

    pub(crate) fn cleanup_receipt(&self) -> Arc<AtomicBool> {
        self.child.cleanup_receipt()
    }

    pub(crate) fn wait_for_output(&self, needle: &str, lifetime: Duration) -> io::Result<bool> {
        self.child.wait_for_output(needle, lifetime)
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

    pub(crate) fn unregistered_launch_ready(&self) -> io::Result<bool> {
        Ok(self
            .child
            .scratch
            .path()
            .join("pre-registration.ready")
            .exists()
            && self.child.owned_pids()?.is_empty()
            && self.members()?.len() >= 2)
    }

    fn cleanup(&mut self) -> io::Result<()> {
        // Preserve the existing 250ms cooperative and 750ms final cleanup
        // observations. Session quiescence consumes the first budget too.
        let started = *self.cleanup_started.get_or_insert_with(Instant::now);
        let quiesce_by = started + Duration::from_millis(250);
        let final_by = started + Duration::from_secs(1);
        let mut failure = drain(self.session, started + Duration::from_millis(100)).err();
        // Stop the root and acknowledge a stable stopped session before kill.
        // Kernel membership covers groups born before registration; no stopped
        // member can fork another child between this census and termination.
        match quiesce(self.session, quiesce_by) {
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
        if let Err(error) = self.child.process.wait() {
            failure = Some(error);
        }
        // The real root wait closes all in-flight fixture launches. Re-enumerate
        // the inherited session after that join, including reparented processes.
        match drain(self.session, final_by) {
            Ok(true) => {}
            Ok(false) => {
                failure = Some(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "owned session still has unreaped members",
                ));
            }
            Err(error) => failure = Some(error),
        }
        if let Some(error) = failure {
            return Err(error);
        }
        self.child.cleaned.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for SessionFixture {
    fn drop(&mut self) {
        if self.cleanup_started.is_none()
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
        let output = tempfile::NamedTempFile::new()?;
        let process = Command::new("/bin/ps")
            .args(["-axo", "pid=,stat="])
            .stdin(Stdio::null())
            .stdout(output.reopen()?)
            .stderr(output.reopen()?)
            .spawn()?;
        Ok(Self { process, output })
    }

    fn finish(&mut self, deadline: Instant) -> io::Result<String> {
        loop {
            if let Some(status) = self.process.try_wait()? {
                let output = std::fs::read_to_string(self.output.path())?;
                return if status.success() {
                    Ok(output)
                } else {
                    Err(io::Error::other(format!(
                        "owned session inventory failed: {output}"
                    )))
                };
            }
            if Instant::now() >= deadline {
                // SIGKILL also terminates SIGSTOPped inventory. Reap our actual
                // child before returning the deadline failure, including unwind.
                self.reap()?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "owned session inventory timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
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

fn process_ids(deadline: Instant) -> io::Result<Vec<(Pid, bool)>> {
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "session inventory budget exhausted",
        ));
    }
    let invocation_by = deadline.min(Instant::now() + Duration::from_millis(50));
    let output = Inventory::spawn()?.finish(invocation_by)?;
    output
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
        .collect()
}

// Linux permits SID/PGID zero for foreign kernel tasks. rustix's typed getsid
// assumes strictly positive success, so never call it on an all-host Linux PID.
// Read the kernel identity as raw integers and validate before constructing Pid.
#[cfg(target_os = "linux")]
fn identity(pid: Pid) -> io::Result<Option<(Pid, Pid)>> {
    match std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_pid())) {
        Ok(stat) => parse_linux_identity(&stat),
        Err(error)
            if matches!(
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
    members_before(session, Instant::now() + Duration::from_millis(250))
}

fn members_before(session: Pid, deadline: Instant) -> io::Result<Vec<Member>> {
    let mut members = Vec::new();
    for (pid, quiet) in process_ids(deadline)? {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "session identity census budget exhausted",
            ));
        }
        if let Some((owner, group)) = identity(pid)?
            && owner == session
        {
            members.push(Member { pid, group, quiet });
        }
    }
    Ok(members)
}

fn drain(session: Pid, deadline: Instant) -> io::Result<bool> {
    let mut failure = None;
    loop {
        // Do not start a new inventory with only the tail of the observation
        // left: that would mislabel ordinary phase expiry as a stalled child.
        // Every started inventory still owns its full bounded slice, counted
        // inside (never added to) this absolute deadline.
        if Instant::now() + Duration::from_millis(50) > deadline {
            return failure.map_or(Ok(false), Err);
        }
        match members_before(session, deadline) {
            Ok(members) => {
                let members: Vec<_> = members
                    .into_iter()
                    .filter(|member| member.pid != session)
                    .collect();
                if members.is_empty() {
                    return failure.map_or(Ok(true), Err);
                }
                for member in members {
                    kill_member(session, member);
                }
            }
            // Preserve failure, but use the remaining absolute budget to regain
            // inventory and clean the fixture; a stalled ps never owns the tree.
            Err(error) => failure = Some(error),
        }
        if Instant::now() >= deadline {
            return failure.map_or(Ok(false), Err);
        }
        std::thread::sleep(Duration::from_millis(5));
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

fn quiesce(session: Pid, deadline: Instant) -> io::Result<Vec<Member>> {
    let _ = kill_process(session, Signal::STOP);
    let mut previous = None;
    loop {
        let members = members_before(session, deadline)?;
        let mut ids: Vec<_> = members
            .iter()
            .map(|member| member.pid.as_raw_pid())
            .collect();
        ids.sort_unstable();
        if members.iter().all(|member| member.quiet) && previous.as_ref() == Some(&ids) {
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
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "fixture session did not quiesce",
            ));
        }
        previous = Some(ids);
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn assert_passed(child: &mut SessionFixture) -> io::Result<()> {
    let status = child.finish(Duration::from_secs(5))?;
    let output = child.output()?;
    assert!(
        status.is_some_and(|status| status.success()),
        "session fixture failed or exceeded five seconds: {output}"
    );
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
fn stopped_inventory_is_owned_reaped_on_timeout_and_unwind() -> io::Result<()> {
    for unwind in [false, true] {
        let started = Instant::now();
        let mut inventory = Inventory::spawn()?;
        let pid = Pid::from_child(&inventory.process);
        kill_process(pid, Signal::STOP)?;
        if unwind {
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _owner = inventory;
                panic!("injected inventory assertion unwind");
            }));
            assert!(failure.is_err());
        } else {
            let error = inventory
                .finish(started + Duration::from_millis(50))
                .expect_err("stopped ps must fail finitely");
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        }
        assert_eq!(rustix::process::test_kill_process(pid), Err(Errno::SRCH));
        assert!(started.elapsed() < Duration::from_millis(250));
        eprintln!(
            "FTQ-010 stopped inventory unwind={unwind}: pid={pid:?} actually reaped in {:?}",
            started.elapsed()
        );
    }
    Ok(())
}
