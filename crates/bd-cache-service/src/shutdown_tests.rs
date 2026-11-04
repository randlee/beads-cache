//! Caller binding to the qualified cache worker while a real writer retains SQLite.
#![expect(clippy::expect_used, reason = "test fixture assertions")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::Application;

#[test]
fn application_joins_contended_worker_before_external_writer_release() {
    crate::test_support::bounded_fixture(
        "shutdown_tests::application_joins_contended_worker_before_external_writer_release",
        || {
            tokio::runtime::Runtime::new()
                .expect("runtime")
                .block_on(contended_shutdown(Duration::from_millis(250)));
        },
    );
}

#[test]
fn zero_timeout_application_joins_contended_worker_without_grace() {
    crate::test_support::bounded_fixture(
        "shutdown_tests::zero_timeout_application_joins_contended_worker_without_grace",
        || {
            tokio::runtime::Runtime::new()
                .expect("runtime")
                .block_on(contended_shutdown(Duration::ZERO));
        },
    );
}

async fn contended_shutdown(grace: Duration) {
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir(repo.path().join(".beads")).expect("beads");
    std::fs::write(
        repo.path().join(".beads/metadata.json"),
        r#"{"dolt_database":"test","project_id":"p-1"}"#,
    )
    .expect("metadata");
    let compose = || {
        bd_cache_sync::compose(
            repo.path(),
            "/never-run-backend",
            Arc::new(sc_beads::SystemClock),
        )
        .expect("compose without backend")
    };
    let schedules = compose().into_schedules().expect("schedule management");
    schedules
        .set(&bd_cache_sync::ScheduleSet {
            expr: "* * * * * *".into(),
            budget_ms: None,
            hook_reserve_ms: None,
            post_sync: None,
            python: None,
            env: vec![],
        })
        .await
        .expect("persist real built-in row");
    drop(schedules);
    let composed = compose();
    let cache_path = composed.cache_path.clone();
    // This independent test-only writer models another cache process. The
    // production application still owns exactly its one existing connection.
    let writer = rusqlite::Connection::open(cache_path).expect("external writer");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("writer lock");
    let app = Application::start(composed, grace).expect("actual application startup");
    let state = app.state();
    // The signal originates in this application's real worker busy callback,
    // not startup timing or a second scheduler constructed by the fixture.
    let entered = app
        .worker_wait
        .as_ref()
        .expect("system constructor retains bounded worker control")
        .recv_timeout(Duration::from_secs(5));
    let asked = Instant::now();
    let shutdown = app.shutdown();
    tokio::pin!(shutdown);
    let bounded = tokio::time::timeout(Duration::from_millis(500), &mut shutdown).await;
    let elapsed = asked.elapsed();
    let joined_before_release = bounded.is_ok();
    writer.execute_batch("ROLLBACK").expect("release writer");
    // Preserve actual join ownership on a failing observation as well. The
    // complete fixture's ten-second child watchdog bounds a stuck join.
    let result = match bounded {
        Ok(result) => result,
        Err(_) => shutdown.await,
    };
    entered.expect("real worker was contended before stop");
    result.expect("application joined worker and readiness thread");
    assert!(joined_before_release, "cleanup waited for writer release");
    let limit = if grace.is_zero() {
        // Existing producer zero-timeout tolerance covers scheduling only.
        Duration::from_millis(100)
    } else {
        grace * 2
    };
    assert!(elapsed < limit, "shutdown exceeded {limit:?}: {elapsed:?}");
    assert!(matches!(
        state.ready().await,
        Err(crate::ReadyError::Closing)
    ));
    let reopened = compose();
    assert_eq!(reopened.facts().store_schema_version, 3);
    eprintln!(
        "actual Application shutdown under SQLite contention: grace={grace:?}, joined={elapsed:?}, writer remained locked until join"
    );
}
