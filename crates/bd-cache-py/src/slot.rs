//! [`Slot`]: the guarded, closable owner of the handle's one `Cache` (pure Rust).
//!
//! A `Cache` is `Send` but not `Sync` (ADR-CORE-0001), and a Python class must
//! be `Sync`, so the handle keeps it in a `Mutex<Option<T>>`. The slot adds what
//! the handle needs from it (ADR-PY-0003):
//!
//! - `with` runs a verb with the value, one verb at a time;
//! - `close` marks the slot closed *before* it waits for the lock, so
//!   `is_closed` is true at once, a verb that has not started yet is refused, and
//!   the in-flight verb finishes; the value is then dropped by the closer;
//! - a verb that panics is caught at the panic: the slot drops the value (the
//!   connection may be mid-statement), marks itself closed and reports
//!   [`NotRun::Panicked`] with the panic message, all before `with` returns, so
//!   `is_closed` is already true and the file is free to reopen. The panic does not
//!   reach Python. (With `panic = "abort"` the process aborts instead, which is
//!   what a panic means, M-PANIC-IS-STOP.)
//! - [`NotRun::Poisoned`] is defensive. Verbs run under `catch_unwind`, so the lock
//!   is not poisoned by them; only a panic elsewhere while the guard is held (a
//!   `Drop` of the value) could, and nothing in production does that. If it ever
//!   happens the slot drops the value, clears the poison while holding the guard,
//!   closes and reports `Poisoned` once; later calls see [`NotRun::Closed`].

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Why a verb did not run on a [`Slot`] (the `Err` of [`Slot::with`]).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NotRun {
    /// The slot is closed.
    Closed,
    /// The verb panicked with this message; the slot dropped its value and closed.
    Panicked(String),
    /// Defensive (see the module docs): the lock was poisoned by a panic outside a verb;
    /// the slot dropped its value and closed.
    Poisoned,
}

/// A closable, mutex-guarded value.
#[derive(Debug)]
pub(crate) struct Slot<T> {
    value: Mutex<Option<T>>,
    closed: AtomicBool,
}

/// The text of a panic payload.
fn message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_owned())
}

impl<T> Slot<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: Mutex::new(Some(value)),
            closed: AtomicBool::new(false),
        }
    }

    /// True once `close` has been called or a panic closed the slot.
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Runs `f` on the value when the slot is open.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R, NotRun> {
        if self.is_closed() {
            return Err(NotRun::Closed);
        }
        match self.value.lock() {
            Ok(mut guard) => {
                // Checked again under the lock: a close that began while this
                // verb waited must refuse it.
                if self.is_closed() {
                    return Err(NotRun::Closed);
                }
                let Some(value) = guard.as_mut() else {
                    return Err(NotRun::Closed);
                };
                // The value is dropped on a panic, so nothing observes it half-updated.
                match catch_unwind(AssertUnwindSafe(|| f(value))) {
                    Ok(done) => Ok(done),
                    Err(payload) => {
                        let dropped = guard.take();
                        self.closed.store(true, Ordering::SeqCst);
                        drop(guard);
                        drop(dropped);
                        Err(NotRun::Panicked(message(payload.as_ref())))
                    }
                }
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                let dropped = guard.take();
                self.closed.store(true, Ordering::SeqCst);
                // Cleared while the guard is held, so no other caller sees the poison again.
                self.value.clear_poison();
                drop(guard);
                drop(dropped);
                Err(NotRun::Poisoned)
            }
        }
    }

    /// Closes the slot: new verbs are refused at once, the in-flight verb
    /// finishes, then the value is dropped here. Idempotent.
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        // A poisoned lock still holds the value; take it either way.
        let taken = match self.value.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                let taken = guard.take();
                self.value.clear_poison();
                drop(guard);
                taken
            }
        };
        drop(taken);
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]
mod tests {
    use std::sync::mpsc;
    use std::sync::{Arc, atomic::AtomicUsize};
    use std::thread;
    use std::time::Duration;

    use super::*;

