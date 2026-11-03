//! Unix supplement: public runtime lifecycle, real signal, and actual instance lock.
use super::{compose, observed_application, temporary_repo};
use bd_cache_service::{Application, ReadyError, ReadyState};
use sc_runtime::{Daemon, DaemonConfig, RuntimeError};
use sc_transport::AppName;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{oneshot, watch};
use utoipa_axum::router::OpenApiRouter;

const WAIT: Duration = Duration::from_secs(2);
const TEST_NAME: &str = "readiness_tests::runtime_lifecycle::canceled_read_keeps_runtime_instance_locked_through_real_signal_cleanup";

#[test]
fn canceled_read_keeps_runtime_instance_locked_through_real_signal_cleanup() {
    crate::observability::adapter_tests::bounded_fixture(TEST_NAME, || {
        tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(scenario());
    });
}

struct Host {
    state: ReadyState,
    task: tokio::task::JoinHandle<Result<(), RuntimeError>>,
    cleanup: oneshot::Receiver<()>,
    evaluation: Option<(oneshot::Receiver<()>, mpsc::Sender<()>)>,
    // Keep the public timeout instrumentation's watch sender alive. Production
    // runtime signal/drain ownership is separate and remains entirely upstream.
    _tracking: watch::Sender<bool>,
}

async fn start(repo: PathBuf, config: DaemonConfig, observed: bool) -> Host {
    let (state_tx, state_rx) = oneshot::channel();
    let (owner_tx, owner_rx) = oneshot::channel();
    let (cleanup_tx, cleanup_rx) = oneshot::channel();
    let (tracking, stopped) = watch::channel(false);
    let task = tokio::spawn(async move {
        let name = AppName::new("readiness-lifecycle").expect("app name");
        Daemon::builder(&name, &config)
            .stores(move || async move {
                let (app, evaluation) = if observed {
                    let (app, entered, release) = observed_application(&repo);
                    (app, Some((entered, release)))
                } else {
                    let app = Application::start(
                        compose(&repo).expect("compose"),
                        Duration::from_millis(150),
                    )
                    .expect("application");
                    (app, None)
                };
                let state = app.state();
                owner_tx.send(app).expect("cleanup owner");
                state_tx
                    .send((state.clone(), evaluation))
                    .expect("state handshake");
                Ok(state)
            })
            .routes(move |state| instrumented_routes(state, stopped))
            .run_with_cleanup(|| async move {
                // Unlike the earlier HTTP fixture, this callback is invoked by
                // the actual published runtime after drain, under its lock.
                if let Ok(app) = owner_rx.await {
                    cleanup_tx.send(()).expect("runtime cleanup entry");
                    app.shutdown().await.expect("actual application joins");
                }
            })
            .await
    });
    let (state, evaluation) = tokio::time::timeout(WAIT, state_rx)
        .await
        .expect("bounded store startup")
        .expect("runtime opened stores");
    Host {
        state,
        task,
        cleanup: cleanup_rx,
        evaluation,
        _tracking: tracking,
    }
}

fn instrumented_routes(state: ReadyState, stopped: watch::Receiver<bool>) -> OpenApiRouter {
    // Reuse the upstream's public testing middleware to cancel an actual HTTP
    // handler before the unchanged one-second admitted-read budget expires.
    // No production config, signal handler or private runtime control is added.
    sc_runtime::testing::instrument_router(
        crate::routes::router(state).split_for_parts().0,
        Arc::new(sc_command::observability::NoopCanonicalLog::new(
            sc_observability_types::ServiceName::new("readiness-lifecycle").expect("service"),
        )),
        Duration::from_millis(50),
        stopped,
        Duration::from_secs(1),
    )
    .into()
}

