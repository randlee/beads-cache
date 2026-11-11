//! Lifecycle test ownership is the kernel session, established before job launch.
//! Child-written PID receipts are diagnostics, never the cleanup authority.

use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::os::unix::process::ExitStatusExt as _;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::io::Errno;
use rustix::process::{
    Pid, Signal, WaitId, WaitIdOptions, kill_process, kill_process_group, waitid,
};
#[cfg(not(target_os = "linux"))]
use rustix::process::{getpgid, getsid};

use super::fixture_child::FixtureChild;

// This is a failure-only hang watchdog, never an operation's expected runtime.
// Every successful observation is a kernel exit/stop or an explicit fixture event.
const WATCHDOG: Duration = Duration::from_secs(120);
const READY_SOCKET: &str = "BD_CACHE_SESSION_READY_SOCKET";

pub(crate) fn notify_marker(marker: &str) -> io::Result<()> {
    if let Some(path) = std::env::var_os(READY_SOCKET) {
        let socket = UnixDatagram::unbound()?;
        socket.set_write_timeout(Some(WATCHDOG))?;
        socket.send_to(marker.as_bytes(), path)?;
    }
    Ok(())
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|budget| !budget.is_zero())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "fixture failure-only hang watchdog expired",
            )
        })
}

/// Observe a child without reaping it. Its identity stays reserved until the
/// owner chooses completion or watchdog cancellation, then physically waits it.
fn observe_child(process: &Child, options: WaitIdOptions) -> io::Result<()> {
    observe_child_with_watchdog(process, options, WATCHDOG)
}

fn observe_child_with_watchdog(
    process: &Child,
    options: WaitIdOptions,
    watchdog: Duration,
) -> io::Result<()> {
    let pid = Pid::from_child(process);
    let (completion, observer) = start_observer(pid, options)?;
    let (observation, expired) = match completion.recv_timeout(watchdog) {
        Ok(result) => (result, false),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // NOWAIT retains the actual child identity during this decision.
            let _ = kill_process(pid, Signal::KILL);
            (collect_killed_observer(&completion, pid), true)
        }
        Err(error) => {
            let _ = observer.join();
            return Err(io::Error::other(error));
        }
    };
    observer
        .join()
        .map_err(|panic| io::Error::other(format!("child observer panicked: {panic:?}")))?;
    let exited = observation?;
    if expired {
        if !exited {
            // STOP may have won the first observation concurrently with the
            // watchdog. The owner's decision still requires a real exit ACK.
            let (completion, observer) = start_observer(pid, WaitIdOptions::EXITED)?;
            let confirmation = collect_killed_observer(&completion, pid);
            observer
                .join()
                .map_err(|panic| io::Error::other(format!("kill observer panicked: {panic:?}")))?;
            confirmation?;
        }
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("owned child {pid:?} failed to acknowledge exit/stop"),
        ));
    }
    Ok(())
}

type Observer = (
    std::sync::mpsc::Receiver<io::Result<bool>>,
    std::thread::JoinHandle<()>,
);

fn start_observer(pid: Pid, options: WaitIdOptions) -> io::Result<Observer> {
    let (finished, completion) = std::sync::mpsc::sync_channel(1);
    let observer = std::thread::Builder::new()
        .name("fixture-child-wait".into())
        .spawn(move || {
            // A kernel notification blocks this thread without consuming a CPU.
            // The actual owner alone reaps the child after receiving this ACK.
            let result = waitid(WaitId::Pid(pid), options | WaitIdOptions::NOWAIT)
                .map_err(io::Error::from)
                .and_then(|status| {
                    let status = status.ok_or_else(|| {
                        io::Error::other("blocking child observer returned no event")
                    })?;
                    let exited = status.exited() || status.killed() || status.dumped();
                    if exited || (options.contains(WaitIdOptions::STOPPED) && status.stopped()) {
                        Ok(exited)
                    } else {
                        Err(io::Error::other(
                            "blocking child observer returned an unrelated event",
                        ))
                    }
                });
            let _ = finished.send(result);
        })?;
    Ok((completion, observer))
}

