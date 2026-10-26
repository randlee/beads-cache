//! The post-sync hook (REQ-BDC-0723, 0727, 0728; ADR-SYNC-0008) through the
//! built-in run, with a stub interpreter (a shell script) in place of
//! `python -I -m bd_cache_rs.run`. Unix: the stub is a shell script. The
//! Windows job-object kill of a hook and of a Python row, and the
//! Ctrl-Break stop of `cron run`, are real-process tests on Windows:
//! `crates/bd-cache-cli/tests/windows.rs`, run by CI's windows-latest job.

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;
use std::time::Duration;

use bd_cache_model::{CacheStore, TickBudget, TickRunReason};
use bd_cache_sync::{BuiltinPayload, EnvNames, HookStep, PostSyncHook};
use common::{HookStub, World, escalate, escalate_with, gate_file, label_scope, run_builtin};
use sc_beads::Timestamp;
use sc_beads::testing::{FakeOp, Injection};

fn with_hook(stub: &HookStub, budget_ms: u64, reserve_ms: u64) -> BuiltinPayload {
    with_hook_env(stub, budget_ms, reserve_ms, EnvNames::default())
}

fn with_hook_env(
    stub: &HookStub,
    budget_ms: u64,
    reserve_ms: u64,
    env: EnvNames,
) -> BuiltinPayload {
    BuiltinPayload {
        budget: TickBudget::new(Some(budget_ms), Some(reserve_ms), true).unwrap(),
        hook: Some(PostSyncHook::new(&stub.python, "consumer.hooks:after_sync", env).unwrap()),
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

/// A request larger than a pipe's buffer (hundreds of KiB) is written while
/// the output is read, whether the hook reads it all or not at all: no
/// deadlock (acceptance 7). Planted negative: the hook that reads it gets
/// every byte the run built.
#[tokio::test(flavor = "multi_thread")]
async fn a_request_larger_than_the_pipe_buffer_is_written_without_a_deadlock() {
    let world = World::new();
    escalate_with(&world, 300, &"e".repeat(2_000));
    let stub = HookStub::new();
    let cache = world.shared(vec![]);
    let payload = with_hook(&stub, 20_000, 5_000);
    let reply = run_builtin(&cache, &payload, &world.ctx()).await.unwrap();
    assert_eq!(reply.hook.status, HookStep::Ok, "{:?}", reply.hook);
    let read = std::fs::metadata(stub.dir.path().join("request.1.json"))
        .unwrap()
        .len();
    assert!(read > 256 * 1024, "the request was {read} bytes");
    stub.mode("noread");
    let reply = tokio::time::timeout(
        Duration::from_secs(30),
        run_builtin(&cache, &payload, &world.ctx()),
    )
    .await
    .expect("no deadlock")
    .unwrap();
    assert_eq!(
        reply.hook.status,
        HookStep::Ok,
        "a hook that never reads its request still answers"
    );
}

/// Escalations whose errors are large are dropped from the end until the
/// whole request fits in 4 MiB, the rest counted (REQ-BDC-0727). Planted
/// negative: small ones keep all 500 (the cap test above).
#[tokio::test(flavor = "multi_thread")]
async fn a_request_over_4_mib_is_trimmed_to_fit() {
    let world = World::new();
    escalate_with(&world, 500, &"x".repeat(10_000));
    let stub = HookStub::new();
    let cache = world.shared(vec![]);
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
        .await
        .unwrap();
    let size = std::fs::metadata(stub.dir.path().join("request.1.json"))
        .unwrap()
        .len();
    let req = stub.request(1);
    let kept = req["escalated"].as_array().unwrap().len();
    assert!(kept < 500 && kept > 0, "kept {kept}");
    assert!(
        size <= bd_cache_sync::MAX_HOOK_REQUEST_BYTES as u64,
        "{size} bytes"
    );
    assert_eq!(req["escalated_remaining"], 500 - kept as u64);
    assert_eq!(req["escalated_truncated"], true);
    assert_eq!(reply.hook.offered, kept);
    // Only what was offered is marked offered.
    let offered = world
        .store
        .unannounced_escalations()
        .unwrap()
        .iter()
        .filter(|e| e.first_offered_at.is_some())
        .count();
    assert_eq!(offered, kept);
}

/// Over 1 MiB on stderr fails the hook as `oversize`, and the reply keeps
/// at most 8 KiB of it (REQ-BDC-0723). Planted negative: stdout's cap is
/// the flood test above; the default stub's stderr passes.
#[tokio::test(flavor = "multi_thread")]
async fn a_hook_that_floods_stderr_fails_as_oversize_and_keeps_8_kib() {
    let world = World::new();
    let stub = HookStub::new();
    stub.mode("errflood");
    let cache = world.shared(vec![]);
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
        .await
        .unwrap();
    let error = reply.hook.error.unwrap();
    assert_eq!(error["details"]["reason"], "oversize");
    let kept = reply.hook.stderr.unwrap();
    assert_eq!(kept.len(), bd_cache_sync::MAX_KEPT_STDERR_BYTES);
}

/// The values of the hook's `--env` variables are redacted from every text
/// the hook wrote that the run keeps: its stderr (cut to 8 KiB), its error
/// message in `hook.error`, and `hook_state.last_error` (REQ-BDC-0724,
/// acceptance 4). Planted negative: with no `--env` name, the value shows.
#[tokio::test(flavor = "multi_thread")]
async fn a_hooks_env_values_are_redacted_from_the_reply_and_the_hook_state() {
    let value = std::env::var("HOME").unwrap();
    let world = World::new();
    let stub = HookStub::new();
    stub.mode("secret");
    let cache = world.shared(vec![]);
    let env = EnvNames::new(["HOME".to_owned()]).unwrap();
    let reply = run_builtin(
        &cache,
        &with_hook_env(&stub, 20_000, 5_000, env),
        &world.ctx(),
    )
    .await
    .unwrap();
    let stderr = reply.hook.stderr.clone().unwrap();
    assert!(stderr.contains("<redacted:HOME>") && !stderr.contains(&value));
    assert_eq!(
        stderr.len(),
        bd_cache_sync::MAX_KEPT_STDERR_BYTES,
        "cut to 8 KiB"
    );
    let error = reply.hook.error.unwrap();
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("<redacted:HOME>"), "{message}");
    assert!(!error.to_string().contains(&value), "{error}");
    let last = world.store.hook_state().unwrap().last_error.unwrap();
    assert!(!last.contains(&value), "{last}");

    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &world.ctx())
        .await
        .unwrap();
    assert!(
        reply.hook.stderr.unwrap().contains(&value),
        "no --env, no redaction"
    );
}

