//! The post-sync hook (REQ-BDC-0723, 0727, 0728; ADR-SYNC-0008) through the
//! built-in run, with a stub interpreter (a shell script) in place of
//! `python -I -m bd_cache_rs.run`. Unix: the stub is a shell script; the
//! Windows job-object kill is sc-cron's (`crates/sc-cron/tests/process_api.rs`
//! compiles it for Windows) and `windows_job_object` below.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::time::Duration;

use bd_cache_model::{CacheStore, TickBudget, TickRunReason};
use bd_cache_sync::{BuiltinPayload, EnvNames, HookStep, PostSyncHook, run_builtin};
use common::{HookStub, World, escalate, gate_file, label_scope};
use sc_beads::Timestamp;
use sc_beads::testing::{FakeOp, Injection};

fn with_hook(stub: &HookStub, budget_ms: u64, reserve_ms: u64) -> BuiltinPayload {
    BuiltinPayload {
        budget: TickBudget::new(Some(budget_ms), Some(reserve_ms), true).unwrap(),
        hook: Some(
            PostSyncHook::new(
                &stub.python,
                "consumer.hooks:after_sync",
                EnvNames::default(),
            )
            .unwrap(),
        ),
    }
}

/// The request shape (`bd-cache.run.v1`), the command, the environment
/// allowlist and a private temp cwd (REQ-BDC-0723, 0727).
#[tokio::test(flavor = "multi_thread")]
async fn the_hook_gets_every_result_in_one_request_from_an_isolated_child() {
    let world = World::new();
    world.add("b1", &["area"], |_| {});
    let seqs = escalate(&world, 2);
    let stub = HookStub::new();
    let cache = world.shared(vec![label_scope("lbl", "area")]);
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
        .await
        .unwrap();
    assert_eq!(reply.hook.status, HookStep::Ok, "{:?}", reply.hook);
    assert_eq!(reply.hook.result.as_ref().unwrap()["handled"], true);
    let req = stub.request(1);
    assert_eq!(req["protocol"], "bd-cache.run.v1");
    assert_eq!(req["kind"], "post_sync");
    assert_eq!(req["run_id"], reply.run_id.as_str());
    assert_eq!(req["scopes"][0]["scope"], "lbl");
    assert_eq!(req["scopes"][0]["result"], "ok");
    assert_eq!(req["escalated"].as_array().unwrap().len(), 2);
    assert_eq!(req["escalated"][0]["seq"], seqs[0].get());
    assert!(req["escalated"][0]["first_offered_at"].is_string());
    assert_eq!(
        (
            req["escalated_truncated"].clone(),
            req["escalated_remaining"].clone()
        ),
        (false.into(), 0.into())
    );
    assert!(req["pending"]["count"].is_u64());
    assert_eq!(
        stub.read("argv").lines().collect::<Vec<_>>(),
        [
            "-I",
            "-m",
            "bd_cache_rs.run",
            "post_sync",
            "consumer.hooks:after_sync"
        ]
    );
    let env = stub.read("env");
    let names: std::collections::BTreeSet<&str> =
        env.lines().filter_map(|l| l.split('=').next()).collect();
    let allowed = [
        "PATH",
        "HOME",
        "LANG",
        "BEADS_DIR",
        "BD_CACHE_PATH",
        "PWD",
        "SHLVL",
        "_",
        "OLDPWD",
    ];
    for name in &names {
        assert!(
            allowed.contains(name) || name.starts_with("LC_"),
            "{name} leaked into the hook"
        );
    }
    assert!(names.contains("BD_CACHE_PATH") && names.contains("BEADS_DIR"));
    let cwd = stub.read("cwd");
    assert!(cwd.contains("sc-cron-child-"), "a private temp cwd: {cwd}");
    assert!(
        !std::path::Path::new(cwd.trim()).exists(),
        "removed after the call"
    );
    // The run marks nothing: the entries are still unannounced, and offered once.
    let left = world.store.unannounced_escalations().unwrap();
    assert_eq!(left.len(), 2);
    assert!(left.iter().all(|e| e.first_offered_at.is_some()));
}

/// When the sync uses everything, the hook still gets `hook_reserve_ms`:
/// the scope list is cut at the sync deadline and the request's deadline is
/// a full reserve after its `now` (REQ-BDC-0703 as amended).
#[tokio::test(flavor = "multi_thread")]
async fn a_sync_that_uses_its_whole_share_leaves_the_hook_its_reserve() {
    let world = World::new();
    world.add("b1", &["area"], |_| {});
    world.bd.inject(
        FakeOp::ListScope,
        Injection::Delay(Duration::from_secs(3600)),
    );
    let stub = HookStub::new();
    let cache = world.shared(vec![label_scope("lbl", "area")]);
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
        .await
        .unwrap();
    assert_eq!(reply.hook.status, HookStep::Ok);
    let req = stub.request(1);
    let now = Timestamp::parse(req["now"].as_str().unwrap()).unwrap();
    let deadline = Timestamp::parse(req["deadline"].as_str().unwrap()).unwrap();
    assert_eq!(
        deadline.unix() - now.unix(),
        5,
        "the reserve is the hook's: {req}"
    );
}

