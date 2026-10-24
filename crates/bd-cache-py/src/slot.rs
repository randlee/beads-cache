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
//! - a verb that panics poisons the mutex. The slot then drops the value
//!   (the connection may be mid-statement), clears the poison, closes, and
//!   reports [`Outcome::Poisoned`] once; later calls see [`Outcome::Closed`].

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// What running a verb on a [`Slot`] came to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome<R> {
    /// The verb ran and returned this.
    Done(R),
    /// The slot is closed; the verb did not run.
    Closed,
    /// An earlier verb panicked; the slot dropped its value and closed, and this verb did not run.
    Poisoned,
}

/// A closable, mutex-guarded value.
#[derive(Debug)]
pub(crate) struct Slot<T> {
    value: Mutex<Option<T>>,
    closed: AtomicBool,
}

impl<T> Slot<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: Mutex::new(Some(value)),
            closed: AtomicBool::new(false),
        }
    }

    /// True once `close` has been called (or a panic closed the slot).
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Runs `f` on the value when the slot is open.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> Outcome<R> {
        if self.is_closed() {
            return Outcome::Closed;
        }
        match self.value.lock() {
            Ok(mut guard) => {
                // Checked again under the lock: a close that began while this
                // verb waited must refuse it.
                if self.is_closed() {
                    return Outcome::Closed;
                }
                match guard.as_mut() {
                    Some(v) => Outcome::Done(f(v)),
                    None => Outcome::Closed,
                }
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                let dropped = guard.take();
                self.closed.store(true, Ordering::SeqCst);
                drop(guard);
                self.value.clear_poison();
                drop(dropped);
                Outcome::Poisoned
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
                let taken = poisoned.into_inner().take();
                self.value.clear_poison();
                taken
            }
        };
        drop(taken);
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
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
        assert_eq!(slot.with(|_| 7), Outcome::Done(7));
        assert!(!slot.is_closed());
        slot.close();
        slot.close();
        assert!(slot.is_closed());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(slot.with(|_| 7), Outcome::Closed);
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
        while !slot.is_closed() {
            thread::yield_now();
        }
        assert_eq!(slot.with(|_| 1), Outcome::Closed, "a new verb is refused");
        release_tx.send(()).unwrap();
        assert_eq!(
            verb.join().unwrap(),
            Outcome::Done(1),
            "the in-flight verb finished"
        );
        closer.join().unwrap();
    }

    #[test]
    fn a_panicking_verb_drops_the_value_clears_the_poison_and_closes() {
        let drops = Arc::new(AtomicUsize::new(0));
        let slot = Arc::new(Slot::new(Probe(Arc::clone(&drops))));
        let crashed = {
            let slot = Arc::clone(&slot);
            thread::spawn(move || slot.with(|_| panic!("a verb panicked")))
        };
        assert!(crashed.join().is_err());
        assert_eq!(slot.with(|_| 1), Outcome::Poisoned, "told once");
        assert_eq!(drops.load(Ordering::SeqCst), 1, "the value is gone");
        assert!(slot.is_closed());
        assert_eq!(slot.with(|_| 1), Outcome::Closed, "then plainly closed");
        slot.close();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn close_after_a_panic_still_drops_the_value() {
        let drops = Arc::new(AtomicUsize::new(0));
        let slot = Arc::new(Slot::new(Probe(Arc::clone(&drops))));
        let crashed = {
            let slot = Arc::clone(&slot);
            thread::spawn(move || slot.with(|_| panic!("a verb panicked")))
        };
        assert!(crashed.join().is_err());
        slot.close();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(slot.with(|_| 1), Outcome::Closed);
    }
}
