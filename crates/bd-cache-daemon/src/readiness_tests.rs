//! Real HTTP adapter and application evaluation under a whole-process watchdog.
#![expect(clippy::expect_used, reason = "test assertions")]
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use bd_cache_service::{Application, ReadyError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn canceled_http_keeps_actual_read_and_cache_claim_until_cleanup() {
    crate::observability::adapter_tests::bounded_fixture(
        "readiness_tests::canceled_http_keeps_actual_read_and_cache_claim_until_cleanup",
        || {
            tokio::runtime::Runtime::new()
                .expect("runtime")
                .block_on(scenario());
        },
    );
}

async fn request(address: std::net::SocketAddr) -> String {
    let mut socket = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect");
    socket
        .write_all(b"GET /ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("request");
    let mut reply = Vec::new();
    socket.read_to_end(&mut reply).await.expect("response");
    String::from_utf8(reply).expect("UTF-8 HTTP")
}

async fn scenario() {
    let repo = temporary_repo();
    let compose = || compose(repo.path());
    let (app, entry, release) = observed_application(repo.path());
    let state = app.state();
    let (shutdown, stopped) = tokio::sync::watch::channel(false);
    let (expire, expiration) = tokio::sync::watch::channel(false);
    let router = timeout_adapter::instrument_router(
        crate::routes::router(state.with_initial_rendezvous_for_fixture())
            .split_for_parts()
            .0,
        expiration,
        stopped,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    let (stop, stop_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
            .expect("serve");
    });
    let first = tokio::spawn(async move { request(address).await });
    tokio::time::timeout(Duration::from_secs(1), entry)
        .await
        .expect("admission deadline")
        .expect("actual admission");
    let busy = tokio::time::timeout(Duration::from_secs(1), request(address))
        .await
        .expect("busy deadline");
    assert!(
        busy.starts_with("HTTP/1.1 503") && busy.contains("DAEMON.READINESS_BUSY"),
        "{busy}"
    );
    expire.send(true).expect("expire admitted HTTP handler");
    let canceled = tokio::time::timeout(Duration::from_secs(1), first)
        .await
        .expect("request timeout deadline")
        .expect("client");
    assert!(
        canceled.starts_with("HTTP/1.1 503") && canceled.contains("RUNTIME.REQUEST_TIMEOUT"),
        "{canceled}"
    );
    let still_busy = request(address).await;
    assert!(
        still_busy.contains("DAEMON.READINESS_BUSY"),
        "response cancellation must not release actual work: {still_busy}"
    );
    shutdown.send(true).expect("drain state");
    stop.send(()).expect("stop serving");
    tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .expect("HTTP drain deadline")
        .expect("server joined");
    let cleanup = tokio::spawn(app.shutdown());
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if matches!(state.ready().await, Err(ReadyError::Closing)) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cleanup closes admission");
    assert!(
        !cleanup.is_finished(),
        "cleanup must join held real evaluation"
    );
    assert!(
        compose().is_err(),
        "cache file claim retained during cleanup"
    );
    release.send(()).expect("release actual read");
    tokio::time::timeout(Duration::from_secs(2), cleanup)
        .await
        .expect("cleanup deadline")
        .expect("cleanup task")
        .expect("joined");
    let reopened = compose().expect("cache claim released after actual cleanup");
    let restarted = Application::start(reopened, Duration::from_millis(150)).expect("restart");
    tokio::time::timeout(Duration::from_secs(2), restarted.shutdown())
        .await
        .expect("restart cleanup deadline")
        .expect("restart joined");
}

fn observed_application(
    repo: &std::path::Path,
) -> (
    Application,
    tokio::sync::oneshot::Receiver<()>,
    mpsc::Sender<()>,
) {
    let (entered, entry) = tokio::sync::oneshot::channel();
    let entered = Mutex::new(Some(entered));
    let (release, held) = mpsc::channel();
    let held = Mutex::new(held);
    let app = Application::start_observed(
        compose(repo).expect("composed"),
        Duration::from_millis(150),
        move |_deadline| {
            if let Some(entered) = entered.lock().expect("entry lock").take() {
                entered.send(()).expect("entered real health scope");
                // Test-only ordering gate. Its watchdog is independent of the
                // unchanged read deadline: after release, actual health work
                // still observes that deadline and may report expiration.
                held.lock()
                    .expect("release lock")
                    .recv_timeout(Duration::from_secs(5))
                    .expect("fixture release watchdog");
            }
        },
    )
    .expect("application");
    (app, entry, release)
}

fn temporary_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir(repo.path().join(".beads")).expect("beads");
    std::fs::write(
        repo.path().join(".beads/metadata.json"),
        r#"{"dolt_database":"test","project_id":"p-1"}"#,
    )
    .expect("metadata");
    repo
}

fn compose(repo: &std::path::Path) -> Result<bd_cache_sync::Composed, bd_cache_sync::ComposeError> {
    bd_cache_sync::compose(repo, "/never-run-backend", Arc::new(sc_beads::SystemClock))
}

#[cfg(unix)]
mod runtime_lifecycle;

mod owned_runtime;

mod timeout_adapter;

fn tcp_address(endpoint: &sc_transport::Endpoint) -> std::net::SocketAddr {
    match endpoint {
        sc_transport::Endpoint::Tcp(address) => Some(*address),
        _ => None,
    }
    .expect("loopback TCP endpoint required")
}
