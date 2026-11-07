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
        self.finish_using(lifetime, &mut process_ids)
    }

    fn finish_using(
        &mut self,
        lifetime: Duration,
        inventory: &mut impl FnMut(Instant) -> io::Result<Vec<(Pid, bool)>>,
    ) -> io::Result<Option<ExitStatus>> {
        let deadline = self.child.started + lifetime;
        loop {
            if let Some(status) = self.child.process.try_wait()? {
                self.cleanup_using(inventory)?;
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                self.cleanup_using(inventory)?;
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

    pub(crate) fn owned_pids(&self, deadline: Instant) -> io::Result<Vec<u32>> {
        Ok(self
            .members(deadline)?
            .into_iter()
            .map(|member| member.pid.as_raw_pid().cast_unsigned())
            .collect())
    }

    pub(crate) fn session_id(&self) -> Pid {
        self.session
    }

    pub(crate) fn members(&self, deadline: Instant) -> io::Result<Vec<Member>> {
        Ok(session_members(
            self.session,
            deadline.min(self.child.started + Duration::from_secs(5)),
        )?
        .into_iter()
        .filter(|member| member.pid != self.session)
        .collect())
    }

    pub(crate) fn unregistered_launch_ready(&self, deadline: Instant) -> io::Result<bool> {
        Ok(self
            .child
            .scratch
            .path()
            .join("pre-registration.ready")
            .exists()
            && self.child.owned_pids()?.is_empty()
            && self.members(deadline)?.len() >= 2)
    }

    fn cleanup(&mut self) -> io::Result<()> {
        self.cleanup_using(&mut process_ids)
    }

    fn cleanup_using(
        &mut self,
        inventory: &mut impl FnMut(Instant) -> io::Result<Vec<(Pid, bool)>>,
    ) -> io::Result<()> {
        if self.cleanup_started.is_some() {
            return if self.child.cleaned.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(io::Error::other("session cleanup was already attempted"))
            };
        }
        // Preserve the existing 250ms cooperative and 750ms final cleanup
        // observations. Session quiescence consumes the first budget too.
        let started = *self.cleanup_started.get_or_insert_with(Instant::now);
        let quiesce_by = started + Duration::from_millis(250);
        let final_by = started + Duration::from_secs(1);
        let mut calls = 0;
        let early_failure = drain(
            self.session,
            started + Duration::from_millis(100),
            false,
            &mut |deadline| {
                stage_inventory(
                    self.session,
                    "initial-drain",
                    started,
                    &mut calls,
                    deadline,
                    inventory,
                )
            },
        )
        .err();
        let mut failure = None;
        // Stop the root and acknowledge a stable stopped session before kill.
        // Kernel membership covers groups born before registration; no stopped
        // member can fork another child between this census and termination.
        match quiesce(self.session, quiesce_by, &mut |deadline| {
            stage_inventory(
                self.session,
                "quiescence",
                started,
                &mut calls,
                deadline,
                inventory,
            )
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
        eprintln!(
            "RQA10_ROOT session={} elapsed={:?} wait={root_wait:?} remaining_final={:?}",
            self.session.as_raw_pid(),
            started.elapsed(),
            final_by.saturating_duration_since(Instant::now())
        );
        if let Err(error) = root_wait {
            failure = Some(error);
        }
        // The real root wait closes all in-flight fixture launches. Re-enumerate
        // the inherited session after that join, including reparented processes.
        match drain(self.session, final_by, true, &mut |deadline| {
            stage_inventory(
                self.session,
                "final-drain",
                started,
                &mut calls,
                deadline,
                inventory,
            )
        }) {
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
            eprintln!(
                "RQA10_CLEANUP session={} calls={calls} elapsed={:?} cleaned=false error={error}",
                self.session.as_raw_pid(),
                started.elapsed()
            );
            return Err(error);
        }
        if let Some(error) = early_failure {
            // This is recovery evidence, not an empty/error census: quiescence,
            // the actual root wait and a fresh full-session drain all succeeded
            // under the original deadlines. The fixture's own status is intact.
            eprintln!("session cleanup recovered preliminary inventory: {error}");
        }
        self.child.cleaned.store(true, Ordering::SeqCst);
        eprintln!(
            "RQA10_CLEANUP session={} calls={calls} elapsed={:?} cleaned=true final_complete_census=true",
            self.session.as_raw_pid(),
            started.elapsed()
        );
        Ok(())
    }
}

fn stage_inventory(
    session: Pid,
    stage: &str,
    origin: Instant,
    calls: &mut usize,
    deadline: Instant,
    inventory: &mut impl FnMut(Instant) -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<(Pid, bool)>> {
    *calls += 1;
    let thread = std::thread::current().id();
    let entered = Instant::now();
    eprintln!(
        "RQA10_SCAN_BEGIN thread={thread:?} session={} stage={stage} call={calls} owner_elapsed={:?} deadline_offset={:?} remaining={:?}",
        session.as_raw_pid(),
        entered.duration_since(origin),
        deadline.saturating_duration_since(origin),
        deadline.saturating_duration_since(entered)
    );
    let result = inventory(deadline);
    eprintln!(
        "RQA10_SCAN_END thread={thread:?} session={} stage={stage} call={calls} elapsed={:?} remaining={:?} result={:?}",
        session.as_raw_pid(),
        entered.elapsed(),
        deadline.saturating_duration_since(Instant::now()),
        result.as_ref().map(Vec::len)
    );
    result
}

fn audit_members_using(
    session: Pid,
    deadline: Instant,
    inventory: &mut impl FnMut(Instant) -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<Member>> {
    let origin = Instant::now();
    let mut calls = 0;
    // This is independent verification, not a second cleanup attempt. The
    // supplied audit deadline is retained exactly, including census/parsing.
    let result = members_before(session, deadline, &mut |deadline| {
        stage_inventory(
            session,
            "post-cleanup-audit",
            origin,
            &mut calls,
            deadline,
            inventory,
        )
    });
    eprintln!(
        "RQA10_AUDIT session={} elapsed={:?} remaining={:?} complete_members={:?}",
        session.as_raw_pid(),
        origin.elapsed(),
        deadline.saturating_duration_since(Instant::now()),
        result.as_ref().map(Vec::len)
    );
    result
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
    started: Instant,
}

impl Inventory {
    fn spawn() -> io::Result<Self> {
        Self::spawn_command(Command::new("/bin/ps").args(["-axo", "pid=,stat="]))
    }

    fn spawn_command(command: &mut Command) -> io::Result<Self> {
        let thread = std::thread::current().id();
        let output = tempfile::NamedTempFile::new()?;
        let started = Instant::now();
        let process = command
            .stdin(Stdio::null())
            .stdout(output.reopen()?)
            .stderr(output.reopen()?)
            .spawn()?;
        eprintln!(
            "RQA10_COMMAND_SPAWN thread={thread:?} pid={} spawn_elapsed={:?}",
            process.id(),
            started.elapsed()
        );
        Ok(Self {
            process,
            output,
            started,
        })
    }

    fn finish(&mut self, deadline: Instant) -> io::Result<String> {
        let thread = std::thread::current().id();
        loop {
            if let Some(status) = self.process.try_wait()? {
                eprintln!(
                    "RQA10_COMMAND_EXIT thread={thread:?} pid={} command_elapsed={:?} actual_wait=true status={status} remaining={:?}",
                    self.process.id(),
                    self.started.elapsed(),
                    deadline.saturating_duration_since(Instant::now())
                );
                let output = std::fs::read_to_string(self.output.path())?;
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "session inventory output budget exhausted",
                    ));
                }
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
                eprintln!(
                    "RQA10_COMMAND_TIMEOUT thread={thread:?} pid={} command_elapsed={:?} actual_kill_wait=true remaining={:?}",
                    self.process.id(),
                    self.started.elapsed(),
                    deadline.saturating_duration_since(Instant::now())
                );
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
    process_ids_using(deadline, Inventory::spawn)
}

fn process_ids_using(
    deadline: Instant,
    spawn: impl FnOnce() -> io::Result<Inventory>,
) -> io::Result<Vec<(Pid, bool)>> {
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "session inventory budget exhausted",
        ));
    }
    // Inventory consumes the caller's remaining absolute budget. A separately
    // scheduled host-wide ps has no independent healthy-speed requirement.
    let output = spawn()?.finish(deadline)?;
    let pids: Vec<_> = output
        .lines()
        .map(|line| {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "session inventory parsing budget exhausted",
                ));
            }
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

pub(crate) fn session_members(session: Pid, deadline: Instant) -> io::Result<Vec<Member>> {
    members_before(session, deadline, &mut process_ids)
}

fn members_before(
    session: Pid,
    deadline: Instant,
    inventory: &mut impl FnMut(Instant) -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<Member>> {
    let mut members = Vec::new();
    for (pid, quiet) in inventory(deadline)? {
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

fn drain(
    session: Pid,
    deadline: Instant,
    root_joined: bool,
    inventory: &mut impl FnMut(Instant) -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<bool> {
    let mut failure = None;
    loop {
        if Instant::now() >= deadline {
            return failure.map_or(Ok(false), Err);
        }
        match members_before(session, deadline, inventory) {
            Ok(members) => {
                let members: Vec<_> = members
                    .into_iter()
                    .filter(|member| root_joined || member.pid != session)
                    .collect();
                if members.is_empty() {
                    if let Some(error) = failure {
                        eprintln!("session drain recovered inventory before deadline: {error}");
                    }
                    return Ok(true);
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

fn quiesce(
    session: Pid,
    deadline: Instant,
    inventory: &mut impl FnMut(Instant) -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<Member>> {
    let _ = kill_process(session, Signal::STOP);
    let mut previous = None;
    let mut failure = None;
    loop {
        if Instant::now() >= deadline {
            return Err(failure.unwrap_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "fixture session did not quiesce")
            }));
        }
        let members = match members_before(session, deadline, inventory) {
            Ok(members) => members,
            Err(error) => {
                failure = Some(error);
                // An incomplete scan cannot be one of the two stable censuses.
                previous = None;
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
        };
        let mut ids: Vec<_> = members
            .iter()
            .map(|member| member.pid.as_raw_pid())
            .collect();
        ids.sort_unstable();
        if members.iter().all(|member| member.quiet) && previous.as_ref() == Some(&ids) {
            if let Some(error) = failure {
                eprintln!("session quiescence recovered inventory before deadline: {error}");
            }
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
        let mut inventory = stopped_inventory(started + Duration::from_millis(250))?;
        let pid = Pid::from_child(&inventory.process);
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

/// Self-stop before exec, then acknowledge the actual kernel stop under the
/// same caller budget. Unlike racing SIGSTOP with a fast ps, this always owns a
/// noncooperative command. There is no shell child or asynchronous reader.
fn stopped_inventory(deadline: Instant) -> io::Result<Inventory> {
    use rustix::process::{WaitOptions, waitpid};

    let inventory = Inventory::spawn_command(
        Command::new("/bin/sh").args(["-c", "kill -STOP $$; exec /bin/ps -axo pid=,stat="]),
    )?;
    let pid = Pid::from_child(&inventory.process);
    loop {
        if let Some((_, status)) = waitpid(Some(pid), WaitOptions::UNTRACED | WaitOptions::NOHANG)?
        {
            if status.stopped() {
                return Ok(inventory);
            }
            return Err(io::Error::other(
                "inventory exited before its injected stop",
            ));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "inventory did not acknowledge its injected stop",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn inventory_delay_uses_the_existing_preflight_budget_and_reaps_its_command() -> io::Result<()> {
    const MODE: &str = "BD_CACHE_DELAYED_INVENTORY_FIXTURE";
    const TEST: &str =
        "session_fixture::inventory_delay_uses_the_existing_preflight_budget_and_reaps_its_command";
    if std::env::var_os(MODE).is_none() {
        // The existing owner includes spawn and all readiness preparation, not
        // just the later caller qualification. Its five seconds never restart.
        let mut child = SessionFixture::spawn(TEST, MODE, "ready")?;
        assert_passed(&mut child)?;
        // Report the real hold/census proof only after owned root cleanup.
        eprintln!("{}", child.output()?);
        return Ok(());
    }
    rustix::process::setsid()?;
    // Acknowledge real readiness under the enclosing owner before supplying
    // the caller's one deadline. No previously supplied deadline is renewed.
    let inventory = stopped_inventory(Instant::now() + Duration::from_secs(5))?;
    let started = Instant::now();
    let deadline = started + Duration::from_millis(250);
    let mut delayed = None;
    let pids = process_ids_using(deadline, || {
        let child = Pid::from_child(&inventory.process);
        // Keep this real delay independent of transient cleanup recovery: it
        // exceeds the old 50ms cap within the existing 250ms caller budget.
        let delay_started = Instant::now();
        std::thread::sleep(Duration::from_millis(80));
        let delay_elapsed = delay_started.elapsed();
        kill_process(child, Signal::CONT)?;
        delayed = Some((child, delay_elapsed));
        Ok(inventory)
    })
    .inspect_err(|error| {
        eprintln!("FTQ-011 ready inventory qualification failed: delayed={delayed:?}; {error}");
    })?;
    assert!(
        pids.iter()
            .any(|(pid, _)| *pid == rustix::process::getpid())
    );
    let (pid, delay_elapsed) = delayed.expect("stopped inventory resumed");
    assert!(delay_elapsed >= Duration::from_millis(80));
    assert!(started.elapsed() < Duration::from_millis(250));
    assert_eq!(rustix::process::test_kill_process(pid), Err(Errno::SRCH));
    eprintln!(
        "FTQ-011 delayed owned inventory completed/reaped in {:?}; pid={pid:?} actual_stop_ack=true requested_delay=80ms actual_delay={delay_elapsed:?} cont_returned_ok=true actual_wait_reaped=true",
        started.elapsed()
    );
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
    assert!(child.wait_for_output(
        "inventory fixture session established",
        Duration::from_secs(3)
    )?);
    let session = child.session;
    let mut scans = 0;
    // Positive transient recovery requires fresh real censuses under the
    // original cleanup deadlines. The independent inventory-delay regression
    // above owns the deliberate STOP/80ms/CONT input; combining it here added a
    // second timing assumption inside the two-stable-census quiescence window.
    let finished = child.finish_using(Duration::from_secs(5), &mut |deadline| {
        scans += 1;
        if scans == 1 {
            Err(io::Error::other("injected transient inventory failure"))
        } else {
            process_ids(deadline)
        }
    });
    eprintln!(
        "RQA10_RECOVERY_RESULT session={} scans={scans} finish={finished:?} root_status={:?} cleaned={}",
        session.as_raw_pid(),
        child.child.process.try_wait()?,
        child.child.cleaned.load(Ordering::SeqCst)
    );
    let status = finished?;
    assert!(status.is_some_and(|status| !status.success()));
    assert!(
        child
            .output()?
            .contains("original fixture failure must remain a failure")
    );
    assert!(child.child.cleaned.load(Ordering::SeqCst));
    assert!(
        scans >= 5,
        "recovery must include fresh stable and final censuses"
    );
    assert!(
        audit_members_using(
            session,
            Instant::now() + Duration::from_millis(250),
            &mut process_ids
        )?
        .is_empty()
    );
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
fn incomplete_inventory_never_marks_cleanup_success_or_restarts_its_budget() -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "wait")?;
    assert!(child.wait_for_output(
        "inventory fixture session established",
        Duration::from_secs(3)
    )?);
    let session = child.session;
    let receipt = child.cleanup_receipt();
    let mut inventory_pids = Vec::new();
    let started = Instant::now();
    let error = child
        .cleanup_using(&mut |deadline| {
            process_ids_using(deadline, || {
                let inventory = stopped_inventory(deadline)?;
                inventory_pids.push(Pid::from_child(&inventory.process));
                Ok(inventory)
            })
        })
        .expect_err("incomplete required censuses must not prove successful cleanup");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(!receipt.load(Ordering::SeqCst));
    assert!(started.elapsed() < Duration::from_secs(2));
    let attempted = child.cleanup_started;
    assert!(
        child.cleanup().is_err(),
        "a second cleanup must not gain a fresh budget"
    );
    assert_eq!(child.cleanup_started, attempted);
    drop(child);
    assert!(!receipt.load(Ordering::SeqCst));
    assert_eq!(
        rustix::process::test_kill_process(session),
        Err(Errno::SRCH)
    );
    assert_eq!(inventory_pids.len(), 3, "one owned scan per existing phase");
    for pid in &inventory_pids {
        assert_eq!(rustix::process::test_kill_process(*pid), Err(Errno::SRCH));
    }
    // This separate audit is evidence only; it cannot retroactively set receipt.
    assert!(session_members(session, Instant::now() + Duration::from_millis(250))?.is_empty());
    assert!(!receipt.load(Ordering::SeqCst));
    eprintln!(
        "FTQ-011 noncooperative census children {inventory_pids:?} reaped; failed cleanup retained"
    );
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
    assert!(child.wait_for_output(
        "inventory fixture session established",
        Duration::from_secs(3)
    )?);
    let session = child.session;
    let mut scans = 0;
    let mut held_pid = None;
    let result = child.finish_using(Duration::from_secs(5), &mut |deadline| {
        scans += 1;
        if !post_audit && scans == 2 {
            process_ids_using(deadline, || {
                let inventory = stopped_inventory(deadline)?;
                held_pid = Some(Pid::from_child(&inventory.process));
                Ok(inventory)
            })
        } else {
            process_ids(deadline)
        }
    });
    if post_audit {
        assert!(result?.is_some_and(|status| !status.success()));
        assert!(child.child.cleaned.load(Ordering::SeqCst));
        let error = audit_members_using(
            session,
            Instant::now() + Duration::from_millis(250),
            &mut |deadline| {
                process_ids_using(deadline, || {
                    let inventory = stopped_inventory(deadline)?;
                    held_pid = Some(Pid::from_child(&inventory.process));
                    Ok(inventory)
                })
            },
        )
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
                .ok_or_else(|| io::Error::other("held quiescence unexpectedly succeeded"))?
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(!child.child.cleaned.load(Ordering::SeqCst));
        let original_attempt = child.cleanup_started;
        assert!(child.cleanup().is_err());
        assert_eq!(child.cleanup_started, original_attempt);
    }
    let root_status = child
        .child
        .process
        .try_wait()?
        .ok_or_else(|| io::Error::other("actual root wait incomplete"))?;
    assert!(
        !root_status.success(),
        "the original child failure must stay failed"
    );
    assert!(
        child
            .output()?
            .contains("original fixture failure must remain a failure")
    );
    assert_eq!(
        rustix::process::test_kill_process(session),
        Err(Errno::SRCH)
    );
    assert_eq!(
        rustix::process::test_kill_process(
            held_pid.ok_or_else(|| io::Error::other("owned held inventory absent"))?
        ),
        Err(Errno::SRCH)
    );
    // A fresh independent audit establishes actual emptiness without changing
    // the failed cleanup/audit observation or granting another cleanup attempt.
    assert!(session_members(session, Instant::now() + Duration::from_millis(250))?.is_empty());
    assert_eq!(child.child.cleaned.load(Ordering::SeqCst), post_audit);
    eprintln!(
        "RQA10_CONTROL post_audit={post_audit} actual_root_wait=true held_inventory_reaped=true complete_independent_census=true cleanup_receipt={post_audit} original_fixture_failed=true before_driver_fallback"
    );
    Ok(())
}
