//! Portable released runtime fixture: actual cleanup, owned endpoint and instance lock.
use super::{compose, observed_application, temporary_repo};
use bd_cache_service::{Application, ReadyError};
use sc_runtime::{Daemon, DaemonConfig, RuntimeError, testing::DaemonFixture};
use sc_transport::{AppName, Endpoint};
use std::time::Duration;
use tokio::sync::{oneshot, watch};
use utoipa_axum::router::OpenApiRouter;

const WAIT: Duration = Duration::from_secs(2);

#[test]
fn canceled_read_joins_under_portable_runtime_ownership() {
    crate::observability::adapter_tests::bounded_fixture(
        "readiness_tests::owned_runtime::canceled_read_joins_under_portable_runtime_ownership",
        || {
            tokio::runtime::Runtime::new()
                .expect("runtime")
                .block_on(scenario());
        },
    );
}

async fn scenario() {
    let repo = temporary_repo();
    let path = repo.path().to_owned();
    let name = AppName::new("readiness-owned-runtime").expect("name");
    let mut config = DaemonConfig::default();
    config.endpoint_flag = Some("tcp:127.0.0.1:0".into());
    let (state_tx, state_rx) = oneshot::channel();
    let (owner_tx, owner_rx) = oneshot::channel();
    let (cleanup_tx, cleanup_rx) = oneshot::channel();
    let (_tracking, stopped) = watch::channel(false);
    let (expire, expiration) = watch::channel(false);
    let host = DaemonFixture::start_with_cleanup(
        &name,
        config.clone(),
        move || async move {
            let (app, entered, release) = observed_application(&path);
            let state = app.state();
            let _ = owner_tx.send(app);
            let _ = state_tx.send((state.clone(), entered, release));
            Ok(state)
        },
        move |state| {
            super::timeout_adapter::instrument_router(
                crate::routes::router(state.with_initial_rendezvous_for_fixture())
                    .split_for_parts()
                    .0,
                expiration,
                stopped,
            )
            .into()
        },
        || async move {
            if let Ok(app) = owner_rx.await {
                let _ = cleanup_tx.send(());
                app.shutdown().await.expect("real worker/readiness joins");
            }
        },
    )
    .await
    .expect("public runtime fixture with cleanup");
    let address = super::tcp_address(host.endpoint());
    assert_eq!(
        std::net::TcpListener::bind(address)
            .expect_err("notified listener remains owned")
            .kind(),
        std::io::ErrorKind::AddrInUse
    );
    config.endpoint_flag = Some(host.endpoint_str());
    config.transport.instance_root = Some(host.instance_root().to_owned());
    let (state, entered, release) = state_rx.await.expect("application state");
    let first = tokio::spawn(async move { super::request(address).await });
    tokio::time::timeout(WAIT, entered)
        .await
        .expect("admission bound")
        .expect("actual read");
    let busy = super::request(address).await;
    assert!(busy.contains("DAEMON.READINESS_BUSY"), "{busy}");
    expire.send(true).expect("expire admitted HTTP handler");
    let canceled = tokio::time::timeout(WAIT, first)
        .await
        .expect("HTTP bound")
        .expect("client joined");
    assert!(canceled.contains("RUNTIME.REQUEST_TIMEOUT"), "{canceled}");
    assert!(
        super::request(address)
            .await
            .contains("DAEMON.READINESS_BUSY")
    );
    let shutdown = tokio::spawn(host.shutdown());
    tokio::time::timeout(WAIT, cleanup_rx)
        .await
        .expect("cleanup bound")
        .expect("actual cleanup entered");
    tokio::time::timeout(WAIT, async {
        loop {
            if matches!(state.ready().await, Err(ReadyError::Closing)) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("admission closes");
    assert!(
        !shutdown.is_finished(),
        "runtime waits for actual readiness join"
    );
    assert_instance_held(&name, &config).await;
    release.send(()).expect("release controlled read");
    tokio::time::timeout(WAIT, shutdown)
        .await
        .expect("cleanup bound")
        .expect("runtime owner joined")
        .expect("runtime clean exit");
    restart(repo.path(), &name, config, address).await;
}

async fn restart(
    repo: &std::path::Path,
    name: &AppName,
    config: DaemonConfig,
    address: std::net::SocketAddr,
) {
    // The public startup must now reach stores on exactly the old root. Stop
    // there with a typed sentinel so this negative never starts another owner.
    let released = Daemon::builder(name, &config)
        .stores(|| async {
            Err::<(), _>(RuntimeError::stores_failed(
                "FIXTURE.REACQUIRED",
                "ownership released after join",
                "expected control",
                None,
            ))
        })
        .routes(|()| OpenApiRouter::new())
        .run()
        .await
        .expect_err("sentinel");
    assert_eq!(released.code(), "FIXTURE.REACQUIRED");
    let app = Application::start(
        compose(repo).expect("cache reopened"),
        Duration::from_millis(150),
    )
    .expect("restart application");
    let state = app.state();
    let restarted = DaemonFixture::start_with_cleanup(
        name,
        config,
        || async { Ok(state) },
        crate::routes::router,
        || async move {
            app.shutdown().await.expect("restart joins");
        },
    )
    .await
    .expect("restart same endpoint");
    assert_eq!(restarted.endpoint(), &Endpoint::Tcp(address));
    restarted.shutdown().await.expect("restarted owner joins");
}

async fn assert_instance_held(name: &AppName, config: &DaemonConfig) {
    let competing = Daemon::builder(name, config)
        .stores(|| async {
            Err::<(), _>(RuntimeError::stores_failed(
                "FIXTURE.EARLY_UNLOCK",
                "released before join",
                "retain ownership",
                None,
            ))
        })
        .routes(|()| OpenApiRouter::new())
        .run()
        .await
        .expect_err("old owner retained");
    assert!(
        matches!(competing, RuntimeError::LockHeld { .. }),
        "{competing}"
    );
}
