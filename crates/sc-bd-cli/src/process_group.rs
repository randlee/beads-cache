//! Signals to a Unix process group, sent with `kill(2)` itself (RSH-002).
//!
//! The bounded calls kill a timed-out child's whole process group, so a `dolt` that
//! `bd` started dies with it (PE-RSH-001). That kill used to run the `kill` program
//! (`/bin/kill`, else `kill` from `PATH`); in a minimal image the program is missing
//! or cannot be started (no free process slot), and then only the leader died: the
//! grandchild survived and the next call on the project collided with it. These
//! functions make the system call directly, so the kill needs no program, no `PATH`
//! and no new process.
//!
//! They are public so that another crate that leads a process group (a subprocess
//! runner) kills it the same way without `unsafe` of its own.
//! Unix only; on Windows a process tree is a job object or `taskkill /T`.
//!
//! # Which groups may be named
//!
//! `kill(2)` gives a pid of `0` and `-1` special meanings (the caller's own group,
//! every process the caller may signal), so a group id of `0` or `1` (`init`'s) is
//! refused with [`io::ErrorKind::InvalidInput`], as is one that is not a valid
//! `pid_t`. Name only a group whose leader is your own child and has not been reaped:
//! until it is reaped its pid, the group's id, cannot be reused (POSIX XBD 4.14).

use std::io;

/// The group id as the negative `pid_t` that names a group in `kill(2)`.
fn group_target(pgid: u32) -> io::Result<libc::pid_t> {
    match libc::pid_t::try_from(pgid) {
        Ok(id) if id > 1 => Ok(-id),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{pgid} is not a process group this crate may signal"),
        )),
    }
}

/// Sends `SIGKILL` to every process in the process group `pgid`.
///
/// # Errors
///
/// [`io::ErrorKind::InvalidInput`] for a `pgid` of `0` or `1` or one that is not a
/// valid `pid_t` (see the module docs); otherwise the OS error of `kill(2)`, e.g.
/// `ESRCH` when the group has no member.
///
/// # Examples
///
/// ```
/// use sc_bd_cli::process_group;
/// // The caller's own group and init's are never signalled.
/// assert!(process_group::kill(0).is_err());
/// assert!(process_group::kill(1).is_err());
/// ```
pub fn kill(pgid: u32) -> io::Result<()> {
    let target = group_target(pgid)?;
    // SAFETY: `kill` takes two integers and touches no memory of this process;
    // `target` is a negative pid other than -1, so it names exactly the group
    // `pgid`, never "every process" or the caller's own group.
    let rc = unsafe { libc::kill(target, libc::SIGKILL) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Whether the process group `pgid` has a member: `kill(-pgid, 0)`, which delivers
/// no signal and only checks that the group exists.
///
/// A member this process may not signal (`EPERM`) still counts: the group exists.
///
/// # Errors
///
/// As [`kill`], except that `ESRCH` (no member) and `EPERM` are answers, not errors.
pub fn exists(pgid: u32) -> io::Result<bool> {
    let target = group_target(pgid)?;
    // SAFETY: as in `kill`; signal 0 is delivered to no one.
    let rc = unsafe { libc::kill(target, 0) };
    if rc == 0 {
        return Ok(true);
    }
    let e = io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt as _;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn the_special_pids_are_refused() {
        for pgid in [0, 1, u32::MAX] {
            assert_eq!(
                kill(pgid).map_err(|e| e.kind()),
                Err(io::ErrorKind::InvalidInput),
                "{pgid}"
            );
            assert_eq!(
                exists(pgid).map_err(|e| e.kind()),
                Err(io::ErrorKind::InvalidInput),
                "{pgid}"
            );
        }
    }

    /// A group with a member exists until it is killed and the member reaped.
    #[test]
    fn a_killed_group_is_gone_once_reaped() {
        let mut child = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = child.id();
        assert!(exists(pgid).unwrap());
        kill(pgid).unwrap();
        child.wait().unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while exists(pgid).unwrap() {
            assert!(Instant::now() < until, "the group {pgid} is still there");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            kill(pgid).map_err(|e| e.raw_os_error()),
            Err(Some(libc::ESRCH))
        );
    }
}