/// A hook killed by the run's kill flag (a signal) fails as `signal`, not
/// as a timeout (B8Q1-034); planted negative: one killed at its deadline is
/// `timeout` (the deadline test above).
#[tokio::test(flavor = "multi_thread")]
async fn a_hook_killed_by_a_signal_is_reported_as_signal() {
    let world = World::new();
    let stub = HookStub::new();
    stub.mode("hang");
    let (ready, life) = (gate_file(stub.fifo("ready")), gate_file(stub.fifo("life")));
    let cache = world.shared(vec![]);
    let payload = with_hook(&stub, 20_000, 10_000);
    let ctx = world.ctx();
    let kill = ctx.kill.clone();
    let run = tokio::spawn(async move { run_builtin(&cache, &payload, &ctx).await });
    tokio::task::spawn_blocking(move || ready.recv_timeout(Duration::from_secs(10)).unwrap())
        .await
        .unwrap();
    kill.stop();
    tokio::task::spawn_blocking(move || life.recv_timeout(Duration::from_secs(20)).unwrap())
        .await
        .unwrap();
    let reply = run.await.unwrap().unwrap();
    assert_eq!(reply.hook.error.unwrap()["details"]["reason"], "signal");
    assert_eq!(world.store.lease().unwrap(), None);
}

/// A run stopped after its request was built and before its hook launched
/// marks nothing offered: `first_offered_at` is set only for a hook that
/// launches (B8Q1-030). Planted negative: the same run unstopped marks them.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_stopped_before_its_hook_marks_nothing_offered() {
    use bd_cache::Cache;
    use common::counting::Counting;
    let world = World::new();
    escalate(&world, 2);
    let stub = HookStub::new();
    let (store, _touched) = Counting::new(world.store.clone());
    let ctx = world.ctx();
    store.stop_on_pending_summary(ctx.stop.clone());
    let cache = std::sync::Arc::new(std::sync::Mutex::new(
        Cache::new(
            bd_cache_model::CacheConfig::new(sc_beads::ProjectRef::local(None), vec![]),
            store,
            world.lock.clone(),
            world.switch.clone(),
            std::sync::Arc::clone(&world.clock) as std::sync::Arc<dyn sc_beads::Clock>,
        )
        .unwrap(),
    ));
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &ctx)
        .await
        .unwrap();
    assert_eq!(reply.hook.status, HookStep::Skipped);
    assert_eq!(stub.calls(), 0);
    let entries = world.store.unannounced_escalations().unwrap();
    assert!(
        entries.iter().all(|e| e.first_offered_at.is_none()),
        "{entries:?}"
    );

    let ctx = world.ctx();
    let reply = run_builtin(&cache, &with_hook(&stub, 20_000, 5_000), &ctx)
        .await
        .unwrap();
    assert_eq!(reply.hook.status, HookStep::Ok);
    let entries = world.store.unannounced_escalations().unwrap();
    assert!(entries.iter().all(|e| e.first_offered_at.is_some()));
}

/// A hook that marked some entries and then crashed keeps those marks; the
/// next run offers only the rest, with `first_offered_at` unchanged
/// (acceptance 8). Planted negative: an unmarked entry is offered again.
#[tokio::test(flavor = "multi_thread")]
async fn a_crash_after_marking_keeps_the_marks_and_re_offers_the_rest() {
    let world = World::new();
    let seqs = escalate(&world, 2);
    let stub = HookStub::new();
    stub.mode("fail");
    let cache = world.shared(vec![]);
    let payload = with_hook(&stub, 20_000, 5_000);
    run_builtin(&cache, &payload, &world.ctx()).await.unwrap();
    let first = stub.request(1)["escalated"][1]["first_offered_at"].clone();
    // What the hook did before it crashed: it marked the first entry.
    cache.lock().unwrap().mark_announced(seqs[0]).unwrap();
    world.clock.advance(Duration::from_secs(5));
    stub.mode("ok");
    run_builtin(&cache, &payload, &world.ctx()).await.unwrap();
    let req = stub.request(2);
    let offered = req["escalated"].as_array().unwrap();
    assert_eq!(offered.len(), 1, "the marked entry is not offered again");
    assert_eq!(offered[0]["seq"], seqs[1].get());
    assert_eq!(offered[0]["first_offered_at"], first, "unchanged");
}
