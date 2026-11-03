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
pub struct ReadyState(Arc<Admission>);

impl ReadyState {
    /// Submits at most one health read without waiting for capacity or queuing.
    ///
    /// # Errors
    /// Returns busy/closing, or the original cache-health evaluation error.
    pub async fn ready(&self) -> Result<Data, ReadyError> {
        let permit = Arc::clone(&self.0.capacity)
            .try_acquire_owned()
            .map_err(|_error| {
                if self.0.capacity.is_closed() {
                    ReadyError::Closing
                } else {
                    ReadyError::Busy
                }
            })?;
        let (response, receive) = oneshot::channel();
        {
            let sender = self.0.sender.lock().map_err(|_error| ReadyError::Closing)?;
            let sender = sender.as_ref().ok_or(ReadyError::Closing)?;
            sender
                .try_send(Job {
                    deadline: Instant::now() + READ_BUDGET,
                    response,
                    _permit: permit,
                })
                .map_err(|error| match error {
                    mpsc::TrySendError::Full(_) => ReadyError::Busy,
                    mpsc::TrySendError::Disconnected(_) => ReadyError::Closing,
                })?;
        }
        receive.await.unwrap_or(Err(ReadyError::Closing))
    }

    pub(crate) fn close(&self) {
        self.0.capacity.close();
        // Poison cannot prevent channel closure and worker join.
        self.0
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
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
        ReadyState(Arc::new(Admission {
            capacity: Arc::new(Semaphore::new(1)),
            sender: Mutex::new(Some(sender)),
        })),
        Worker(worker),
    ))
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "test assertions")]
    use super::*;

    #[test]
    fn canceled_response_retains_capacity_and_cleanup_joins_real_work() {
        crate::test_support::bounded_fixture(
            "readiness::tests::canceled_response_retains_capacity_and_cleanup_joins_real_work",
            || {
                tokio::runtime::Runtime::new()
                    .expect("runtime")
                    .block_on(scenario());
            },
        );
    }

    async fn scenario() {
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
        let first = tokio::spawn(async move {
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
        assert_eq!(state.0.capacity.available_permits(), 0);
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
