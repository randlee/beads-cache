//! Lifecycle test ownership is the kernel session, established before job launch.
//! Child-written PID receipts are diagnostics, never the cleanup authority.

use std::io;
use std::process::{Command, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rustix::io::Errno;
use rustix::process::{Pid, Signal, getpgid, getsid, kill_process, kill_process_group};

use super::fixture_child::FixtureChild;

#[derive(Debug)]
pub(crate) struct SessionFixture {
    child: FixtureChild,
    session: Pid,
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
        let child = FixtureChild::spawn(test, key, mode)?;
        let session = Pid::from_child(&child.process);
        Ok(Self { child, session })
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
        let quiesce_by = Instant::now() + Duration::from_millis(250);
        let mut failure = match drain(self.session, Duration::from_millis(100)) {
            Err(error) if error.kind() != io::ErrorKind::TimedOut => Some(error),
            _ => None,
        };
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
        if let Err(error) = drain(self.session, Duration::from_millis(750)) {
            failure = Some(error);
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
        if !self.child.cleaned.load(Ordering::SeqCst)
            && let Err(error) = self.cleanup()
        {
            eprintln!("session fixture cleanup failed: {error}");
        }
    }
}

fn process_ids() -> io::Result<Vec<(Pid, bool)>> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,stat="])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "owned session inventory unavailable: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8_lossy(&output.stdout)
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
            let pid = Pid::from_raw(raw)
                .ok_or_else(|| io::Error::other("invalid process inventory PID"))?;
            Ok((pid, state.starts_with(['T', 't', 'Z', 'X'])))
        })
        .collect()
}

pub(crate) fn session_members(session: Pid) -> io::Result<Vec<Member>> {
    let mut members = Vec::new();
    for (pid, quiet) in process_ids()? {
        match getsid(Some(pid)) {
            Ok(owner) if owner == session => match getpgid(Some(pid)) {
                Ok(group) => members.push(Member { pid, group, quiet }),
                Err(Errno::SRCH) => {}
                Err(error) => return Err(error.into()),
            },
            Ok(_) | Err(Errno::SRCH | Errno::PERM | Errno::ACCESS) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(members)
}

fn drain(session: Pid, lifetime: Duration) -> io::Result<()> {
    let deadline = Instant::now() + lifetime;
    loop {
        let members: Vec<_> = session_members(session)?
            .into_iter()
            .filter(|member| member.pid != session)
            .collect();
        if members.is_empty() {
            return Ok(());
        }
        for member in members {
            kill_member(session, member);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "owned session still has unreaped members",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn kill_member(session: Pid, member: Member) {
    if getsid(Some(member.pid)) == Ok(session) {
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
        let members = session_members(session)?;
        let mut ids: Vec<_> = members
            .iter()
            .map(|member| member.pid.as_raw_pid())
            .collect();
        ids.sort_unstable();
        if members.iter().all(|member| member.quiet) && previous.as_ref() == Some(&ids) {
            return Ok(members);
        }
        for member in &members {
            if !member.quiet && getsid(Some(member.pid)) == Ok(session) {
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
