//! Cooperative cancellation of the worker's own synchronous schedule sessions.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

thread_local! {
    static MANAGEMENT: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
    static SCOPES: RefCell<Vec<(usize, ScheduleShutdownControl)>> = const { RefCell::new(Vec::new()) };
}

/// A shutdown deadline for one worker view of the existing schedule connection.
///
/// This handle opens no connection and affects only calls through its worker
/// view. Health reads, public schedule reads and ordinary cache work retain
/// their own policies. Already-running mutex and SQLite waits observe updates.
#[derive(Clone, Debug)]
pub struct ScheduleShutdownControl {
    connection: usize,
    deadline: Arc<Mutex<Option<Instant>>>,
    #[cfg(any(test, feature = "testing"))]
    waiting: Arc<Mutex<Option<std::sync::mpsc::Sender<()>>>>,
    #[cfg(any(test, feature = "testing"))]
    management_waiting: Arc<Mutex<Option<std::sync::mpsc::Sender<()>>>>,
}

impl ScheduleShutdownControl {
    pub(crate) fn over(connection: usize) -> Self {
        Self {
            connection,
            deadline: Arc::default(),
            #[cfg(any(test, feature = "testing"))]
            waiting: Arc::default(),
            #[cfg(any(test, feature = "testing"))]
            management_waiting: Arc::default(),
        }
    }

    fn identity(&self) -> (usize, usize) {
        (Arc::as_ptr(&self.deadline) as usize, self.connection)
    }

    /// Marks an inline public management call on this exact worker view.
    ///
    /// Create the guard inside the owned blocking task before calling the
    /// scheduler's synchronous management API. Its same-core reload notification
    /// remains intact. This does not cover worker, run-due or job callback work,
    /// other worker views, other connections, or health read-only/deadline rules.
    #[must_use]
    pub fn management_call(&self) -> ScheduleManagementCall {
        MANAGEMENT.with_borrow_mut(|calls| calls.push(self.identity()));
        ScheduleManagementCall {
            control: self.clone(),
            _thread: PhantomData,
        }
    }

    /// Starts a new worker lifetime after the preceding worker has been joined.
    pub fn restart(&self) {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// Shortens the worker's store deadline, including calls already in progress.
    pub fn finish_by(&self, deadline: Instant) {
        let mut current = self.deadline.lock().unwrap_or_else(PoisonError::into_inner);
        *current = Some(current.map_or(deadline, |d| d.min(deadline)));
    }

    /// Observes the next actual worker mutex, SQLite busy or progress callback.
    #[cfg(feature = "testing")]
    #[doc(hidden)]
    #[must_use]
    pub fn wait_attempt_for_tests(&self) -> std::sync::mpsc::Receiver<()> {
        let (send, receive) = std::sync::mpsc::channel();
        self.observe_wait(send);
        receive
    }

    /// Observes entry into this view's actual management store operation.
    #[cfg(feature = "testing")]
    #[doc(hidden)]
    #[must_use]
    pub fn management_attempt_for_tests(&self) -> std::sync::mpsc::Receiver<()> {
        let (send, receive) = std::sync::mpsc::channel();
        *self
            .management_waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(send);
        receive
    }

    /// Returns the current worker deadline for lifecycle regression assertions.
    #[cfg(feature = "testing")]
    #[doc(hidden)]
    #[must_use]
    pub fn deadline_for_tests(&self) -> Option<Instant> {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn observe_wait(&self, probe: std::sync::mpsc::Sender<()>) {
        *self.waiting.lock().unwrap_or_else(PoisonError::into_inner) = Some(probe);
    }

    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn note_wait(&self) {
        if let Some(probe) = self
            .waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = probe.send(());
        }
    }

    pub(crate) fn expired(&self) -> bool {
        self.deadline
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    pub(crate) fn enter(&self, connection: usize) -> Option<Scope> {
        if connection == self.connection
            && MANAGEMENT.with_borrow(|calls| calls.contains(&self.identity()))
        {
            #[cfg(any(test, feature = "testing"))]
            if let Some(probe) = self
                .management_waiting
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
            {
                let _ = probe.send(());
            }
            return None;
        }
        SCOPES.with_borrow_mut(|scopes| scopes.push((connection, self.clone())));
        Some(Scope)
    }
}

pub(crate) fn current(connection: usize) -> Option<ScheduleShutdownControl> {
    SCOPES.with_borrow(|scopes| {
        scopes
            .iter()
            .rev()
            .find(|(key, _)| *key == connection)
            .map(|(_, control)| control.clone())
    })
}

pub(crate) struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        SCOPES.with_borrow_mut(|scopes| {
            scopes.pop();
        });
    }
}

/// Calling-thread ownership of one explicitly identified management operation.
///
/// The guard is neither Send nor Sync; dropping it restores the worker policy
/// on success, error and unwind before the blocking task returns.
#[derive(Debug)]
pub struct ScheduleManagementCall {
    control: ScheduleShutdownControl,
    _thread: PhantomData<Rc<()>>,
}

impl Drop for ScheduleManagementCall {
    fn drop(&mut self) {
        MANAGEMENT.with_borrow_mut(|calls| {
            if let Some(index) = calls
                .iter()
                .rposition(|identity| *identity == self.control.identity())
            {
                calls.remove(index);
            }
        });
    }
}
