---
id: a-7
phase: a
status: planned
closure_type: integration
target_boundary: bd-cache-sync and phase composition
vertical_rationale: "The bd-cache-sync boundary does not exist yet: this sprint creates it, then composes it with sc-bd-cli, sc-cron, bd-cache-store and bd-cache. Its only contract (EscalationSender) is already frozen in a-1."
track: integration
wave: 3
branch: sprint/a-7-sync-integration
worktree: sprint/a-7-sync-integration
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [a-2, a-3, a-4, a-5, a-6]
parallel_safe: [a-8]
requirements:
  owns: [REQ-BDC-0003, REQ-BDC-0701, REQ-BDC-0702, REQ-BDC-0703, REQ-BDC-0704, REQ-BDC-0705, REQ-BDC-0706, REQ-BDC-0707, REQ-BDC-0903, NFR-BDC-0001, NFR-BDC-0003]
  constrained_by: [REQ-BDC-0405, REQ-BDC-0503, REQ-BDC-0904, NFR-BDC-0004, NFR-BDC-0006]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0005, ADR-BDC-0006]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache-sync/benches/**"
  - "crates/bd-cache-sync/tests/**"
  - ".claude/project/quality-policy.md"

  - "docs/bd-cache-sync/**"
  - "docs/plans/phase-a/evidence/**"
  - "Cargo.lock"
  - ".gitignore"
  - "docs/architecture.md"
---

# Sprint a-7 — sync task and phase integration

## Outcome and closure

Create `bd-cache-sync`, compose it with the real `sc-bd-cli`, `sc-cron`,
`bd-cache-store` and `bd-cache`, and close every feature-level criterion of
phase A. Closes when the cron cases pass, the end-to-end flow passes through
the real composition, and `docs/architecture.md` matches what shipped.

## must_follow rationale

Composes `BdCli` (a-2), the scheduler (a-3), `SqliteStore` (a-4) and the
write and read paths (a-5, a-6).

## Deliverables

1. `bd-cache-sync` tick (REQ-BDC-0703): 25 s budget ending at T; drain with
   `DrainBudget { start_gate: T - 5 s, hard: T - 2 s }`, then sync each
   active scope with deadline T, deferring scopes not started by T - 2 s;
   sc-cron's run `Instant` becomes T through `Deadline::at`; one schedule per repository with a team and a required `fallback_lead`
   for escalations whose entry has no scope (REQ-BDC-0701); active phases from `.atm-bd/phase-*.toml` and the
   configured list (REQ-BDC-0702); stop rule (REQ-BDC-0707); scheduled on `sc-cron`. Threading per ADR-CORE-0001: the `Cache` is held in `Arc<Mutex<_>>` and
   called under `spawn_blocking` with the tick deadline; a tick that finds
   the mutex held (including a blocking task left over from a timed-out
   tick) is skipped and reported busy; `should_stop` returns Continue when
   busy; a poisoned mutex is recovered and reported in the tick reply.
2. Announcements (REQ-BDC-0704..0706): sync failures once per cause,
   escalations once with the `resolve` command, undelivered messages
   refused and retried, through a-1's `EscalationSender`; an implementation
   that runs `atm send <lead>@<team> --team tool` as `cron`. An outage is announced through the per-scope sync alert with the cause
   key `backend_unavailable`, so each active scope's lead hears it once;
   when it ends the alert is reset and no message is sent; a failed send is
   refused and retried like any announcement (REQ-BDC-0704, 0706). The tick is a library entry point,
   `tick(&mut Cache, &TickConfig, &dyn EscalationSender, end: Deadline)`, so it
   can be driven by `sc-cron` here or by an external scheduler such as the
   ATM cron module (through the later `bdc tick` or Python bindings); only
   one ticker may run per cache file.
3. End-to-end tests (`crates/bd-cache-sync/tests/`): a journaled write applied by a scheduled
   drain through `BdCli` with a recording invoker in CI, and against a
   scratch `bd` workspace on the testbed (REQ-BDC-0003); an outage of
   several ticks (REQ-BDC-0903).
4. Performance: in-process cached `get` on `SqliteStore`, asserted by a
   test on the median of N calls (NFR-BDC-0001; a criterion bench in
   `crates/bd-cache-sync/benches/` is informational); incremental sync of a 1000-bead scope on the testbed
   (NFR-BDC-0003).
5. Testbed evidence (team-lead on `m5-atmbench`): the a-2 live probes, the
   end-to-end run against scratch `bd`, and the sync timing, recorded in
   `docs/plans/phase-a/evidence/`. If the testbed is unavailable, the
   recording-invoker results close the sprint and the testbed items are
   recorded as deferred evidence with an owner and date.
6. Repository bookkeeping: remove `/Cargo.lock` from `.gitignore` and commit
   the lock; update ADR-BDC statuses and Crate Layout in `docs/architecture.md` from
   the crate docs; add the phase-end validation command to the Validation
   Policy in `.claude/project/quality-policy.md`; `docs/bd-cache-sync/`
   requirements and architecture.

## Acceptance criteria

| Criterion | Root |
|---|---|
| `test_tick_syncs_only_phases_with_an_open_root`, `test_custom_phases_limits_sync_and_stop`, `test_malformed_custom_phases_is_refused` | req:REQ-BDC-0702 |
| `test_missing_team_is_refused` | req:REQ-BDC-0701 |
| `test_tick_budget_defers_drain_entries`; no tick exceeds 25 s with every `bd` call timing out | req:REQ-BDC-0703 |
| `test_sync_failure_is_announced_once_per_cause` | req:REQ-BDC-0704 |
| `test_escalation_is_sent_once_to_the_phase_lead` | req:REQ-BDC-0705 |
| `test_undelivered_escalation_is_retried_and_the_tick_refuses` | req:REQ-BDC-0706 |
| `test_stop_only_when_no_active_phase_and_no_open_work` | req:REQ-BDC-0707 |
| `test_backend_outage_is_announced_once_and_sync_failures_are_not`, `test_outage_with_uncached_roots_still_tells_every_configured_lead`, `test_outage_is_not_cleared_while_only_the_backend_check_answers`, `test_undelivered_outage_announcement_is_retried` | req:REQ-BDC-0903 |
| `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`, through the real store | req:REQ-BDC-0003 |
| A write journaled through the core lands in `bd` via the scheduled drain and is verified by read-back, end to end | req:REQ-BDC-0003 |
| During an outage of several ticks, reads of cached beads answer with
`outage` set in `GetReply`/`ListReply`, no escalation is sent, and the journal drains on recovery (the attempts rule itself is a-5's) | req:REQ-BDC-0903 |
| Median in-process cached `get` on `SqliteStore` under 1 ms over N calls (test) | req:NFR-BDC-0001 |
| Incremental sync of a 1000-bead scope under 1 s on the testbed, recorded (or deferred evidence per deliverable 5) | req:NFR-BDC-0003 |
| A second tick while a blocking sync is still running is skipped and reported busy | req:REQ-BDC-0703 |
| An outage is announced once per scope with a lead under `backend_unavailable` and reset when it ends; a scope without a lead uses `fallback_lead` | req:REQ-BDC-0704 |
| `test_e2e_blocker_close_and_reopen_update_ready`: a journaled close of a cached blocker, applied by a scheduled drain on the real store, makes its depender ready in the same tick's sync; reopen makes it blocked again; the same for a blocker outside every scope | req:REQ-BDC-0904 |
| An escalation on an entry with no scope goes to `fallback_lead`, and the stop rule still fires | req:REQ-BDC-0706 |
| `Cargo.lock` is committed; `docs/architecture.md` ADRs and Crate Layout match the shipped crates | phase plan |
| Planted bugs: unannounced filter dropped, open-work ignores announcement, refusal dropped, team check dropped, closed roots active, stop ignores open work each fail a test | req:REQ-BDC-0705 |

## Required validation

Full workspace validation; the end-to-end tests; the testbed run recorded.
