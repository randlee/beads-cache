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

    #[tokio::test]
    async fn canceled_response_retains_capacity_and_cleanup_joins_real_work() {
        let (entered, entered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        let (state, worker) = start(move |_| {
            entered.send(()).expect("entered");
            release_rx.lock().expect("lock").recv().expect("release");
            Err(ReadyError::Busy)
        })
        .expect("worker");
        // A rendezvous refuses while the thread is starting; no second job queues.
        let first = loop {
            let state = state.clone();
            let request = tokio::spawn(async move { state.ready().await });
            tokio::time::sleep(Duration::from_millis(5)).await;
            if entered_rx.try_recv().is_ok() {
                break request;
            }
            assert!(matches!(
                request.await.expect("request"),
                Err(ReadyError::Busy)
            ));
        };
        first.abort();
        assert!(matches!(state.ready().await, Err(ReadyError::Busy)));
        state.close();
        assert!(matches!(state.ready().await, Err(ReadyError::Closing)));
        let joined = tokio::task::spawn_blocking(move || worker.join());
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!joined.is_finished());
        release.send(()).expect("release");
        joined.await.expect("task").expect("thread");
    }
}