fn collect_killed_observer(
    completion: &std::sync::mpsc::Receiver<io::Result<bool>>,
    pid: Pid,
) -> io::Result<bool> {
    match completion.recv_timeout(WATCHDOG) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::other(
            "killed-child observer disconnected before acknowledgement",
        )),
        Err(error @ std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // A child that will not acknowledge SIGKILL cannot be safely
            // detached or joined without a bound. Fail this test process
            // decisively rather than leave a blocked observer thread alive.
            eprintln!("FATAL fixture watchdog: child {pid:?} did not acknowledge SIGKILL: {error}");
            std::process::abort();
        }
    }
}

pub(crate) fn wait_for_exit(process: &mut Child) -> io::Result<ExitStatus> {
    if let Some(status) = process.try_wait()? {
        return Ok(status);
    }
    match observe_child(process, WaitIdOptions::EXITED) {
        Ok(()) => process.wait(),
        Err(error) => {
            // A watchdog may have killed and observed this child before
            // reporting failure. Reap only after an actual exit receipt.
            if waitid(
                WaitId::Pid(Pid::from_child(&*process)),
                WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG,
            )
            .is_ok_and(|status| {
                status.is_some_and(|status| status.exited() || status.killed() || status.dumped())
            }) {
                let _ = process.wait();
            }
            Err(error)
        }
    }
}

#[derive(Debug)]
pub(crate) struct SessionFixture {
    child: FixtureChild,
    session: Pid,
    cleanup_attempted: bool,
    events: UnixDatagram,
    _event_root: tempfile::TempDir,
    markers: HashSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupPhase {
    InitialDrain,
    Quiescence,
    FinalDrain,
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
        let event_root = tempfile::Builder::new().prefix("s-").tempdir_in("/tmp")?;
        let event_path = event_root.path().join("ready.sock");
        let events = UnixDatagram::bind(&event_path)?;
        events.set_read_timeout(Some(WATCHDOG))?;
        let mut child =
            FixtureChild::spawn_with_env(test, key, mode, Some((READY_SOCKET, &event_path)))?;
        let session = Pid::from_child(&child.process);
        // Transfer cleanup ownership; a failed session cleanup must not restart
        // the registry supervisor with a fresh budget during field destruction.
        child.cleanup_on_drop = false;
        Ok(Self {
            child,
            session,
            cleanup_attempted: false,
            events,
            _event_root: event_root,
            markers: HashSet::new(),
        })
    }

    pub(crate) fn finish(&mut self) -> io::Result<ExitStatus> {
        let observation = observe_child(&self.child.process, WaitIdOptions::EXITED);
        let cleanup = self.cleanup_using(&mut process_ids);
        match observation {
            Ok(()) => {
                cleanup?;
                self.child.process.wait()
            }
            Err(error) => Err(io::Error::new(
                error.kind(),
                format!("{error}; cleanup={cleanup:?}; output: {}", self.output()?),
            )),
        }
    }

    pub(crate) fn terminate(&mut self) -> io::Result<ExitStatus> {
        let exited = waitid(
            WaitId::Pid(self.session),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG,
        )?
        .is_some_and(|status| status.exited() || status.killed() || status.dumped());
        if exited {
            self.cleanup_using(&mut process_ids)?;
            return Err(io::Error::other(
                "fixture exited before explicit termination",
            ));
        }
        let _ = self.child.process.kill();
        observe_child(&self.child.process, WaitIdOptions::EXITED)?;
        self.cleanup_using(&mut process_ids)?;
        self.child.process.wait()
    }

    pub(crate) fn output(&self) -> io::Result<String> {
        self.child.output()
    }

    pub(crate) fn cleanup_receipt(&self) -> Arc<AtomicBool> {
        self.child.cleanup_receipt()
    }

    pub(crate) fn wait_for_output(&mut self, needle: &str) -> io::Result<bool> {
        self.wait_for_marker(needle, WATCHDOG)
    }

