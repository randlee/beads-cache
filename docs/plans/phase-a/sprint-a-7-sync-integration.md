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
parallel_safe: []
requirements:
  owns: [REQ-BDC-0003, REQ-BDC-0701, REQ-BDC-0702, REQ-BDC-0703, REQ-BDC-0704, REQ-BDC-0705, REQ-BDC-0706, REQ-BDC-0707, REQ-BDC-0903, NFR-BDC-0003]
  constrained_by: [REQ-BDC-0405, REQ-BDC-0503, NFR-BDC-0001, NFR-BDC-0004, NFR-BDC-0006]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0005, ADR-BDC-0006]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "tests/e2e/**"
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

1. `bd-cache-sync` tick (REQ-BDC-0703): 25 s budget, drain with a deadline 5 s
   before the end, then sync each active scope, deferring scopes not
   started 2 s before the end; one schedule per repository with a team
   (REQ-BDC-0701); active phases from `.atm-bd/phase-*.toml` and the
   configured list (REQ-BDC-0702); stop rule (REQ-BDC-0707); scheduled on
   `sc-cron`.
2. Announcements (REQ-BDC-0704..0706): sync failures once per cause,
   escalations once with the `resolve` command, undelivered messages
   refused and retried, through a-1's `EscalationSender`; an implementation
   that runs `atm send <lead>@<team> --team tool` as `cron`.
3. End-to-end tests (`tests/e2e/`): a journaled write applied by a scheduled
   drain through `BdCli` with a recording invoker in CI, and against a
   scratch `bd` workspace on the testbed (REQ-BDC-0003); an outage of
   several ticks (REQ-BDC-0903).
4. Performance on the testbed (NFR-BDC-0003): incremental sync of a
   1000-bead scope.
5. Repository bookkeeping: remove `/Cargo.lock` from `.gitignore` and commit
   the lock; fold `docs/plans/phase-a/notes/a-2.md`, `a-3.md`, `a-4.md` into
   ADR-BDC-0002, 0006, 0001; update Crate Layout to the shipped state.

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
| `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`, through the real store | req:REQ-BDC-0003 |
| A write journaled through the core lands in `bd` via the scheduled drain and is verified by read-back, end to end | req:REQ-BDC-0003 |
| During an outage of several ticks, reads answer with staleness reported, no escalation is sent, and the journal drains on recovery (the attempts rule itself is a-5's) | req:REQ-BDC-0903 |
| Incremental sync of a 1000-bead scope under 1 s on the testbed, recorded | req:NFR-BDC-0003 |
| `Cargo.lock` is committed; `docs/architecture.md` ADRs and Crate Layout match the shipped crates | phase plan |
| Planted bugs: unannounced filter dropped, open-work ignores announcement, refusal dropped, team check dropped, closed roots active, stop ignores open work each fail a test | req:REQ-BDC-0705 |

## Required validation

Full workspace validation; the end-to-end tests; the testbed run recorded.