    /// Counts drops, to see when the value goes.
    #[derive(Debug)]
    struct Probe(Arc<AtomicUsize>);

    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn runs_verbs_and_close_drops_the_value() {
        let drops = Arc::new(AtomicUsize::new(0));
        let slot = Slot::new(Probe(Arc::clone(&drops)));
        assert_eq!(slot.with(|_| 7), Ok(7));
        assert!(!slot.is_closed());
        slot.close();
        slot.close();
        assert!(slot.is_closed());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(slot.with(|_| 7), Err(NotRun::Closed));
    }

    #[test]
    fn close_marks_closed_at_once_waits_for_the_in_flight_verb_and_refuses_new_ones() {
        let slot = Arc::new(Slot::new(0_u32));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let verb = {
            let slot = Arc::clone(&slot);
            thread::spawn(move || {
                slot.with(|v| {
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(30)).unwrap();
                    *v += 1;
                    *v
                })
            })
        };
        started_rx.recv_timeout(Duration::from_secs(30)).unwrap();
        let closer = {
            let slot = Arc::clone(&slot);
            thread::spawn(move || slot.close())
        };
        // `is_closed` turns true while the verb is still holding the lock.
        let end = std::time::Instant::now() + Duration::from_secs(30);
        while !slot.is_closed() {
            assert!(
                std::time::Instant::now() < end,
                "close() never marked the slot closed"
            );
            thread::yield_now();
        }
        assert_eq!(
            slot.with(|_| 1),
            Err(NotRun::Closed),
            "a new verb is refused"
        );
        release_tx.send(()).unwrap();
        assert_eq!(verb.join().unwrap(), Ok(1), "the in-flight verb finished");
        closer.join().unwrap();
    }

    #[test]
    fn a_panicking_verb_closes_the_slot_at_the_panic_and_drops_the_value() {
        let drops = Arc::new(AtomicUsize::new(0));
        let slot = Slot::new(Probe(Arc::clone(&drops)));
        let outcome = slot.with(|_| -> u32 { panic!("a verb panicked") });
        // Straight after the panic: closed, the value gone, the panic reported, no poison.
        assert_eq!(outcome, Err(NotRun::Panicked("a verb panicked".to_owned())));
        assert!(slot.is_closed());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(!slot.value.is_poisoned());
        assert_eq!(slot.with(|_| 1), Err(NotRun::Closed));
        slot.close();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_panic_with_a_formatted_message_is_reported_with_it() {
        let slot = Slot::new(0_u32);
        let code = 7;
        let outcome = slot.with(|_| -> u32 { panic!("failed with code {code}") });
        assert_eq!(
            outcome,
            Err(NotRun::Panicked("failed with code 7".to_owned()))
        );
    }

    #[test]
    fn a_lock_poisoned_outside_a_verb_is_cleared_under_the_guard_and_reported_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let slot = Arc::new(Slot::new(Probe(Arc::clone(&drops))));
        let poisoner = {
            let slot = Arc::clone(&slot);
            thread::spawn(move || {
                let _guard = slot.value.lock().unwrap();
                panic!("a drop panicked");
            })
        };
        assert!(poisoner.join().is_err());
        assert!(slot.value.is_poisoned());
        assert_eq!(slot.with(|_| 1), Err(NotRun::Poisoned), "told once");
        assert!(!slot.value.is_poisoned());
        assert_eq!(drops.load(Ordering::SeqCst), 1, "the value is gone");
        assert!(slot.is_closed());
        assert_eq!(slot.with(|_| 1), Err(NotRun::Closed), "then plainly closed");
    }

    #[test]
    fn close_of_a_poisoned_slot_still_drops_the_value() {
        let drops = Arc::new(AtomicUsize::new(0));
        let slot = Arc::new(Slot::new(Probe(Arc::clone(&drops))));
        let poisoner = {
            let slot = Arc::clone(&slot);
            thread::spawn(move || {
                let _guard = slot.value.lock().unwrap();
                panic!("a drop panicked");
            })
        };
        assert!(poisoner.join().is_err());
        slot.close();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(!slot.value.is_poisoned());
        assert_eq!(slot.with(|_| 1), Err(NotRun::Closed));
    }
}
