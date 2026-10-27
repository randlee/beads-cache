//! The process-wide shutdown request every bounded call observes (RSH-001).
//!
//! A bounded `bd` child leads its own process group ([`bounded`](crate::bounded)), so
//! a `SIGTERM`, `SIGINT` or `SIGHUP` sent to the host does not reach it: a host that
//! dies of the signal leaves the `bd` (and a `dolt` it started) running, with no
//! project lock held. A host that handles those signals calls [`request`] from its
//! handler instead; from then on every bounded call in the process stops:
//!
//! - a call waiting for the project lock gives up without starting `bd`;
//! - a call whose child runs kills the child's process group (Windows: its job) and
//!   reaps the child, exactly as on a timeout, and releases the locks by returning;
//! - a call waiting for the output of an exited child kills what holds the pipes;
//! - a call that has not started yet starts nothing.
//!
//! Each of these ends with [`BoundedError::Interrupted`](crate::BoundedError::Interrupted)
//! within one poll interval (5 ms) plus the kill bound
//! ([`KILL_BOUND`](crate::bounded::KILL_BOUND)). The request is never cleared: the
//! process is shutting down.
//!
//! # Why a process-wide flag
//!
//! A signal is delivered to the process, not to a call, and the bounded calls run deep
//! inside the cache's composition, which a signal handler cannot reach. So the request
//! is one static of this crate, set once and read by every call (M-AVOID-STATICS
//! accepts it: a signal is process-global by nature). Two copies of this crate in one
//! binary (two incompatible versions) would have two flags; the host must call the
//! [`request`] of the copy its `bd` calls use.
//!
//! [`request`] only stores to an atomic, so it is async-signal-safe and may be called
//! from a signal handler itself as well as from a thread that receives signals.
//!
//! # Examples
//!
//! ```
//! use sc_bd_cli::shutdown;
//! // Nothing asked the process to stop yet.
//! assert_eq!(shutdown::requested(), None);
//! ```

use std::sync::atomic::{AtomicI32, Ordering};

/// The signal that asked the process to stop; `0` while none has.
static REQUESTED: AtomicI32 = AtomicI32::new(0);

/// Asks every bounded call in this process to stop, recording `signal` (the signal
/// number, e.g. 15 for `SIGTERM`).
///
/// The first request wins: a later one does not change the recorded signal, so the
/// host exits with the code of the signal that started the shutdown. A `signal` of
/// zero or less is recorded as `-1`, still a request (a number no signal has).
/// Async-signal-safe: one compare-and-swap on an atomic.
pub fn request(signal: i32) {
    let recorded = if signal > 0 { signal } else { -1 };
    // Losing the race means another request was first; that one is kept.
    let _ = REQUESTED.compare_exchange(0, recorded, Ordering::SeqCst, Ordering::SeqCst);
}

/// The signal of the first [`request`], or `None` while the process has not been
/// asked to stop.
#[must_use]
pub fn requested() -> Option<i32> {
    match REQUESTED.load(Ordering::SeqCst) {
        0 => None,
        signal => Some(signal),
    }
}
