//! Compile-time pin of the public items sprint a-2 adds to btit's API.
//!
//! The "Public contract" block of `docs/plans/phase-a/sprint-a-2-sc-bd-cli.md` is
//! normative; this file fails to compile when a signature in it changes. btit's own
//! items are pinned by `cli_api_freeze.rs` and `bd_api_freeze.rs`.

use std::sync::Arc;
use std::time::Duration;

use sc_bd_cli::argv;
use sc_bd_cli::bounded::run_bounded;
use sc_bd_cli::cache::UNREACHABLE_MARKERS;
use sc_bd_cli::testing::{FakeBd, RecordingInvoker};
use sc_bd_cli::{BdCli, BoundedError, CliInvoker, ProjectLocks};
use sc_beads::error::BeadsError;
use sc_beads::testing::{FakeBackend, ManualClock};
use sc_beads::{
    Actor, BeadId, CacheBackend, CliOutput, Clock, DepType, Guards, Patch, ProjectRef, ScopeFilter,
    SyncCursor,
};

fn assert_send_sync<T: ?Sized + Send + Sync>() {}

#[test]
fn bounded_run_signature_and_error_variants() {
    type RunBounded =
        fn(&str, &ProjectLocks, &str, &[String], Duration) -> Result<CliOutput, BoundedError>;
    let _: RunBounded = run_bounded;
    let errors = [
        BoundedError::TimedOut,
        BoundedError::Failed(BeadsError::Unsupported {
            operation: "pin",
            client: sc_beads::CliClient::Bd,
        }),
    ];
    assert_eq!(errors.len(), 2);
    assert!(std::error::Error::source(&errors[0]).is_none());
    assert!(std::error::Error::source(&errors[1]).is_some());
    assert_send_sync::<BoundedError>();
}

/// `run_bounded` is provided: the five-method implementor of `cli_api_freeze.rs`
/// compiles without it, and the provided body refuses instead of spawning.
#[test]
fn invoker_run_bounded_is_provided_and_refuses() {
    struct Pin;
    impl CliInvoker for Pin {
        fn binary(&self) -> String {
            "pin".to_owned()
        }
        fn client_info(&self) -> Option<sc_beads::CliProbe> {
            None
        }
        fn probe(&self) -> Option<sc_beads::CliProbe> {
            None
        }
        fn run_json(&self, _: &ProjectRef, _: &str, _: &[String]) -> Result<String, BeadsError> {
            Ok(String::new())
        }
        fn run_raw(&self, _: &ProjectRef, _: &[&str]) -> Result<CliOutput, BeadsError> {
            Ok(CliOutput {
                status: Some(0),
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }
    let r = Pin.run_bounded(&ProjectRef::local(None), &[], Duration::from_secs(1));
    assert!(matches!(
        r,
        Err(BoundedError::Failed(BeadsError::Unsupported { .. }))
    ));
}

#[test]
fn bd_cli_constructors_and_the_cache_backend() {
    let _: fn(String, Arc<ProjectLocks>, Arc<dyn Clock>) -> BdCli = BdCli::new_with_clock;
    let _: fn(BdCli, Arc<dyn Clock>) -> BdCli = BdCli::with_clock;
    assert_send_sync::<BdCli>();
    let cli = BdCli::new_with_clock(
        "bd",
        Arc::new(ProjectLocks::new()),
        Arc::new(sc_beads::SystemClock),
    );
    let _: &dyn CacheBackend = &cli;
}

#[test]
fn argv_signatures_and_constants() {
    let _: fn() -> Vec<String> = argv::version;
    let _: fn() -> Vec<String> = argv::schema;
    let _: fn(&[&BeadId]) -> Vec<String> = argv::list_by_ids;
    let _: fn(&ScopeFilter, Option<&SyncCursor>) -> Vec<String> = argv::list_scope;
    let _: fn(&ScopeFilter) -> Vec<String> = argv::blocked;
    let _: fn(&ScopeFilter) -> Vec<String> = argv::ready;
    let _: fn(&BeadId, Option<&str>, &Actor) -> Vec<String> = argv::close;
    let _: fn(&BeadId, &Patch, &Guards, &Actor) -> Vec<String> = argv::update;
    let _: fn(&BeadId, &BeadId, DepType, &Actor) -> Vec<String> = argv::dep_add;
    assert_eq!(
        argv::ALL_STATUSES,
        "open,in_progress,blocked,deferred,closed,pinned,hooked"
    );
    assert_eq!(argv::IDS_PER_CALL, 100);
    assert!(UNREACHABLE_MARKERS.contains(&"connection refused"));
}

#[test]
fn test_doubles() {
    let _: fn(RecordingInvoker, Result<CliOutput, BoundedError>) -> RecordingInvoker =
        RecordingInvoker::reply_bounded;
    let _: fn(&RecordingInvoker) -> Vec<Duration> = RecordingInvoker::timeouts;
    let _: fn(Arc<ManualClock>) -> FakeBd = FakeBd::new;
    let _: fn(FakeBackend) -> FakeBd = FakeBd::over;
    let _: fn(&FakeBd) -> &FakeBackend = FakeBd::backend;
    let fake = FakeBd::new(Arc::new(ManualClock::default()));
    fake.set_schema("{}");
    fake.set_bd_version("9.9.9");
    let _: fn(&FakeBd) -> Vec<Vec<String>> = FakeBd::calls;
    assert_send_sync::<FakeBd>();
    assert_send_sync::<RecordingInvoker>();
}
