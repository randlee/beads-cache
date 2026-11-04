//! Calling-thread health budgets over the existing connection, never a watchdog.

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use bd_cache_model::CacheError;

use crate::store::{Inner, Shared};

thread_local! {
    #[cfg(test)]
    static EXPIRE_AFTER_DECODES: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static ACTIVE: RefCell<Vec<Option<Instant>>> = const { RefCell::new(Vec::new()) };
    static BUDGETS: RefCell<Vec<(usize, Instant)>> = const { RefCell::new(Vec::new()) };
}

/// A read-only health budget tied to the cache's one connection and file claim.
///
/// Cloning this handle opens no connection. A scope applies only on its calling
/// thread; concurrent scheduler work keeps its normal lock and SQLite settings.
#[derive(Clone, Debug)]
pub struct HealthReadControl {
    pub(crate) shared: Arc<Shared>,
}

impl HealthReadControl {
    /// Runs ordinary store reads under an absolute monotonic deadline.
    ///
    /// Connection acquisition retries every millisecond. SQLite checks its
    /// deadline every 1000 VM instructions and on each busy retry; row decoding
    /// checks between rows. The scheduling tolerance is 50 ms plus one row's
    /// decoding time. The scope restores settings on error and unwind.
    ///
    /// # Errors
    /// Returns `BD_CACHE.STORE_FAILED` when the budget expires or a mutation is
    /// attempted. Preserves the original read error and source otherwise.
    pub fn within<T, E>(
        &self,
        deadline: Instant,
        read: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CacheError>,
    {
        let key = Arc::as_ptr(&self.shared) as usize;
        let deadline = self
            .shared
            .health_deadline()
            .map_or(deadline, |d| d.min(deadline));
        check(Some(deadline)).map_err(E::from)?;
        BUDGETS.with_borrow_mut(|budgets| budgets.push((key, deadline)));
        let _scope = Scope;
        let value = read()?;
        // A final check covers a read that did no SQL, and the last decoded row.
        check(Some(deadline)).map_err(E::from)?;
        Ok(value)
    }
}

struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        BUDGETS.with_borrow_mut(|budgets| {
            budgets.pop();
        });
    }
}

pub(crate) fn check(deadline: Option<Instant>) -> Result<(), CacheError> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(expired());
    }
    Ok(())
}

pub(crate) fn is_scoped() -> bool {
    ACTIVE.with_borrow(|sessions| sessions.last().copied().flatten().is_some())
}

pub(crate) fn check_decode() -> Result<(), CacheError> {
    #[cfg(test)]
    EXPIRE_AFTER_DECODES.with(|remaining| {
        if let Some(n) = remaining.get() {
            remaining.set(Some(n.saturating_sub(1)));
            if n == 0 {
                ACTIVE.with_borrow_mut(|active| {
                    if let Some(deadline) = active.last_mut() {
                        *deadline = Some(Instant::now());
                    }
                });
            }
        }
    });
    check(ACTIVE.with_borrow(|sessions| sessions.last().copied().flatten()))
}

pub(crate) fn reject_mutation(deadline: Option<Instant>) -> Result<(), CacheError> {
    if deadline.is_some() {
        return Err(CacheError::store_failed(
            "bounded health read: mutation refused in read-only scope",
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "health scope is read only",
            ),
        ));
    }
    Ok(())
}

fn expired() -> CacheError {
    CacheError::store_failed(
        "bounded health read: deadline expired while acquiring or reading the cache connection",
        std::io::Error::new(std::io::ErrorKind::TimedOut, "health read deadline expired"),
    )
}

fn busy(_: i32) -> bool {
    let deadline = ACTIVE.with_borrow(|sessions| sessions.last().copied().flatten());
    let Some(left) = deadline.and_then(|d| d.checked_duration_since(Instant::now())) else {
        return false;
    };
    std::thread::sleep(left.min(Duration::from_millis(1)));
    deadline.is_some_and(|d| Instant::now() < d)
}

/// Restores connection settings before releasing the connection mutex.
pub(crate) struct HealthSession<'a> {
    guard: MutexGuard<'a, Inner>,
    restore: Option<(i64, bool)>,
}

impl Shared {
    pub(crate) fn health_deadline(&self) -> Option<Instant> {
        let key = std::ptr::from_ref(self) as usize;
        BUDGETS.with_borrow(|budgets| {
            budgets
                .iter()
                .filter(|(id, _)| *id == key)
                .map(|(_, d)| *d)
                .min()
        })
    }