async fn http(address: SocketAddr, path: &str) -> std::io::Result<String> {
    let mut socket = tokio::net::TcpStream::connect(address).await?;
    socket
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    let mut reply = Vec::new();
    socket.read_to_end(&mut reply).await?;
    String::from_utf8(reply)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

async fn serving(address: SocketAddr) {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Ok(reply) = http(address, "/health").await {
                assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("bounded real runtime serving handshake");
}

async fn request(address: SocketAddr) -> String {
    tokio::time::timeout(WAIT, http(address, "/ready"))
        .await
        .expect("bounded HTTP response")
        .expect("ready response")
}

async fn terminate_owned_child() {
    // Only the exactly filtered owned child receives a signal. The signal
    // sender itself has a bounded kill/reap path, beneath the whole watchdog.
    assert_eq!(
        std::env::var("SC_ADAPTER_FIXTURE").expect("owned child"),
        TEST_NAME
    );
    let mut child = tokio::process::Command::new("/bin/kill")
        .args(["-TERM", &std::process::id().to_string()])
        .kill_on_drop(true)
        .spawn()
        .expect("SIGTERM sender");
    let status = tokio::time::timeout(WAIT, child.wait()).await;
    if status.is_err() {
        child.start_kill().expect("kill stalled signal sender");
        tokio::time::timeout(WAIT, child.wait())
            .await
            .expect("signal sender reap deadline")
            .expect("signal sender reaped");
    }
    assert!(
        status
            .expect("signal sender deadline")
            .expect("signal sender result")
            .success()
    );
}

async fn assert_instance_held(config: &DaemonConfig) {
    let name = AppName::new("readiness-lifecycle").expect("name");
    let attempt = Daemon::builder(&name, config)
        .stores(|| async {
            // If ownership was wrongly released, fail before bind/serving so
            // this planted negative cannot create an unowned second daemon.
            Err::<(), _>(RuntimeError::stores_failed(
                "FIXTURE.UNEXPECTED_UNLOCK",
                "instance released before reader join",
                "retain ownership",
                None,
            ))
        })
        .routes(|()| OpenApiRouter::new())
        .run();
    let error = tokio::time::timeout(WAIT, attempt)
        .await
        .expect("bounded competing startup")
        .expect_err("instance remains locked");
    assert!(matches!(error, RuntimeError::LockHeld { .. }), "{error}");
}

async fn scenario() {
    let repo = temporary_repo();
    let instance = tempfile::tempdir().expect("instance root");
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").expect("free loopback endpoint");
    let address = reservation.local_addr().expect("address");
    drop(reservation);
    let mut config = DaemonConfig::default();
    config.endpoint_flag = Some(format!("tcp:{address}"));
    config.transport.instance_root = Some(instance.path().to_path_buf());
    let mut host = start(repo.path().to_owned(), config.clone(), true).await;
    serving(address).await;
    let (entered, release) = host.evaluation.take().expect("observed read");
    let first = tokio::spawn(async move {
        loop {
            let reply = request(address).await;
            if !reply.contains("DAEMON.READINESS_BUSY") {
                return reply;
            }
            tokio::task::yield_now().await;
        }
    });
    tokio::time::timeout(WAIT, entered)
        .await
        .expect("admission deadline")
        .expect("actual admitted read");
    let busy = request(address).await;
    assert!(
        busy.starts_with("HTTP/1.1 503") && busy.contains("DAEMON.READINESS_BUSY"),
        "{busy}"
    );
    let canceled = tokio::time::timeout(WAIT, first)
        .await
        .expect("HTTP deadline")
        .expect("client task");
    assert!(
        canceled.starts_with("HTTP/1.1 503") && canceled.contains("RUNTIME.REQUEST_TIMEOUT"),
        "{canceled}"
    );
    assert!(
        request(address).await.contains("DAEMON.READINESS_BUSY"),
        "permit retained after HTTP cancellation"
    );
    terminate_owned_child().await;
    tokio::time::timeout(WAIT, &mut host.cleanup)
        .await
        .expect("runtime cleanup deadline")
        .expect("runtime cleanup entered");
    tokio::time::timeout(WAIT, async {
        loop {
            if matches!(host.state.ready().await, Err(ReadyError::Closing)) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual cleanup closed admission");
    assert!(
        !host.task.is_finished(),
        "runtime must await actual reader join"
    );
    assert_instance_held(&config).await;
    release
        .send(())
        .expect("release real read inside original budget");
    tokio::time::timeout(WAIT, host.task)
        .await
        .expect("bounded runtime cleanup")
        .expect("runtime task")
        .expect("clean runtime exit");
    let restarted = start(repo.path().to_owned(), config, false).await;
    serving(address).await;
    let ready = request(address).await;
    assert!(
        ready.contains("\"ok\":true") && ready.contains("\"probe\":null"),
        "{ready}"
    );
    terminate_owned_child().await;
    tokio::time::timeout(WAIT, restarted.task)
        .await
        .expect("restart cleanup deadline")
        .expect("restart task")
        .expect("restart exits cleanly");
}
