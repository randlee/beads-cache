//! The drain over the real `BdCli` mapping (`sc-bd-cli` over its `FakeBd`): a
//! write the CLI layer finds no bead for escalates `NOT_FOUND` (PE-RBP-F013).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{CacheConfig, DrainBudget, IdemKey, JournalState, ScopeId};
use sc_bd_cli::testing::FakeBd;
use sc_bd_cli::{BdCli, BoundedError, CliInvoker};
use sc_beads::error::BeadsError;
use sc_beads::testing::{ManualClock, actor, bead, id};
use sc_beads::{CliOutput, CliProbe, Clock, Deadline, Guards, ProjectRef};

/// `FakeBd`, except that once armed, every `bd list --id` after the first `pass`
/// answers `[]`: the bead vanishes between the drain's pre-read and the guard
/// check of its guarded close.
struct Vanishing {
    bd: FakeBd,
    /// `bd list --id` calls still answered normally; `usize::MAX` = not armed.
    pass: AtomicUsize,
    calls: Mutex<Vec<Vec<String>>>,
}

impl Vanishing {
    fn arm(&self, pass: usize) {
        self.pass.store(pass, Ordering::SeqCst);
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Lets the test keep a handle on the invoker it gives `BdCli`.
struct Shared(Arc<Vanishing>);

impl CliInvoker for Shared {
    fn binary(&self) -> String {
        self.0.bd.binary()
    }
    fn client_info(&self) -> Option<CliProbe> {
        self.0.bd.client_info()
    }
    fn probe(&self) -> Option<CliProbe> {
        self.0.bd.probe()
    }
    fn run_json(
        &self,
        project: &ProjectRef,
        command: &str,
        args: &[String],
    ) -> Result<String, BeadsError> {
        self.0.bd.run_json(project, command, args)
    }
    fn run_raw(&self, project: &ProjectRef, args: &[&str]) -> Result<CliOutput, BeadsError> {
        self.0.bd.run_raw(project, args)
    }
    fn run_bounded(
        &self,
        project: &ProjectRef,
        argv: &[String],
        timeout: Duration,
    ) -> Result<CliOutput, BoundedError> {
        self.0
            .calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(argv.to_vec());
        let by_id = argv.first().is_some_and(|a| a == "list") && argv.iter().any(|a| a == "--id");
        if by_id {
            let left = self.0.pass.load(Ordering::SeqCst);
            if left == 0 {
                return Ok(CliOutput {
                    status: Some(0),
                    success: true,
                    stdout: "[]".into(),
                    stderr: String::new(),
                });
            }
            if left != usize::MAX {
                self.0.pass.store(left - 1, Ordering::SeqCst);
            }
        }
        self.0.bd.run_bounded(project, argv, timeout)
    }
}

fn x() -> ScopeId {
    ScopeId::phase("x").unwrap()
}

#[test]
fn a_guarded_close_of_a_bead_gone_from_bd_escalates_not_found_without_retrying() {
    let clock = Arc::new(ManualClock::new("2026-10-23T10:00:00Z"));
    let bd = FakeBd::new(Arc::clone(&clock));
    let mut b1 = bead("b1");
    b1.labels = vec!["phase-x".into()];
    b1.updated_at = clock.now_utc().into();
    bd.backend().insert(b1);
    let inv = Arc::new(Vanishing {
        bd,
        pass: AtomicUsize::new(usize::MAX),
        calls: Mutex::new(Vec::new()),
    });
    let dyn_clock: Arc<dyn Clock> = Arc::clone(&clock) as Arc<dyn Clock>;
    let cli =
        BdCli::with_invoker(Box::new(Shared(Arc::clone(&inv)))).with_clock(Arc::clone(&dyn_clock));
    let store = FakeStore::new();
    let config = CacheConfig::new(ProjectRef::local(None), vec![x()]);
    let mut c = Cache::new(
        config,
        store.clone(),
        FakeDrainLock::new(),
        cli,
        Arc::clone(&dyn_clock),
    )
    .unwrap();
    c.sync(&x(), Deadline::after(&*clock, Duration::from_secs(30)))
        .unwrap();
    let guards = Guards {
        status: Some("open".into()),
        assignee: None,
    };
    let seq = c
        .write(
            &id("b1"),
            "close",
            "{}",
            guards,
            IdemKey::new("k1").unwrap(),
            actor("san"),
        )
        .unwrap()
        .seq;

    // The drain's pre-read still sees b1; the guard check of `bd close` does not.
    inv.arm(1);
    let before = inv.calls().len();
    let budget = DrainBudget {
        start_gate: Deadline::after(&*clock, Duration::from_secs(20)),
        hard: Deadline::after(&*clock, Duration::from_secs(23)),
    };
    let r = c.drain(&budget).unwrap();

    assert_eq!(r.escalated.len(), 1, "{r:?}");
    assert_eq!(r.escalated[0].seq, seq);
    assert!(
        r.escalated[0]
            .reason
            .starts_with("BD_CACHE.NOT_FOUND: bead b1"),
        "{}",
        r.escalated[0].reason
    );
    assert!(r.retrying.is_empty() && r.applied.is_empty(), "{r:?}");
    let entry = store.journal().into_iter().find(|e| e.seq == seq).unwrap();
    assert_eq!(entry.state, JournalState::Escalated);
    assert_eq!(entry.attempts, 0, "a missing bead is no transient failure");
    let drained: Vec<_> = inv.calls().into_iter().skip(before).collect();
    assert!(
        !drained
            .iter()
            .any(|a| a.first().is_some_and(|w| w == "close")),
        "bd close never ran: {drained:?}"
    );
}