    pub(crate) fn session_lock(&self) -> Result<HealthSession<'_>, CacheError> {
        let deadline = self.health_deadline();
        let guard = if let Some(deadline) = deadline {
            loop {
                check(Some(deadline))?;
                match self.inner.try_lock() {
                    Ok(guard) => break guard,
                    Err(TryLockError::Poisoned(e)) => break e.into_inner(),
                    Err(TryLockError::WouldBlock) => {
                        std::thread::sleep(
                            deadline
                                .saturating_duration_since(Instant::now())
                                .min(Duration::from_millis(1)),
                        );
                    }
                }
            }
        } else {
            #[cfg(test)]
            if matches!(self.inner.try_lock(), Err(TryLockError::WouldBlock))
                && let Some(probe) = self
                    .contention_probe
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_ref()
            {
                let _ = probe.send(());
            }
            self.lock()
        };
        ACTIVE.with_borrow_mut(|sessions| sessions.push(deadline));
        let mut session = HealthSession {
            guard,
            restore: None,
        };
        if let Some(deadline) = deadline {
            let conn = &session.guard.conn;
            let busy_ms = conn
                .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
                .map_err(setting)?;
            let query_only = conn
                .query_row("PRAGMA query_only", [], |r| r.get(0))
                .map_err(setting)?;
            session.restore = Some((busy_ms, query_only));
            let conn = &session.guard.conn;
            conn.pragma_update(None, "query_only", true)
                .map_err(setting)?;
            conn.busy_handler(Some(busy)).map_err(setting)?;
            conn.progress_handler(1000, Some(move || Instant::now() >= deadline))
                .map_err(setting)?;
            check(Some(deadline))?;
        }
        Ok(session)
    }
}

fn setting(error: rusqlite::Error) -> CacheError {
    CacheError::store_failed("bounded health read: configuring connection", error)
}

