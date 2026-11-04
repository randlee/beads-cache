//! Public retained-management startup contention, with real caller-owned joins.
#![expect(clippy::expect_used, reason = "test fixture assertions")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use sc_cron::ScheduleStore;

use crate::{Application, ReadyError};

#[test]
fn startup_shutdown_joins_before_retained_management_writer_release() {
    crate::test_support::bounded_fixture(
        "startup_tests::startup_shutdown_joins_before_retained_management_writer_release",
        || {
            tokio::runtime::Runtime::new()
                .expect("runtime")
                .block_on(startup_shutdown(Duration::from_millis(250)));
        },
    );
}

#[test]
fn startup_zero_timeout_joins_before_retained_management_writer_release() {
    crate::test_support::bounded_fixture(
        "startup_tests::startup_zero_timeout_joins_before_retained_management_writer_release",
        || {
            tokio::runtime::Runtime::new()
                .expect("runtime")
                .block_on(startup_shutdown(Duration::ZERO));
        },
    );
}

// Rescue the ordinary caller on failed observations and assertion unwind too.
// The enclosing owned-child watchdog bounds even a broken product join.
struct Management {
    writer: rusqlite::Connection,
    thread: Option<std::thread::JoinHandle<Result<bool, sc_cron::CronError>>>,
}

impl Management {
    fn release_and_join(&mut self) -> Result<bool, sc_cron::CronError> {
        self.writer
            .execute_batch("ROLLBACK")
            .expect("release writer");
        self.thread
            .take()
            .expect("owned management thread")
            .join()
            .expect("management thread joined")
    }
}

impl Drop for Management {
    fn drop(&mut self) {
        let _ = self.writer.execute_batch("ROLLBACK");
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn compose(repo: &std::path::Path) -> bd_cache_sync::Composed {
    bd_cache_sync::compose(repo, "/never-run-backend", Arc::new(sc_beads::SystemClock))
        .expect("compose without backend")
}

async fn prepared_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repo");
    std::fs::create_dir(repo.path().join(".beads")).expect("beads");
    std::fs::write(
        repo.path().join(".beads/metadata.json"),
        r#"{"dolt_database":"test","project_id":"p-1"}"#,
    )
    .expect("metadata");
    let schedules = compose(repo.path())
        .into_schedules()
        .expect("schedule management");
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
    repo
}

async fn startup_shutdown(grace: Duration) {
    let repo = prepared_repo().await;
    let composed = compose(repo.path());
    let store = composed.schedule_store().expect("public retained store");
    let name = store.list_active().expect("active row").remove(0).spec.name;
    let writer = rusqlite::Connection::open(&composed.cache_path).expect("external writer");
    writer
        .execute_batch("BEGIN IMMEDIATE")
        .expect("writer lock");
    let management_store = Arc::clone(&store);
    let mut management = Management {
        writer,
        thread: Some(std::thread::spawn(move || {
            management_store.set_enabled(&name, false)
        })),
    };
    let (probe, control) = store.worker_view();
    let contention = control.wait_attempt_for_tests();
    let probing = std::thread::spawn(move || {
        loop {
            if let Err(error) = probe.list_active() {
                return error;
            }
            std::thread::yield_now();
        }
    });
    let management_entered = contention.recv_timeout(Duration::from_secs(1));
    control.finish_by(Instant::now());
    let probe_result = probing.join();
    // Real failed connection acquisition proves the ordinary management write
    // owns the shared mutex; this is neither a timing guess nor a fake lock.
    management_entered.expect("management owns shared connection");
    probe_result.expect("canceled contention probe really joined");
    let app = Application::start(composed, grace).expect("public application start");
    let state = app.state();
    // Strengthen the original public probe: observe this exact application's
    // startup read waiting before stop, rather than relying on task scheduling.
    let startup_entered = app
        .worker_wait
        .as_ref()
        .expect("system worker control")
        .recv_timeout(Duration::from_secs(1));
    let before_callback = sc_bd_cli::shutdown::requested();
    let asked = Instant::now();
    let shutdown = app.shutdown();
    tokio::pin!(shutdown);
    let bounded = tokio::time::timeout(Duration::from_millis(500), &mut shutdown).await;
    let elapsed = asked.elapsed();
    let joined_before_release = bounded.is_ok();
    let still_pending = !management
        .thread
        .as_ref()
        .expect("management")
        .is_finished();
    let callback = sc_bd_cli::shutdown::requested();
    let readiness = state.ready().await;
    // Rescue precedes every assertion and we retain the same shutdown future.
    let management_result = management.release_and_join();
    let result = match bounded {
        Ok(result) => result,
        Err(_) => shutdown.await,
    };
    startup_entered.expect("actual Application startup read was contended");
    result.expect("actual worker and readiness thread joined");
    assert!(management_result.expect("normal management policy retained"));
    assert!(
        joined_before_release,
        "startup shutdown waited for writer release"
    );
    assert!(still_pending, "shutdown canceled unrelated management work");
    assert_eq!(before_callback, None, "no premature expiry callback");
    assert_eq!(callback, Some(15), "production expiry callback ran");
    assert!(matches!(readiness, Err(ReadyError::Closing)));
    let limit = if grace.is_zero() {
        Duration::from_millis(100) // existing zero-timeout scheduling tolerance
    } else {
        grace * 2
    };
    assert!(
        elapsed < limit,
        "original shutdown bound exceeded: {elapsed:?}"
    );
    assert!(
        !store
            .list_active()
            .expect("normal store after stop")
            .iter()
            .any(|r| r.enabled)
    );
    drop(store);
    assert_eq!(compose(repo.path()).facts().store_schema_version, 3);
    eprintln!(
        "Application startup grace={grace:?}: joined={elapsed:?}, callback={callback:?}, readiness={readiness:?}, management pending until rollback; probe/management/worker/readiness joined and cache reopened"
    );
}