    fn wait_for_marker(&mut self, needle: &str, watchdog: Duration) -> io::Result<bool> {
        if self.markers.contains(needle) {
            return Ok(true);
        }
        let deadline = Instant::now() + watchdog;
        loop {
            let budget = remaining(deadline).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "fixture marker {needle:?} absent: {error}; {}",
                        self.output().unwrap_or_default()
                    ),
                )
            })?;
            self.events.set_read_timeout(Some(budget))?;
            let mut marker = [0; 4096];
            let length = self.events.recv(&mut marker).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "fixture marker {needle:?} absent: {error}; {}",
                        self.output().unwrap_or_default()
                    ),
                )
            })?;
            let marker = String::from_utf8(
                marker
                    .get(..length)
                    .ok_or_else(|| io::Error::other("invalid readiness event length"))?
                    .to_vec(),
            )
            .map_err(io::Error::other)?;
            self.markers.insert(marker);
            if self.markers.contains(needle) {
                return Ok(true);
            }
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
        self.wait_for_output("pre-registration-ready")?;
        if !self.child.owned_pids()?.is_empty() {
            return Err(io::Error::other(
                "preregistration marker arrived after PID registration",
            ));
        }
        let members = self.members()?;
        if members.len() < 2 {
            return Err(io::Error::other(
                "preregistration handshake lacks shell and blocked child",
            ));
        }
        Ok(members)
    }

    fn cleanup(&mut self) -> io::Result<()> {
        self.cleanup_using(&mut process_ids)
    }

    fn cleanup_using(
        &mut self,
        inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
    ) -> io::Result<()> {
        self.cleanup_by_phase(&mut |_| inventory())
    }

    fn cleanup_with_explicit_recovery(
        &mut self,
        inventory: &mut impl FnMut(CleanupPhase) -> io::Result<Vec<(Pid, bool)>>,
    ) -> io::Result<()> {
        self.cleanup_by_phase(&mut |phase| match inventory(phase) {
            Ok(census) => Ok(census),
            Err(error) => {
                eprintln!("caller-selected inventory recovery phase={phase:?}: {error}");
                inventory(phase)
            }
        })
    }

    fn cleanup_by_phase(
        &mut self,
        inventory: &mut impl FnMut(CleanupPhase) -> io::Result<Vec<(Pid, bool)>>,
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
        // Every required phase is attempted once. An error fails the original
        // outcome; continuing distinct teardown phases is not a census retry.
        let stopped = stop_root(&mut self.child.process, self.session);
        if stopped.is_err() {
            let _ = self.child.process.kill();
        }
        let mut failure = stopped.err();
        let initial = drain_matching(self.session, false, &mut || {
            calls += 1;
            inventory(CleanupPhase::InitialDrain)
        });
        if let Err(error) = initial {
            failure.get_or_insert(error);
        }
        // Stop the root and acknowledge a stable stopped session before kill.
        // Kernel membership covers groups born before registration; no stopped
        // member can fork another child between this census and termination.
        let quiesced = quiesce(&mut self.child.process, self.session, &mut || {
            calls += 1;
            inventory(CleanupPhase::Quiescence)
        });
        match quiesced {
            Ok(members) => {
                for member in members {
                    if member.pid != self.session {
                        kill_member(self.session, member);
                    }
                }
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
        let _ = self.child.process.kill();
        // Observe actual root exit without reaping: the reserved PID/SID
        // remains our authority through the last descendant inventory.
        if let Err(error) = observe_child(&self.child.process, WaitIdOptions::EXITED) {
            // Some tests have already physically waited the root, caching its
            // status in Child. No other code may reap this direct child.
            if error.raw_os_error() != Some(Errno::CHILD.raw_os_error())
                || self.child.process.try_wait()?.is_none()
            {
                failure.get_or_insert(error);
            }
        }
        // Exited root cannot launch further children. Re-enumerate its owned
        // session while its identity remains anchored, including separate PGIDs.
        let drain_result = drain(self.session, &mut || {
            calls += 1;
            inventory(CleanupPhase::FinalDrain)
        });
        match drain_result {
            Ok(true) => {}
            Ok(false) => {
                failure.get_or_insert_with(|| {
                    io::Error::other("owned session still has unreaped members")
                });
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
        if failure.is_some() {
            // Physical teardown is independent of the failed phase receipts.
            // It cannot turn a failed required census into successful cleanup.
            if let Err(error) = drain(self.session, &mut process_ids) {
                eprintln!("additional best-effort teardown failure: {error}");
            }
        }
        // Reap the actual direct child last, after descendant work is gone.
        if let Err(error) = wait_for_exit(&mut self.child.process) {
            failure.get_or_insert(error);
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
    release: Option<ChildStdin>,
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
        Ok(Self {
            process,
            output,
            release: None,
        })
    }

    fn finish_with_watchdog(&mut self, watchdog: Duration) -> io::Result<String> {
        let observed = observe_child_with_watchdog(&self.process, WaitIdOptions::EXITED, watchdog);
        let status = wait_for_exit(&mut self.process);
        observed?;
        let status = status?;
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
        wait_for_exit(&mut self.process)?;
        Ok(())
    }

    fn reap(&mut self) -> io::Result<()> {
        if self.process.try_wait()?.is_none() {
            self.process.kill()?;
        }
        wait_for_exit(&mut self.process)?;
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
    process_ids_with_watchdog(WATCHDOG, spawn)
}

fn process_ids_with_watchdog(
    watchdog: Duration,
    spawn: impl FnOnce() -> io::Result<Inventory>,
) -> io::Result<Vec<(Pid, bool)>> {
    // Inventory completion is the handshake: the caller waits for the actual
    // scan process and owns/reaps it before parsing its complete output.
    let output = spawn()?.finish_with_watchdog(watchdog)?;
    let pids: Vec<_> = output
        .lines()
        .filter(|line| {
            !line
                .split_whitespace()
                .nth(1)
                .is_some_and(|state| state.starts_with(['Z', 'X']))
        })
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
            Ok((pid, state.starts_with(['T', 't'])))
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
    drain_matching(session, true, inventory)
}

fn drain_matching(
    session: Pid,
    include_root: bool,
    inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<bool> {
    let deadline = Instant::now() + WATCHDOG;
    loop {
        remaining(deadline)?;
        let members: Vec<_> = members_before(session, inventory)?
            .into_iter()
            .filter(|member| include_root || member.pid != session)
            .collect();
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

fn stop_root(process: &mut Child, session: Pid) -> io::Result<()> {
    let exited = match waitid(
        WaitId::Pid(session),
        WaitIdOptions::EXITED
            | WaitIdOptions::STOPPED
            | WaitIdOptions::NOWAIT
            | WaitIdOptions::NOHANG,
    ) {
        Ok(status) => status.is_some_and(|status| {
            status.exited() || status.killed() || status.dumped() || status.stopped()
        }),
        Err(Errno::CHILD) => process.try_wait()?.is_some(),
        Err(error) => return Err(error.into()),
    };
    if !exited {
        let _ = kill_process(session, Signal::STOP);
        observe_child(process, WaitIdOptions::STOPPED | WaitIdOptions::EXITED)?;
    }
    Ok(())
}

fn quiesce(
    process: &mut Child,
    session: Pid,
    inventory: &mut impl FnMut() -> io::Result<Vec<(Pid, bool)>>,
) -> io::Result<Vec<Member>> {
    stop_root(process, session)?;
    let deadline = Instant::now() + WATCHDOG;
    let mut previous_quiet = None;
    loop {
        remaining(deadline)?;
        let members = members_before(session, inventory)?;
        if members.iter().all(|member| member.quiet) {
            // A complete matching census after the known parents stopped
            // discovers any child forked while the preceding census ran.
            let mut identities: Vec<_> = members
                .iter()
                .map(|member| (member.pid, member.group))
                .collect();
            identities.sort_unstable_by_key(|(pid, group)| (pid.as_raw_pid(), group.as_raw_pid()));
            if previous_quiet.as_ref() == Some(&identities) {
                return Ok(members);
            }
            previous_quiet = Some(identities);
            continue;
        }
        previous_quiet = None;
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
fn stopped_inventory_is_owned_reaped_on_timeout_and_unwind() -> io::Result<()> {
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
            let error = inventory
                .finish_with_watchdog(Duration::ZERO)
                .expect_err("held inventory must fail on injected watchdog expiry");
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        }
        assert_eq!(rustix::process::test_kill_process(pid), Err(Errno::SRCH));
    }
    Ok(())
}

/// The child writes its ACK before reading the release input. Buffered input
/// makes parent-before-child scheduling safe: there is no STOP/CONT window.
fn stopped_inventory() -> io::Result<Inventory> {
    held_inventory_with_script(
        "printf ready >&2; IFS= read -r release; exec /bin/ps -axo pid=,stat=",
    )
}

fn held_inventory_with_script(script: &str) -> io::Result<Inventory> {
    let (mut acknowledgement, child_ack) = UnixStream::pair()?;
    acknowledgement.set_read_timeout(Some(WATCHDOG))?;
    let output = tempfile::NamedTempFile::new()?;
    let mut command = Command::new("/bin/sh");
    let mut process = command
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(output.reopen()?)
        .stderr(Stdio::from(std::os::fd::OwnedFd::from(child_ack)))
        .spawn()?;
    drop(command);
    let release = process.stdin.take();
    let mut inventory = Inventory {
        process,
        output,
        release,
    };
    let mut ready = [0; 5];
    let acknowledgement_result = acknowledgement.read_exact(&mut ready).and_then(|()| {
        if &ready == b"ready" {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "inventory ACK mismatch",
            ))
        }
    });
    if let Err(error) = acknowledgement_result {
        let cleanup = inventory.cancel();
        let status = inventory.process.try_wait();
        let exit_code = status
            .as_ref()
            .ok()
            .and_then(|status| status.as_ref())
            .and_then(std::process::ExitStatus::code);
        let output = std::fs::read_to_string(inventory.output.path());
        return Err(io::Error::new(
            error.kind(),
            format!(
                "{error}; ACK={ready:?}; child_status={status:?}; exit_code={exit_code:?}; output={output:?}; cleanup={cleanup:?}"
            ),
        ));
    }
    Ok(inventory)
}

#[test]
fn inventory_delay_uses_the_existing_preflight_budget_and_reaps_its_command() -> io::Result<()> {
    const MODE: &str = "BD_CACHE_DELAYED_INVENTORY_FIXTURE";
    const TEST: &str =
        "session_fixture::inventory_delay_uses_the_existing_preflight_budget_and_reaps_its_command";
    if std::env::var_os(MODE).is_none() {
        let mut child = SessionFixture::spawn(TEST, MODE, "ready")?;
        assert_passed(&mut child)?;
        eprintln!("{}", child.output()?);
        return Ok(());
    }
    rustix::process::setsid()?;
    let mut inventory = stopped_inventory()?;
    let pid = Pid::from_child(&inventory.process);
    let pids = process_ids_using(|| {
        let mut release = inventory
            .release
            .take()
            .ok_or_else(|| io::Error::other("held inventory release missing"))?;
        release.write_all(b"scan\n")?;
        drop(release);
        Ok(inventory)
    })?;
    assert!(
        pids.iter()
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
        notify_marker("inventory fixture session established")?;
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
    observe_child(&child.child.process, WaitIdOptions::EXITED)?;
    let mut phases = Vec::new();
    let mut inject_failure = true;
    child.cleanup_with_explicit_recovery(&mut |phase| {
        phases.push(phase);
        if std::mem::take(&mut inject_failure) {
            Err(io::Error::other("injected transient inventory failure"))
        } else {
            process_ids()
        }
    })?;
    assert!(
        !child.child.process.wait()?.success(),
        "original fixture failure retained"
    );
    assert!(
        child
            .output()?
            .contains("original fixture failure must remain a failure")
    );
    assert!(child.child.cleaned.load(Ordering::SeqCst));
    assert!(
        phases
            .iter()
            .filter(|phase| **phase == CleanupPhase::InitialDrain)
            .count()
            >= 2,
        "explicit recovery must obtain a fresh initial census"
    );
    assert!(
        phases
            .iter()
            .filter(|phase| **phase == CleanupPhase::Quiescence)
            .count()
            >= 2,
        "recovery must retain two fresh matching quiet censuses"
    );
    assert!(
        phases.contains(&CleanupPhase::FinalDrain),
        "recovery must include a final full census"
    );
    assert!(audit_members_using(session, &mut process_ids)?.is_empty());
    assert_eq!(
        rustix::process::test_kill_process(session),
        Err(Errno::SRCH)
    );
    Ok(())
}

#[test]
fn first_inventory_failure_is_not_retried_or_reported_as_success() -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "fail")?;
    child.wait_for_output("inventory fixture session established")?;
    observe_child(&child.child.process, WaitIdOptions::EXITED)?;
    let mut phases = Vec::new();
    let error = child
        .cleanup_by_phase(&mut |phase| {
            phases.push(phase);
            Err(io::Error::other("injected first census failure"))
        })
        .expect_err("default cleanup must preserve first census failure");
    assert!(error.to_string().contains("injected first census failure"));
    assert_eq!(
        phases,
        [
            CleanupPhase::InitialDrain,
            CleanupPhase::Quiescence,
            CleanupPhase::FinalDrain
        ],
        "distinct teardown phases do not retry a failed census"
    );
    assert!(!child.child.cleaned.load(Ordering::SeqCst));
    assert!(!child.child.process.wait()?.success());
    Ok(())
}

#[test]
fn incomplete_inventory_never_marks_cleanup_success_or_restarts_its_budget() -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "wait")?;
    assert!(child.wait_for_output("inventory fixture session established")?);
    let session = child.session;
    let receipt = child.cleanup_receipt();
    let mut inventories = Vec::new();
    let error = child
        .cleanup_by_phase(&mut |phase| {
            process_ids_with_watchdog(Duration::ZERO, || {
                let inventory = stopped_inventory()?;
                inventories.push((phase, Pid::from_child(&inventory.process)));
                Ok(inventory)
            })
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
        inventories
            .iter()
            .map(|(phase, _)| *phase)
            .collect::<Vec<_>>(),
        [
            CleanupPhase::InitialDrain,
            CleanupPhase::Quiescence,
            CleanupPhase::FinalDrain
        ],
        "one owned census per existing phase"
    );
    assert_eq!(inventories.len(), 3, "all three phase commands were owned");
    for (_, pid) in &inventories {
        assert_eq!(
            rustix::process::test_kill_process(*pid),
            Err(Errno::SRCH),
            "owned census child was not physically reaped"
        );
    }
    // This separate audit is evidence only; it cannot retroactively set receipt.
    assert!(session_members(session)?.is_empty());
    assert!(!receipt.load(Ordering::SeqCst));
    eprintln!("FTQ-011 first incomplete census failed; failed cleanup retained");
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
    let status = wait_for_exit(&mut child.child.process)?;
    assert!(
        !status.success(),
        "the fixture's panic must remain a failure"
    );
    let mut held_pids = Vec::new();
    let result = child.cleanup_using(&mut || {
        if post_audit {
            process_ids()
        } else {
            controlled_incomplete_inventory(&mut held_pids)
        }
    });
    if post_audit {
        result?;
        assert!(child.child.cleaned.load(Ordering::SeqCst));
        let error = audit_members_using(session, &mut || {
            controlled_incomplete_inventory(&mut held_pids)
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
            .child
            .process
            .try_wait()?
            .is_some_and(|status| !status.success()),
        "actual root wait must remain cached and failed after cleanup"
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
    assert!(
        !held_pids.is_empty(),
        "actual held inventory command must be owned"
    );
    for pid in &held_pids {
        assert_eq!(
            rustix::process::test_kill_process(*pid),
            Err(Errno::SRCH),
            "actual held inventory command must be reaped"
        );
    }
    // A fresh independent audit establishes actual emptiness without changing
    // the failed cleanup/audit observation or granting another cleanup attempt.
    assert!(session_members(session)?.is_empty());
    assert_eq!(child.child.cleaned.load(Ordering::SeqCst), post_audit);
    eprintln!(
        "RQA10_CONTROL post_audit={post_audit} actual_root_wait=true complete_independent_census=true cleanup_receipt={post_audit} original_fixture_failed=true"
    );
    Ok(())
}

fn controlled_incomplete_inventory(pids: &mut Vec<Pid>) -> io::Result<Vec<(Pid, bool)>> {
    process_ids_with_watchdog(Duration::ZERO, || {
        let inventory = stopped_inventory()?;
        pids.push(Pid::from_child(&inventory.process));
        Ok(inventory)
    })
}

#[test]
fn held_inventory_watchdog_kills_and_physically_reaps_without_pid_reuse() -> io::Result<()> {
    let mut inventory = stopped_inventory()?;
    // The acknowledged input-held child cannot exit before this injected watchdog
    // decision. Zero is an explicit failure input, not a scheduling threshold.
    let error =
        observe_child_with_watchdog(&inventory.process, WaitIdOptions::EXITED, Duration::ZERO)
            .expect_err("held child cannot finish before explicit cancellation");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(inventory.process.wait()?.signal(), Some(9));
    assert!(
        inventory.process.try_wait()?.is_some(),
        "cached physical wait receipt"
    );
    Ok(())
}

#[test]
fn cleanup_observes_an_exited_root_without_consuming_its_wait_status() -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "fail")?;
    child.wait_for_output("inventory fixture session established")?;
    observe_child(&child.child.process, WaitIdOptions::EXITED)?;
    child.cleanup()?;
    assert!(
        !child.child.process.wait()?.success(),
        "original panic status retained"
    );
    assert!(child.child.cleaned.load(Ordering::SeqCst));
    assert!(
        child
            .output()?
            .contains("original fixture failure must remain a failure")
    );
    Ok(())
}

#[test]
fn readiness_retains_markers_received_before_the_requested_marker() -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "wait")?;
    child.wait_for_output("inventory fixture session established")?;
    let producer = UnixDatagram::unbound()?;
    let address = child.events.local_addr()?;
    let path = address
        .as_pathname()
        .ok_or_else(|| io::Error::other("readiness address has no path"))?;
    producer.send_to(b"first", path)?;
    producer.send_to(b"second", path)?;
    assert!(child.wait_for_output("second")?);
    assert!(
        child.wait_for_marker("first", Duration::ZERO)?,
        "already received marker must not wait again"
    );
    assert_eq!(child.terminate()?.signal(), Some(9));
    Ok(())
}

#[test]
fn missing_readiness_marker_fails_with_output_and_keeps_cleanup_ownership() -> io::Result<()> {
    let mut child = SessionFixture::spawn(INVENTORY_FIXTURE_TEST, INVENTORY_FIXTURE_MODE, "wait")?;
    child.wait_for_output("inventory fixture session established")?;
    // Inject watchdog expiry while the actual fixture is causally parked.
    let error = child
        .wait_for_marker("never sent", Duration::ZERO)
        .expect_err("missing marker cannot be successful readiness");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(
        error
            .to_string()
            .contains("inventory fixture session established")
    );
    let receipt = child.cleanup_receipt();
    drop(child);
    assert!(
        receipt.load(Ordering::SeqCst),
        "readiness failure retains owned teardown"
    );
    Ok(())
}

#[test]
fn inventory_exit_before_ack_reports_status_and_output_without_losing_ownership() {
    let error = held_inventory_with_script("printf inventory-before-ack; exit 23")
        .expect_err("child exit before ACK must remain a protocol failure");
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    let message = error.to_string();
    assert!(message.contains("inventory-before-ack"), "{message}");
    assert!(message.contains("child_status=Ok(Some("), "{message}");
    assert!(
        message.contains("23") || message.contains("5888"),
        "{message}"
    );
    assert!(message.contains("cleanup=Ok(())"), "{message}");
}