impl Deref for HealthSession<'_> {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.guard
    }
}
impl DerefMut for HealthSession<'_> {
    fn deref_mut(&mut self) -> &mut Inner {
        &mut self.guard
    }
}
impl Drop for HealthSession<'_> {
    fn drop(&mut self) {
        if let Some((busy_ms, query_only)) = self.restore {
            let conn = &self.guard.conn;
            let progress = conn.progress_handler(0, None::<fn() -> bool>);
            // Each setter is independent: still restore the other after an error.
            let busy =
                conn.busy_timeout(Duration::from_millis(u64::try_from(busy_ms).unwrap_or(0)));
            let read_only = conn.pragma_update(None, "query_only", query_only);
            if let Err(error) = progress.and(busy).and(read_only) {
                sc_beads::logging::tracing::error!(%error, "restoring health connection settings failed");
            }
        }
        ACTIVE.with_borrow_mut(|sessions| {
            sessions.pop();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseIdentity, DoltDatabase, ProjectId, SqliteStore};
    use bd_cache_model::{CacheStore, Scope, ScopeId};
    use sc_cron::{NewSchedule, ScheduleSpec, ScheduleStore};

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let identity = DatabaseIdentity {
            dolt_database: DoltDatabase::new("health").unwrap(),
            project_id: ProjectId::new("health").unwrap(),
        };
        let store = SqliteStore::open_at(&dir.path().join("health.db"), &identity).unwrap();
        (dir, store)
    }

    #[test]
    fn mutex_wait_is_bounded_and_does_not_wait_for_the_holder() {
        const CHILD_MODE: &str = "BD_CACHE_HEALTH_MUTEX_CHILD";
        if let Ok(mode) = std::env::var(CHILD_MODE) {
            mutex_wait_fixture(mode == "stalled");
            return;
        }
        let mut child = MutexChild::spawn(CHILD_MODE, "normal");
        let status = child.finish().unwrap_or_else(|| {
            panic!(
                "health mutex child exceeded its five-second lifetime: {}",
                child.output()
            )
        });
        assert!(
            status.success(),
            "health mutex child failed: {}",
            child.output()
        );
        assert!(
            child.output().contains("1 passed"),
            "child did not execute the mutex scenario: {}",
            child.output()
        );
    }

    /// Owns the whole synchronous scenario: a stuck reader can only outlive the
    /// assertion inside this process, never the supervising test's deadline.
    struct MutexChild {
        process: std::process::Child,
        output: tempfile::NamedTempFile,
        _scratch: tempfile::TempDir,
    }

    impl MutexChild {
        fn spawn(key: &str, mode: &str) -> Self {
            let output = tempfile::NamedTempFile::new().unwrap();
            let scratch = tempfile::tempdir().unwrap();
            let stdout = output.reopen().unwrap();
            let stderr = stdout.try_clone().unwrap();
            let process = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "health::tests::mutex_wait_is_bounded_and_does_not_wait_for_the_holder",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(key, mode)
                // The parent removes even a killed child's temporary cache.
                .env("TMPDIR", scratch.path())
                .env("TMP", scratch.path())
                .env("TEMP", scratch.path())
                .stdin(std::process::Stdio::null())
                .stdout(stdout)
                .stderr(stderr)
                .spawn()
                .unwrap();
            Self {
                process,
                output,
                _scratch: scratch,
            }
        }

        fn finish(&mut self) -> Option<std::process::ExitStatus> {
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(status) = self.process.try_wait().unwrap() {
                    return Some(status);
                }
                if Instant::now() >= until {
                    self.kill_and_reap();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn kill_and_reap(&mut self) {
            // Once killed, wait only reaps the OS child; no test thread or cache
            // lock must cooperate. Drop also covers assertions and unwind.
            let _ = self.process.kill();
            self.process.wait().unwrap();
        }

        fn output(&self) -> String {
            std::fs::read_to_string(self.output.path()).unwrap()
        }
    }

    impl Drop for MutexChild {
        fn drop(&mut self) {
            let _ = self.process.kill();
            let _ = self.process.wait();
        }
    }

    #[test]
    fn mutex_wait_supervisor_kills_and_reaps_a_stalled_reader() {
        let mut child = MutexChild::spawn("BD_CACHE_HEALTH_MUTEX_CHILD", "stalled");
        assert!(
            child.finish().is_none(),
            "stalled fixture unexpectedly exited: {}",
            child.output()
        );
        assert!(
            child.process.try_wait().unwrap().is_some(),
            "timed-out child was not reaped"
        );
        assert!(
            child
                .output()
                .contains("health read did not return while holder remained locked"),
            "stalled reader did not reach the contended-read failure path: {}",
            child.output()
        );
    }

    fn mutex_wait_fixture(stall_reader: bool) {
        let (_dir, store) = store();
        let control = store.health_read_control();
        let shared = Arc::clone(&control.shared);
        let (ready, observed) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let holder = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            let guard = loop {
                if let Ok(guard) = shared.inner.try_lock() {
                    break guard;
                }
                assert!(
                    Instant::now() < until,
                    "holder could not acquire connection"
                );
                std::thread::yield_now();
            };
            ready.send(()).unwrap();
            let _ = released.recv(); // owner Drop closes this on every unwind
            drop(guard);
        });
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        let (began, starting) = std::sync::mpsc::channel();
        let (answer, answers) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let started = Instant::now();
            began.send(()).unwrap();
            if stall_reader {
                // Fault injection: model a synchronous health read that never
                // completes, even after the holder releases its connection.
                loop {
                    std::thread::park();
                }
            }
            let result = control.within(started + Duration::from_millis(50), || store.status());
            let _ = answer.send((result, started.elapsed(), store));
        });
        starting.recv_timeout(Duration::from_secs(5)).unwrap();
        let bounded = answers.recv_timeout(Duration::from_millis(200));
        drop(release); // explicit release occurs after read refusal, or on failure
        if let Err(error) = &bounded {
            eprintln!("health read did not return while holder remained locked: {error}");
        }
        // These synchronous joins are confined to the owned child. Even an
        // actual deadlock here is killed and reaped by the parent after 5 s.
        reader.join().unwrap();
        holder.join().unwrap();
        let (result, elapsed, store) = bounded.expect("health read exceeded its answer deadline");
        let error = result.unwrap_err();
        assert_eq!(error.code(), "BD_CACHE.STORE_FAILED");
        assert!(error.detail().contains("bounded health read"));
        assert!(elapsed < Duration::from_millis(150));
        assert!(store.status().is_ok());
    }

    const LONG_SQL: &str = "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n";

    #[test]
    fn sqlite_work_finishes_within_one_second_and_scheduler_write_survives() {
        let (_dir, store) = store();
        let control = store.health_read_control();
        let schedules = store.schedule_store().unwrap();
        let (attempt, attempted) = std::sync::mpsc::channel();
        *control.shared.contention_probe.lock().unwrap() = Some(attempt);
        let (done, completed) = std::sync::mpsc::channel();
        let mut writer = None;
        let start = Instant::now();
        let result: Result<i64, CacheError> =
            control.within(start + Duration::from_secs(1), || {
                let session = control.shared.session_lock()?;
                // Only spawn once the health session owns the connection. The
                // probe acknowledges a real failed acquisition by the normal writer.
                writer = Some(std::thread::spawn(move || {
                    let at = "2026-11-01T00:00:00Z".parse().unwrap();
                    let result = schedules.create(
                        &NewSchedule {
                            spec: ScheduleSpec::new(
                                "normal",
                                "@hourly",
                                Duration::from_secs(1),
                                serde_json::json!({}),
                            )
                            .unwrap(),
                            jitter: Duration::ZERO,
                            next_run_at: at,
                        },
                        at,
                    );
                    let _ = done.send(result);
                }));
                attempted.recv_timeout(Duration::from_millis(500)).unwrap();
                session
                    .conn
                    .query_row(LONG_SQL, [], |r| r.get(0))
                    .map_err(setting)
            });
        let elapsed = start.elapsed();
        completed
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        writer.unwrap().join().unwrap();
        *control.shared.contention_probe.lock().unwrap() = None;
        assert_eq!(result.unwrap_err().code(), "BD_CACHE.STORE_FAILED");
        assert!(
            elapsed < Duration::from_millis(1100),
            "the blocking work actually ended"
        );
        assert_eq!(
            store.schedule_store().unwrap().list_active().unwrap().len(),
            1
        );
        // A stale progress hook would interrupt this after the budget elapsed.
        let count: i64 = store.inner().conn.query_row("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100000) SELECT sum(x) FROM n", [], |r| r.get(0)).unwrap();
        assert!(count > 100_000);
    }

    #[test]
    fn row_iteration_checks_the_deadline_between_decodes() {
        let (_dir, store) = store();
        let control = store.health_read_control();
        let started = Instant::now();
        let result: Result<(), CacheError> = control.within(started + Duration::from_millis(40), || {
            let guard = control.shared.session_lock()?;
            let mut statement = guard.conn.prepare("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000) SELECT x FROM n").map_err(setting)?;
            let mut rows = statement.query([]).map_err(setting)?;
            while rows.next().map_err(setting)?.is_some() {
                check_decode()?;
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_millis(140));
        assert!(store.status().is_ok());
    }

    #[test]
    fn read_only_error_and_unwind_restore_all_settings_before_unlock() {
        let (_dir, mut store) = store();
        let control = store.health_read_control();
        let scope = Scope::legacy(ScopeId::new("normal").unwrap());
        let error = control
            .within(Instant::now() + Duration::from_secs(1), || {
                store.ensure_scope(&scope)
            })
            .unwrap_err();
        assert_eq!(error.code(), "BD_CACHE.STORE_FAILED");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), CacheError> =
                control.within(Instant::now() + Duration::from_secs(1), || {
                    let _session = control.shared.session_lock()?;
                    std::panic::resume_unwind(Box::new("health unwind probe"));
                });
        }));
        assert!(result.is_err());
        let guard = store.inner();
        let query_only: bool = guard
            .conn
            .pragma_query_value(None, "query_only", |r| r.get(0))
            .unwrap();
        let busy_ms: u64 = guard
            .conn
            .pragma_query_value(None, "busy_timeout", |r| r.get::<_, i64>(0))
            .map(|n| u64::try_from(n).unwrap())
            .unwrap();
        assert!(!query_only);
        assert_eq!(
            busy_ms,
            u64::try_from(crate::BUSY_TIMEOUT.as_millis()).unwrap()
        );
        drop(guard);
        assert!(store.ensure_scope(&scope).is_ok());
    }
    #[test]
    fn public_store_row_decoding_obeys_the_budget() {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                EXPIRE_AFTER_DECODES.with(|n| n.set(None));
            }
        }
        let (_dir, mut store) = store();
        let scope = Scope::new(
            ScopeId::new("decode").unwrap(),
            bd_cache_model::ScopeFilter::All,
        );
        let rows: Vec<_> = (0..10)
            .map(|n| sc_beads::testing::bead(&format!("b{n}")))
            .collect();
        store.upsert_rows(Some(&scope), &rows, false).unwrap();
        let control = store.health_read_control();
        let _reset = Reset;
        EXPIRE_AFTER_DECODES.with(|n| n.set(Some(2)));
        let result = control.within(Instant::now() + Duration::from_secs(1), || {
            store.rows_in_scope(&scope.id)
        });
        assert!(result.unwrap_err().detail().contains("bounded health read"));
        EXPIRE_AFTER_DECODES.with(|n| n.set(None));
        assert_eq!(store.rows_in_scope(&scope.id).unwrap().len(), 10);
        assert!(
            control
                .within(Instant::now() + Duration::from_secs(1), || store.status())
                .is_ok()
        );
    }

    #[test]
    fn scope_filters_and_status_assembly_check_each_boundary() {
        let (_dir, mut store) = store();
        for n in 0..10 {
            store
                .ensure_scope(&Scope::new(
                    ScopeId::new(format!("s{n}")).unwrap(),
                    bd_cache_model::ScopeFilter::label("area").unwrap(),
                ))
                .unwrap();
        }
        let control = store.health_read_control();
        for status in [false, true] {
            EXPIRE_AFTER_DECODES.with(|n| n.set(Some(2)));
            let result = control.within(Instant::now() + Duration::from_secs(1), || {
                if status {
                    store.status().map(|_| ())
                } else {
                    store.scopes().map(|_| ())
                }
            });
            EXPIRE_AFTER_DECODES.with(|n| n.set(None));
            assert!(result.unwrap_err().detail().contains("bounded health read"));
        }
        assert_eq!(store.scopes().unwrap().len(), 10);
    }
}