/// A hook that outlives its deadline is killed with everything it started
/// (causal: the grandchild's pipe closes), the run finishes and the lease is
/// free; the failure is `TICK_RUN_FAILED` (`timeout`) in `hook.error` while
/// the run itself is ok.
#[tokio::test(flavor = "multi_thread")]
async fn a_hook_past_its_deadline_is_killed_with_its_tree_and_the_run_finishes() {
    let world = World::new();
    let stub = HookStub::new();
    stub.mode("hang");
    let (ready, life) = (gate_file(stub.fifo("ready")), gate_file(stub.fifo("life")));
    let cache = world.shared(vec![]);
    let payload = with_hook(&stub, 2_000, 1_000);
    let ctx = world.ctx();
    let run = tokio::spawn(async move { run_builtin(&cache, &payload, &ctx).await });
    tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(10)).unwrap())
        .await
        .unwrap();
    tokio::task::spawn_blocking(move || life.recv_timeout(Duration::from_secs(20)).unwrap())
        .await
        .unwrap();
    let reply = run.await.unwrap().unwrap();
    assert_eq!(reply.hook.status, HookStep::Failed);
    let error = reply.hook.error.unwrap();
    assert_eq!(error["code"], "BD_CACHE.TICK_RUN_FAILED");
    assert_eq!(error["details"]["reason"], "timeout");
    assert_eq!(world.store.lease().unwrap(), None);
}

/// Output over 1 MiB on a pipe fails the hook (`oversize`); planted
/// negative: the default stub passes (first test).
#[tokio::test(flavor = "multi_thread")]
async fn a_hook_that_floods_a_pipe_fails_as_oversize() {
    let world = World::new();
    let stub = HookStub::new();
    stub.mode("flood");
    let cache = world.shared(vec![]);
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
        .await
        .unwrap();
    assert_eq!(reply.hook.error.unwrap()["details"]["reason"], "oversize");
}

/// A cache file that is group-writable runs no hook (REQ-BDC-0723).
#[tokio::test(flavor = "multi_thread")]
async fn an_untrusted_cache_file_runs_no_hook() {
    let world = World::new();
    let stub = HookStub::new();
    let ctx = world.ctx();
    std::fs::set_permissions(&ctx.cache_path, std::fs::Permissions::from_mode(0o664)).unwrap();
    let cache = world.shared(vec![]);
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &ctx)
        .await
        .unwrap();
    assert_eq!(stub.calls(), 0, "nothing ran");
    let error = reply.hook.error.unwrap();
    assert_eq!(
        error["details"]["reason"],
        TickRunReason::Untrusted.as_str()
    );
    // Planted negative: private again, the hook runs.
    std::fs::set_permissions(&ctx.cache_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &ctx)
        .await
        .unwrap();
    assert_eq!((reply.hook.status, stub.calls()), (HookStep::Ok, 1));
}

/// A wrong protocol and a non-zero exit are each `TICK_RUN_FAILED`.
#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_protocol_and_a_failing_exit_are_each_a_failed_hook() {
    let world = World::new();
    let stub = HookStub::new();
    let cache = world.shared(vec![]);
    for mode in ["wrong", "fail"] {
        stub.mode(mode);
        let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
            .await
            .unwrap();
        assert_eq!(reply.hook.status, HookStep::Failed, "{mode}");
        assert_eq!(
            reply.hook.error.as_ref().unwrap()["code"],
            "BD_CACHE.TICK_RUN_FAILED"
        );
    }
    assert!(stub_stderr_kept(&world));
}

fn stub_stderr_kept(world: &World) -> bool {
    world
        .store
        .hook_state()
        .unwrap()
        .last_error
        .is_some_and(|e| e.contains("boom"))
}

/// Anything the hook does not mark is offered again on a later run with
/// `first_offered_at` unchanged; from the third consecutive failure the hook
/// is backed off, and the first success clears the count (REQ-BDC-0727).
#[tokio::test(flavor = "multi_thread")]
async fn unmarked_entries_are_offered_again_and_three_failures_start_the_backoff() {
    let world = World::new();
    escalate(&world, 1);
    let stub = HookStub::new();
    stub.mode("fail");
    let cache = world.shared(vec![]);
    let mut ctx = world.ctx();
    ctx.period = Some(Duration::from_secs(60));
    let payload = with_hook(&stub, 20_000, 5_000);
    for _ in 0..3 {
        let reply = run_builtin(&cache, &payload, &ctx).await.unwrap();
        assert_eq!(reply.hook.status, HookStep::Failed);
        world.clock.advance(Duration::from_secs(1));
    }
    let first = stub.request(1)["escalated"][0]["first_offered_at"].clone();
    assert_eq!(
        stub.request(3)["escalated"][0]["first_offered_at"],
        first,
        "unchanged"
    );
    let state = world.store.hook_state().unwrap();
    assert_eq!(state.consecutive_failures, 3);
    assert!(state.next_attempt_at.is_some(), "backed off from the third");
    let reply = run_builtin(&cache, &payload, &ctx).await.unwrap();
    assert_eq!(reply.hook.status, HookStep::BackedOff);
    assert_eq!(stub.calls(), 3, "not called while backed off");
    world.clock.advance(Duration::from_secs(61));
    stub.mode("ok");
    let reply = run_builtin(&cache, &payload, &ctx).await.unwrap();
    assert_eq!(reply.hook.status, HookStep::Ok);
    assert_eq!(
        world.store.hook_state().unwrap().consecutive_failures,
        0,
        "a success clears it"
    );
}

/// A request holds at most 500 escalations and fits in 4 MiB; the rest are
/// counted (REQ-BDC-0727).
#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_offer_is_capped_at_500_escalations() {
    let world = World::new();
    escalate(&world, 520);
    let stub = HookStub::new();
    let cache = world.shared(vec![]);
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
        .await
        .unwrap();
    let req = stub.request(1);
    assert_eq!(req["escalated"].as_array().unwrap().len(), 500);
    assert_eq!(
        (
            req["escalated_truncated"].clone(),
            req["escalated_remaining"].clone()
        ),
        (true.into(), 20.into())
    );
    assert_eq!(reply.hook.offered, 500);
    assert!(serde_json::to_vec(&req).unwrap().len() <= bd_cache_sync::MAX_HOOK_REQUEST_BYTES);
}
