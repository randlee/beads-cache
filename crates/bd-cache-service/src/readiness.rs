//! One admitted synchronous health read, owned until actual completion.

use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bd_cache_api::{ApiError, Data};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};

const READ_BUDGET: Duration = Duration::from_secs(1);

/// A health result or finite-admission refusal, without changing domain codes.
#[derive(Debug)]
pub enum ReadyError {
    /// The single capacity or cache mutex is busy.
    Busy,
    /// Cleanup has closed admission.
    Closing,
    /// The existing health evaluation returned an error.
    Cache(ApiError),
}

#[derive(Debug)]
struct Job {
    deadline: Instant,
    response: oneshot::Sender<Result<Data, ReadyError>>,
    // Ownership belongs to real work, never to the request's response wait.
    _permit: OwnedSemaphorePermit,
}

#[derive(Debug)]
struct Admission {
    capacity: Arc<Semaphore>,
    // Shared so cleanup can close every clone of HTTP state at once.
    sender: Mutex<Option<mpsc::SyncSender<Job>>>,
}

/// Cloneable finite readiness admission; it never opens another cache connection.
#[derive(Clone, Debug)]
pub struct ReadyState {
    admission: Arc<Admission>,
    #[cfg(feature = "testing")]
    setup_rendezvous: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl ReadyState {
    /// Submits at most one health read without waiting for capacity or queuing.
    ///
    /// # Errors
    /// Returns busy/closing, or the original cache-health evaluation error.
    pub async fn ready(&self) -> Result<Data, ReadyError> {
        let permit = Arc::clone(&self.admission.capacity)
            .try_acquire_owned()
            .map_err(|_error| {
                if self.admission.capacity.is_closed() {
                    ReadyError::Closing
                } else {
                    ReadyError::Busy
                }
            })?;
        let (response, receive) = oneshot::channel();
        let job = Job {
            deadline: Instant::now() + READ_BUDGET,
            response,
            _permit: permit,
        };
        #[cfg(feature = "testing")]
        let setup = self
            .setup_rendezvous
            .as_ref()
            .is_some_and(|initial| initial.swap(false, std::sync::atomic::Ordering::AcqRel));
        #[cfg(not(feature = "testing"))]
        let setup = false;
        if setup {
            #[cfg(feature = "testing")]
            self.submit_setup(job).await?;
        } else {
            self.submit(job)?;
        }
        // Both admission modes retain precisely the same job/response ownership.
        receive.await.unwrap_or(Err(ReadyError::Closing))
    }

    fn submit(&self, job: Job) -> Result<(), ReadyError> {
        {
            let sender = self
                .admission
                .sender
                .lock()
                .map_err(|_error| ReadyError::Closing)?;
            let sender = sender.as_ref().ok_or(ReadyError::Closing)?;
            sender.try_send(job).map_err(|error| match error {
                mpsc::TrySendError::Full(_) => ReadyError::Busy,
                mpsc::TrySendError::Disconnected(_) => ReadyError::Closing,
            })?;
        }
        Ok(())
    }

    /// Selects one fixture-setup rendezvous on this clone and its descendants.
    ///
    /// This is not production admission proof: the first request waits for the
    /// real zero-capacity receiver instead of requiring ordinary `try_send` to
    /// accept a startup probe. All later requests use production admission.
    /// The fixture must retain its independent owned-process watchdog, since a
    /// synchronous rendezvous cannot be interrupted by an async timeout.
    #[cfg(feature = "testing")]
    #[doc(hidden)]
    #[must_use]
    pub fn with_initial_rendezvous_for_fixture(&self) -> Self {
        Self {
            admission: Arc::clone(&self.admission),
            setup_rendezvous: Some(Arc::new(std::sync::atomic::AtomicBool::new(true))),
        }
    }

    #[cfg(feature = "testing")]
    async fn submit_setup(&self, job: Job) -> Result<(), ReadyError> {
        // Only setup owns this extra sender thread. The actual reader/evaluator
        // is unchanged; no accepted job can be queued on its zero-capacity channel.
        let sender = self
            .admission
            .sender
            .lock()
            .map_err(|_error| ReadyError::Closing)?
            .as_ref()
            .ok_or(ReadyError::Closing)?
            .clone();
        let (done, completed) = oneshot::channel();
        let owned = SetupSender(Some(
            std::thread::Builder::new()
                .name("readiness-fixture-handoff".into())
                .spawn(move || {
                    let sent = sender.send(job).is_ok();
                    let _ = done.send(sent);
                })
                .map_err(|_error| ReadyError::Closing)?,
        ));
        // Awaiting the handoff leaves even current-thread HTTP runtimes runnable.
        // The guard physically joins this sender on success, cancellation or unwind.
        let sent = completed.await.unwrap_or(false);
        drop(owned);
        if sent {
            Ok(())
        } else {
            Err(ReadyError::Closing)
        }
    }

    pub(crate) fn close(&self) {
        self.admission.capacity.close();
        // Poison cannot prevent channel closure and worker join.
        self.admission
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

#[cfg(feature = "testing")]
struct SetupSender(Option<JoinHandle<()>>);

#[cfg(feature = "testing")]
impl Drop for SetupSender {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            // The fixture's independent process watchdog owns failure rescue.
            // Cancellation must not silently detach a blocked sender.
            let _ = sender.join();
        }
    }
}

#[derive(Debug)]
pub(crate) struct Worker(JoinHandle<()>);
impl Worker {
    pub(crate) fn join(self) -> std::thread::Result<()> {
        self.0.join()
    }
}

pub(crate) fn start(
    evaluate: impl Fn(Instant) -> Result<Data, ReadyError> + Send + 'static,
) -> std::io::Result<(ReadyState, Worker)> {
    let (sender, receiver) = mpsc::sync_channel::<Job>(0);
    let worker = std::thread::Builder::new()
        .name("cache-readiness".to_owned())
        .spawn(move || {
            while let Ok(job) = receiver.recv() {
                let result = evaluate(job.deadline);
                let _ = job.response.send(result);
                // The permit drops only here, after evaluate returned and response delivery.
            }
        })?;
    Ok((
        ReadyState {
            admission: Arc::new(Admission {
                capacity: Arc::new(Semaphore::new(1)),
                sender: Mutex::new(Some(sender)),
            }),
            #[cfg(feature = "testing")]
            setup_rendezvous: None,
        },
        Worker(worker),
    ))
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "test assertions")]
    use super::*;

    #[tokio::test]
    async fn nonreceiving_worker_refuses_without_queuing_or_retaining_capacity() {
        let (sender, receiver) = mpsc::sync_channel::<Job>(0);
        let state = ReadyState {
            admission: Arc::new(Admission {
                capacity: Arc::new(Semaphore::new(1)),
                sender: Mutex::new(Some(sender)),
            }),
            #[cfg(feature = "testing")]
            setup_rendezvous: None,
        };
        // Keep the receiver alive but never call recv: try_send must refuse a
        // real nonreceiving worker, rather than accepting or queuing any job.
        assert!(matches!(state.ready().await, Err(ReadyError::Busy)));
        assert_eq!(state.admission.capacity.available_permits(), 1);
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        state.close();
        assert!(matches!(state.ready().await, Err(ReadyError::Closing)));
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn canceled_response_retains_capacity_and_cleanup_joins_real_work() {
        crate::test_support::bounded_fixture(
            "readiness::tests::canceled_response_retains_capacity_and_cleanup_joins_real_work",
            || {
                tokio::runtime::Runtime::new()
                    .expect("runtime")
                    .block_on(scenario(false));
            },
        );
    }

    #[cfg(feature = "testing")]
    #[test]
    fn fixture_setup_rendezvous_keeps_real_work_after_response_cancellation() {
        crate::test_support::bounded_fixture(
            "readiness::tests::fixture_setup_rendezvous_keeps_real_work_after_response_cancellation",
            || {
                // Match the released runtime fixture's current-thread executor.
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("runtime")
                    .block_on(scenario(true));
            },
        );
    }

    async fn scenario(setup: bool) {
        let (entered, entered_rx) = oneshot::channel();
        let entered = Mutex::new(Some(entered));
        let (release, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        let (state, worker) = start(move |_| {
            entered
                .lock()
                .expect("entered lock")
                .take()
                .expect("one evaluation")
                .send(())
                .expect("entered");
            release_rx
                .lock()
                .expect("lock")
                .recv_timeout(Duration::from_secs(2))
                .expect("release");
            Err(ReadyError::Busy)
        })
        .expect("worker");
        // Sender ownership releases the evaluate closure even during unwind.
        // Startup Busy is retried only under a finite watchdog, never by awaiting
        // an admitted response before independently observing worker entry.
        let request_state = state.clone();
        #[cfg(feature = "testing")]
        let request_state = if setup {
            request_state.with_initial_rendezvous_for_fixture()
        } else {
            request_state
        };
        let first = tokio::spawn(async move {
            if setup {
                return request_state.ready().await;
            }
            loop {
                let result = request_state.ready().await;
                if !matches!(result, Err(ReadyError::Busy)) {
                    return result;
                }
                tokio::task::yield_now().await;
            }
        });
        tokio::time::timeout(Duration::from_secs(2), entered_rx)
            .await
            .expect("entry deadline")
            .expect("entered");
        first.abort();
        let _ = first.await;
        assert_eq!(state.admission.capacity.available_permits(), 0);
        assert!(matches!(state.ready().await, Err(ReadyError::Busy)));
        state.close();
        assert!(matches!(state.ready().await, Err(ReadyError::Closing)));
        assert!(
            !worker.0.is_finished(),
            "the signaled evaluator remains held"
        );
        let (joining, join_attempt) = oneshot::channel();
        let joined = tokio::task::spawn_blocking(move || {
            joining.send(()).expect("join attempt");
            worker.join()
        });
        tokio::time::timeout(Duration::from_secs(2), join_attempt)
            .await
            .expect("join attempt deadline")
            .expect("join attempt");
        assert!(
            !joined.is_finished(),
            "join cannot complete while evaluator is held"
        );
        release.send(()).expect("release");
        tokio::time::timeout(Duration::from_secs(2), joined)
            .await
            .expect("join deadline")
            .expect("task")
            .expect("thread");
    }
}
