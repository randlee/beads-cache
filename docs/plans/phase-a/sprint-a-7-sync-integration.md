---
id: a-7
phase: a
status: in-review
closure_type: integration
target_boundary: bd-cache-sync and phase composition
vertical_rationale: "The bd-cache-sync boundary does not exist yet: this sprint creates it, then composes it with sc-bd-cli, sc-cron, bd-cache-store and bd-cache. Its only contract (EscalationSender) was defined in a-1 and is amended by the ADR-MODEL-0003 a-7 amendment."
track: integration
wave: 3
branch: sprint/a-7-sync-integration
worktree: sprint/a-7-sync-integration
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [a-2, a-3, a-4, a-5, a-6]
parallel_safe: [a-8]
requirements:
  owns: [REQ-BDC-0003, REQ-BDC-0701, REQ-BDC-0702, REQ-BDC-0703, REQ-BDC-0704, REQ-BDC-0705, REQ-BDC-0706, REQ-BDC-0707, REQ-BDC-0903, NFR-BDC-0001, NFR-BDC-0003, REQ-SYNC-0001, REQ-SYNC-0002, REQ-SYNC-0003, REQ-SYNC-0004, REQ-SYNC-0005, REQ-SYNC-0006, REQ-SYNC-0007, REQ-SYNC-0008, REQ-SYNC-0009, REQ-SYNC-0010, REQ-SYNC-0011, REQ-SYNC-0012, REQ-SYNC-0013, REQ-STORE-0008]
  constrained_by: [REQ-BDC-0006, REQ-BDC-0101, REQ-BDC-0105, REQ-BDC-0201, REQ-BDC-0401, REQ-BDC-0402, REQ-BDC-0403, REQ-BDC-0405, REQ-BDC-0406, REQ-BDC-0501, REQ-BDC-0503, REQ-BDC-0504, REQ-BDC-0601, REQ-BDC-0604, REQ-BDC-0902, REQ-BDC-0904, REQ-BDC-0905, REQ-BDC-0911, REQ-BDC-0912, NFR-BDC-0004, NFR-BDC-0005, NFR-BDC-0006, NFR-BDC-0007, REQ-CORE-0001, REQ-CORE-0002, REQ-CORE-0003, REQ-CORE-0004, REQ-STORE-0001, REQ-STORE-0002, REQ-STORE-0003, REQ-STORE-0004, REQ-STORE-0005, REQ-STORE-0006, REQ-STORE-0007, REQ-MODEL-0001, REQ-MODEL-0002, REQ-MODEL-0003, REQ-MODEL-0004, REQ-MODEL-0005]
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0007, ADR-CORE-0001, ADR-CORE-0002, ADR-CORE-0101, ADR-CORE-0102, ADR-CORE-0103, ADR-CORE-0201, ADR-CORE-0202, ADR-CORE-0203, ADR-CORE-0204, ADR-CORE-0205, ADR-CORE-0206, ADR-CRON-0005, ADR-MODEL-0001, ADR-MODEL-0002, ADR-MODEL-0003, ADR-MODEL-0004, ADR-STORE-0001, ADR-STORE-0002, ADR-STORE-0003, ADR-STORE-0004, ADR-SYNC-0001, ADR-SYNC-0002, ADR-SYNC-0003, ADR-SYNC-0004, ADR-SYNC-0005, ADR-SYNC-0006, ADR-SYNC-0007]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "crates/bd-cache-sync/benches/**"
  - "crates/bd-cache-sync/tests/**"
  - ".claude/project/quality-policy.md"

  - "docs/bd-cache-sync/**"
  - "docs/plans/phase-a/evidence/**"
  - "docs/architecture.md"
  - "crates/bd-cache-store/src/lib.rs"  # S4/S14: module docs and re-exports (fix-a-7-3)
  - "crates/bd-cache-store/src/error.rs"  # S14/S18: error text built without ignored write! results (RBP-F029)
  - "crates/bd-cache-model/src/op.rs"  # S14/S18: CLOSED_STATUS constant (RBP-F017)
  - "crates/bd-cache-model/src/pyjson.rs"  # S14/S18: shared formatting fix (RBP-F029)
  - "crates/bd-cache-model/src/testing/store.rs"  # S14/S18: shared contract store (RBP-F029)
  - "crates/bd-cache-store/tests/open.rs"  # S14
  - "crates/bd-cache-store/tests/store.rs"  # S14
  - "crates/bd-cache-model/src/lib.rs"  # S14: re-exports (a-8 also edits it; whoever merges second resolves)
  - "docs/bd-cache/architecture.md"  # S14
  - "docs/bd-cache-model/architecture.md"  # S14: ADR-MODEL-0003 a-7 amendment
  - "docs/bd-cache-store/requirements.md"  # S14 (fix-a-7-2)
  - "docs/bd-cache-store/architecture.md"  # ADR-STORE-0004
  - "docs/bd-cache/read-path.md"  # ADR-CORE-0206
  - "docs/project-plan.md"  # S12
  - "docs/plans/phase-a/phase-a-plan.md"  # S6/S7
  - "docs/requirements.md"  # S3/S5 amendments
  - "Cargo.toml"  # S7: workspace dependencies; S14/S18 fix-a-7-3 adds a new workspace dependency (libc, for O_NONBLOCK in phases.rs, RSH-024)
  - "crates/bd-cache-store/src/{store,schema,codec}.rs"  # S4 amendment
  - "crates/bd-cache-model/src/testing/contract.rs"
  - "crates/bd-cache-model/src/{sender,state,store}.rs"  # S4/S11 amendment
  - "crates/bd-cache/src/ready.rs"
  - "crates/bd-cache/src/get.rs"
  - "crates/bd-cache/src/lib.rs"  # S2/S4 amendment: Cache::clock, get fast path
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
   ATM cron module through the later `bdc tick` (the Python path composes
   `drain`, `sync` and the announcement verbs instead); only
   one ticker may run per cache file. The tick reads its keys from
   `RepoConfig.tick` via a-4's `load_repo_config`.
3. End-to-end tests (`crates/bd-cache-sync/tests/`): a journaled write applied by a scheduled
   drain through `BdCli` with a recording invoker in CI, and against a
   scratch `bd` workspace on the testbed (REQ-BDC-0003); an outage of
   several ticks (REQ-BDC-0903).
4. Performance (amended by lead ruling S13, bc-lead, 2026-10-24): the CI gate for
   NFR-BDC-0001 is deterministic, not a wall-clock median: (a) the number of store
   reads a cached `get` makes is the same at 10, 100 and 1000 beads, and (b) it is
   the same with thousands of applied entries older than the base, with an
   `EXPLAIN QUERY PLAN` assertion that the journal check uses an index. Release-profile
   medians are recorded below as information, and the criterion bench in
   `crates/bd-cache-sync/benches/` prints the same; no wall-clock assertion runs in CI.
   Incremental sync of a 1000-bead scope on the testbed (NFR-BDC-0003).
5. Testbed evidence (team-lead on `m5-atmbench`): the a-2 live probes, the
   end-to-end run against scratch `bd`, and the sync timing, recorded in
   `docs/plans/phase-a/evidence/`. If the testbed is unavailable, the
   recording-invoker results close the sprint and the testbed items are
   recorded as deferred evidence with an owner and date.
6. Repository bookkeeping: update ADR-BDC statuses and Crate Layout in `docs/architecture.md` from
   the crate docs; add the phase-end validation command to the Validation
   Policy in `.claude/project/quality-policy.md`; `docs/bd-cache-sync/`
   requirements and architecture.

- **bd-compat workflow, real run (deferred from a-2, lead ruling bc-lead 2026-10-24).** After develop→main at phase close-out, run `.github/workflows/bd-compat.yml` once by `workflow_dispatch` at the pinned bd version (expect pass) and once by `repository_dispatch` with a different version (expect a failure that opens one issue with the `bd schema` diff). a-2's proof-branch runs 40073075092 and 40073178515 are its interim evidence.

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
| A cached `get` on `SqliteStore` reads the same rows at 10, 100 and 1000 beads and with thousands of old applied entries (index plan asserted); release medians under 1 ms recorded (amended by lead ruling S13, bc-lead, 2026-10-24) | req:NFR-BDC-0001 |
| Incremental sync of a 1000-bead scope under 1 s on the testbed, recorded (or deferred evidence per deliverable 5) | req:NFR-BDC-0003 |
| A second tick while a blocking sync is still running is skipped and reported busy | req:REQ-BDC-0703 |
| An outage is announced once per scope with a lead under `backend_unavailable` and reset when it ends; a scope without a lead uses `fallback_lead` | req:REQ-BDC-0704 |
| `test_e2e_blocker_close_and_reopen_update_ready`: a journaled close of a cached blocker, applied by a scheduled drain on the real store, makes its depender ready in the same tick's sync; reopen makes it blocked again; the same for a blocker outside every scope | req:REQ-SYNC-0010 (constrained by REQ-BDC-0904, which a-6 owns; lead ruling S6) |
| An escalation on an entry with no scope goes to `fallback_lead`, and the stop rule still fires | req:REQ-BDC-0706 |
| `docs/architecture.md` ADRs and Crate Layout match the shipped crates | phase plan |
| Planted bugs: unannounced filter dropped, open-work ignores announcement, refusal dropped, team check dropped, closed roots active, stop ignores open work each fail a test | req:REQ-BDC-0705 |

## Required validation

Full workspace validation; the end-to-end tests; the testbed run recorded.

## Requirement scope notes

- **REQ-BDC-0405, 0503** (read-back, stale re-read; a-5/a-6): run through the real store and `BdCli` by
  `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync` (`tests/e2e.rs`).
- **REQ-BDC-0904, 0911, 0912** (blocked state and `ready`; a-6 owns them): a-7 adds
  `test_e2e_blocker_close_and_reopen_update_ready` on the real store, rooted in REQ-SYNC-0010; 0904 stays in
  `constrained_by` (lead ruling S6).
- **REQ-BDC-0006, 0105, 0201, 0403, 0406, 0501, 0504, 0601, 0902** (a-2, a-4, a-5, a-6 rules the tick and the e2e
  tests run through: the CLI backend, the rebuild rule, the cache-miss fill, the pre-read, the drain report, the
  incremental sync, the cursor, resolve/prune bookkeeping, scope listing): exercised by the end-to-end tests, owned
  elsewhere.
- **REQ-BDC-0101, 0401, 0402, 0604** (cache location, drain lock, drain budget, escalation bookkeeping):
  `compose` uses the first two; the tick passes the third; the sender loop uses the fourth.
- **REQ-BDC-0905** (bd release compatibility): a-2 owns the workflow; a-7 owns the evidence of its real runs
  (`docs/plans/phase-a/evidence/a-7-bd-compat.md`; phase-a-plan ownership table).
- **NFR-BDC-0002** (CLI `get` latency, new-process): deferred with the `bdc` binary (phase plan, "deferred").
- **NFR-BDC-0004, 0005, 0006, 0007**: `no_tick_exceeds_25_s_with_every_bd_call_timing_out`; the planted bugs below; the
  live tests run only on the testbed; the store refuses a second connection to the cache file.
- **REQ-SYNC-0001..0013, ADR-SYNC-0001..0007**: this crate's own ids (`docs/bd-cache-sync/`), **Approved by lead
  ruling S1 (bc-lead, 2026-10-24)** with this round's corrections.

## Rulings applied in fix-a-7-1 (QA round 1)

Lead rulings of bc-lead, 2026-10-24: **S1** crate ids approved, ADR-SYNC-0006 (dependencies) and 0007 (phase-file
schema and `atm send`) added; **S2** `Cache::clock()` and the four-parameter `tick` restored (Deviation 1 is closed);
**S3** `fallback_lead` (Deviation 3) approved, REQ-BDC-0701/0704/0706 amended; **S4** a-6 amendment so a cached `get`
is independent of scope size (ADR-CORE-0206, ADR-STORE-0004); **S5** the stop rule's narrowing (Deviation 4) approved,
REQ-BDC-0707/0702 amended; **S6** ownership reconciled; **S7** workspace dependencies; **S8** testbed items; **S9** the
bd-compat close-out runs are phase close-out gates owned by bc-lead; **S10** phase-file format confirmed against
`bd_cache_cron.py` at atm-core `05831df`; **S11** `send_within` and `SendError` kinds; **S12** project-plan row.

## Rulings applied in fix-a-7-2 (QA round 2)

Lead rulings of bc-lead, 2026-10-24: **S13** the NFR-BDC-0001 CI gate is deterministic (store reads
independent of scope size, and of journal length with an index-plan assertion); wall-clock medians are
information (deliverable 4 and its acceptance row are amended above); **S14** the owned_paths list gains
the store, model and core docs, `crates/bd-cache-model/src/lib.rs` and two store test files; **S15**
`EscalationSender` keeps `&str` for lead and team (callers pass validated names; `AtmSender` keeps a
commented guard); **S16** the testbed is due 2026-10-31, owner bc-lead; **S17** the tick checks its budget
after every send and stops sending once it is spent (the rest is counted in `unsent`).

## Rulings applied in fix-a-7-4 and fix-a-7-5 (QA rounds 4 and 5)

Lead rulings of bc-lead, 2026-10-24: **S22** the `libc` dependency (ADR-SYNC-0006) and the atm output-reading
amendment (ADR-SYNC-0007) are Approved; **S23** a root that bd does not return is unknown: neither open nor
unreadable, reported with the unreadable roots, and the stop rule answers Continue (ADR-SYNC-0004); **S24** the
stop flag is checked before the root refresh (extends the S20 note), and a cached root is trusted only when the
phase's own scope has synced (ADR-SYNC-0005 note); **S25** the journal reads date the entries `bd_cache.py` applied
while the store is open, best effort, with no busy wait (ADR-STORE-0004); **S26** that dating runs only above
`READ_DATING_THRESHOLD` (2000) undated entries and never waits for the write lock; ADR-STORE-0004 Approved, an
exception note in ADR-STORE-0002's Concurrency bullet, and the port docs allow a write that neither waits nor changes
the answer; **S27** the tree kill and the reap of a send's child share one `KILL_BOUND` (1 s) deadline, and
`SEND_RESERVE` is 1.5 s (REQ-SYNC-0011, ADR-SYNC-0007); **S28** one detached reaper thread takes a child or
helper that was given up on, so none stays a zombie for the host's life; **S29** the compile-time check of
`next_sync_cause` enforces the declaration order and the hand-kept count is gone; **S30** `Cache::scope_synced`
reads one `sync_state` row through the port, and `active.rs` uses it (ADR-SYNC-0005).

## Deviations from this document

1. **Phase file format** (closed by S10): confirmed against `bd_cache_cron.py` at atm-core `05831df` and two real
   phase files ported as fixtures. Deliberate differences from the reference are listed in ADR-SYNC-0007 (a file
   without `lead` is kept; an unreadable directory is refused and never stops the schedule; an empty phase list is
   refused). The `atm send` invocation was wrong in round 0 and now matches the reference.
2. **Fallback lead** (S3): an *alert about the backend* for a phase without a lead goes to `fallback_lead`; an
   *escalated entry* whose phase has no lead is refused; an entry with no scope goes to `fallback_lead`.
3. **The stop rule requires positive knowledge** (S5; ADR-SYNC-0004).
4. **Sprint status.** The cursor task's gate asks for `status: complete`; the wave-2 lead notes ask for
   `in-review` at hand-off. This doc says `in-review`; the lead sets `complete` at merge.
5. **Edits outside the sprint's `owned_paths`**, each by a lead ruling named in the frontmatter comments:
   `crates/bd-cache` (S2, S4), `crates/bd-cache-model` and `crates/bd-cache-store` (S4, S11), `Cargo.toml` (S7),
   `docs/requirements.md` (S3, S5), the phase plan (S6, S7), `docs/project-plan.md` (S12).
6. **`SendError` gained a `kind` field** (S11), so the struct literal `SendError { detail }` no longer compiles; the
   constructors `SendError::new` and `with_kind` replace it. No other crate in the workspace or on the a-8 branch
   built one.

## Acceptance evidence

CI-run tests use `FakeBackend`, `FakeStore` or, for the end-to-end rows, the real composition with `FakeBd` as the
`bd` process. Files are in `crates/bd-cache-sync/tests/`.

| Criterion | Tests |
|---|---|
| REQ-BDC-0702 | `test_tick_syncs_only_phases_with_an_open_root`, `test_custom_phases_limits_sync_and_stop`, `test_malformed_custom_phases_is_refused`, `a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase`, `a_configured_phase_without_a_file_and_a_broken_file_are_refused_but_the_rest_runs`, `an_unreadable_phases_directory_is_refused_and_the_journal_still_drains`, `the_phase_files_atm_core_keeps_are_read`, `an_empty_phase_list_is_refused_not_a_schedule_that_ends_at_once` (`tick_phases.rs`) |
| REQ-BDC-0701 | `test_missing_team_is_refused` (`tick_phases.rs`); `a_relative_phases_dir_and_the_default_are_rooted_at_the_repository` (`compose.rs`) |
| REQ-BDC-0703 | `test_tick_budget_defers_drain_entries`, `no_tick_exceeds_25_s_with_every_bd_call_timing_out`, `the_drain_runs_before_the_sync` (`tick_budget.rs`); `a_second_tick_while_a_blocking_sync_is_running_is_skipped_and_reported_busy`, `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `should_stop_returns_continue_when_busy`, `a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply`, `a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick`, `the_stop_flag_ends_a_tick_between_phases` (`job_threading.rs`); `a_scheduled_tick_with_a_slow_bd_ends_within_its_budget` (`e2e.rs`) |
| REQ-BDC-0704 | `test_sync_failure_is_announced_once_per_cause`, `an_undelivered_sync_failure_is_refused_and_retried_not_recorded`, `a_phase_without_a_lead_hears_about_a_sync_failure_through_the_fallback_lead` (`announce.rs`); `a_sync_failure_caused_by_an_unreachable_backend_is_not_announced_per_phase` (unit); `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once`, `an_outage_is_not_told_to_the_lead_of_a_phase_known_to_be_closed` (`outage.rs`) |
| REQ-BDC-0705 | `test_escalation_is_sent_once_to_the_phase_lead` (`announce.rs`); `an_escalation_is_announced_once_and_the_store_remembers_it` (`e2e.rs`, real store) |
| REQ-BDC-0706 | `test_undelivered_escalation_is_retried_and_the_tick_refuses`, `an_entry_whose_phase_has_no_lead_is_refused_every_tick_until_it_has_one`, `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `many_escalations_and_a_slow_sender_stay_within_the_tick_budget`, `a_stopped_tick_does_nothing_and_the_next_tick_does_it_all`, `a_sender_that_ignores_its_timeout_stops_the_tick_after_the_send_that_overran` (`announce.rs`); `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried` (`e2e.rs`); the sender, 12 cases in `atm_process.rs` (the process-tree cases use pid files and liveness checks): `delivers_the_body_on_stdin_with_the_reference_invocation`, `a_failing_atm_is_a_send_error_that_says_why`, `output_beyond_the_limit_is_dropped_and_the_error_says_so`, `an_atm_that_hangs_is_killed_and_gone_at_the_timeout`, `the_whole_process_tree_is_killed_at_the_timeout_not_just_the_child`, `the_time_a_tick_passes_bounds_the_send`, `the_stop_flag_kills_the_child_and_its_descendants`, `an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts`, `a_missing_binary_is_a_send_error_and_does_not_resolve`, `a_timeout_that_no_message_could_use_is_refused` |
| REQ-BDC-0707 | `test_stop_only_when_no_active_phase_and_no_open_work`, `a_phase_whose_root_cannot_be_read_never_ends_the_schedule`, `configuration_the_tick_would_refuse_never_ends_the_schedule` (`stop_rule.rs`) |
| REQ-BDC-0903 (outage) | `test_backend_outage_is_announced_once_and_sync_failures_are_not`, `test_outage_with_uncached_roots_still_tells_every_configured_lead`, `test_outage_is_not_cleared_while_only_the_backend_check_answers`, `test_undelivered_outage_announcement_is_retried`, `the_alert_of_a_scope_that_is_not_synced_is_reset_when_the_outage_ends` (`outage.rs`) |
| Outage of several ticks | `during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains` (`outage.rs`, fakes); `an_outage_of_several_ticks_through_the_real_store_and_cli` (`e2e.rs`, real store) |
| REQ-BDC-0003 | `test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync` and `a_write_journaled_through_the_core_lands_in_bd_via_the_scheduled_drain` (`e2e.rs`: `SyncJob` on an `sc-cron` `Scheduler`, `SqliteStore`, `FlockDrainLock`, `BdCli` over `FakeBd`, which records the argv); the same flow against a scratch `bd`: `tests/live.rs` (testbed) |
| REQ-SYNC-0010 (with REQ-BDC-0904 constraining) | `test_e2e_blocker_close_and_reopen_update_ready` (`e2e.rs`): a journaled close of a cached blocker, applied by a scheduled drain on the real store, makes its depender ready in the same tick's sync; reopen blocks it again; the same for a blocker outside every scope |
| NFR-BDC-0001 (S13) | `a_cached_get_touches_the_same_rows_at_10_100_and_1000_beads`, `a_cached_get_reads_the_same_with_thousands_of_old_applied_entries` (`perf.rs`, gating, deterministic), `the_entries_since_check_uses_indexes_and_ignores_a_long_history` (query plan, `bd-cache-store`), `a_pending_entry_makes_get_adjust_the_whole_base_and_still_agree` (`perf.rs`); `base_entry_rules` (store contract suite, both stores); wall-clock medians below |
| NFR-BDC-0003 | `tests/live.rs::an_incremental_sync_of_a_1000_bead_phase_is_incremental_and_its_time_is_reported` (testbed; the figure is reported with PASS/OVER and the test gates on rows stored < 50); developer smoke run 508 ms |
| `docs/architecture.md` matches the crates | Crate Layout, ADR implementation table, configuration paragraph; the CI boundary check passes |
| Planted bugs | below |

## Evidence: cached `get` latency (NFR-BDC-0001)

Release-profile medians of 1000 cached `get`s on the real `SqliteStore` (macOS arm64, `cargo test --release -p
bd-cache-sync --test perf -- --ignored --nocapture print_median_cached_get_by_scope_size`; a tenth of the beads
blocked), before and after the S4 amendment:

| Beads in scope | before (`984978b`) | after |
|---|---|---|
| 10 | 22 us | 14.0 us |
| 100 | 53 us | 14.4 us |
| 300 | 154 us | 14.5 us |
| 1000 | 1431 us | 14.5 us |

The superlinear part was the base's ready-id dedupe (`Vec::contains` per id) plus decoding and adjusting the whole
base on every `get`. The gate in CI is deterministic: `Counting`, a `CacheStore` wrapper, counts the store calls and
items a `get` returns, and the count is the same at 10, 100 and 1000 beads. The wall-clock figures are informational
(the ignored test and `benches/get.rs`). With a write in flight the whole base is still adjusted (a blocker's close
can unblock any bead), so `get` costs the old amount until the next sync; ADR-CORE-0206.

## Evidence: planted bugs (NFR-BDC-0005)

The first 65 mutants were run on `9e45418` (the 94-mutant re-run at `9f3ceb5` is under "Re-run at the final code commit" below); the two that survived (a stop arriving between phases, and the outcome clip checked against its own constant) got tests and were re-run on `bc1e4ab`, the last commit that changes code (later commits change documents only); the source of the others is identical at both commits. The script
and its raw results are committed: `docs/plans/phase-a/evidence/a-7-planted-bugs.py` and `.json`. It applies each
mutation to the working tree (one literal replacement, checked to match exactly once), runs the whole crate suite,
and restores the file:

```text
cargo test -p bd-cache-sync --all-features --no-fail-fast
git checkout -- <file>
```

The six bugs the sprint names come first; the rest are this sprint's own rules and the amendments' rules.

| Planted bug | File | Exit | Compile error | Failing tests |
|---|---|---|---|---|
| unannounced filter dropped | `crates/bd-cache-model/src/testing/store.rs` | 101 | no | `announcement_state_passes_through_to_the_store`, `journal_lifecycle`, `many_escalations_and_a_slow_sender_stay_within_the_tick_budget`, `test_escalation_is_sent_once_to_the_phase_lead` |
| open-work ignores announcement | `crates/bd-cache-model/src/testing/store.rs` | 101 | no | `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `announcement_state_passes_through_to_the_store`, `journal_lifecycle`, `many_escalations_and_a_slow_sender_stay_within_the_tick_budget` (+1) |
| refusal dropped | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried`, `many_escalations_and_a_slow_sender_stay_within_the_tick_budget`, `test_undelivered_escalation_is_retried_and_the_tick_refuses` |
| team check dropped | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule`, `test_missing_team_is_refused` |
| closed roots active | `crates/bd-cache-sync/src/active.rs` | 101 | no | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule`, `a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase`, `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `an_outage_is_not_told_to_the_lead_of_a_phase_known_to_be_closed` (+6) |
| stop ignores open work | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `test_stop_only_when_no_active_phase_and_no_open_work` |
| fallback lead dropped for entries with no scope | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires` |
| escalation not marked announced | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_sender_that_ignores_its_timeout_stops_the_tick_after_the_send_that_overran`, `an_escalation_is_announced_once_and_the_store_remembers_it`, `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried` (+4) |
| escalation message without the resolve command | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `an_escalation_with_no_scope_goes_to_the_fallback_lead_and_the_stop_rule_still_fires`, `test_escalation_is_sent_once_to_the_phase_lead` |
| sync failure announced every tick (alert not compared) | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `test_sync_failure_is_announced_once_per_cause` |
| sync failure alert not recorded | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `announce::tests::a_sync_failure_caused_by_an_unreachable_backend_is_not_announced_per_phase`, `test_sync_failure_is_announced_once_per_cause` |
| sync success does not clear the alert | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `test_sync_failure_is_announced_once_per_cause` |
| outage alert not recorded after the send | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once`, `an_outage_of_several_ticks_through_the_real_store_and_cli`, `during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains`, `test_backend_outage_is_announced_once_and_sync_failures_are_not` (+4) |
| outage alert not reset when it ends | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `the_alert_of_a_scope_that_is_not_synced_is_reset_when_the_outage_ends` |
| outage: no lead for a scope without one | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_phase_without_a_lead_hears_about_a_sync_failure_through_the_fallback_lead`, `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once`, `test_outage_with_uncached_roots_still_tells_every_configured_lead` |
| outage not told to every configured lead when no root is readable | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `an_outage_is_not_told_to_the_lead_of_a_phase_known_to_be_closed`, `test_outage_with_uncached_roots_still_tells_every_configured_lead` |
| outage message per phase instead of per lead | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_scope_without_a_lead_uses_the_fallback_lead_and_one_lead_hears_once` |
| undelivered outage message recorded as delivered | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `test_undelivered_outage_announcement_is_retried` |
| sync failure of an unreachable cause announced per phase | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `announce::tests::a_sync_failure_caused_by_an_unreachable_backend_is_not_announced_per_phase` |
| drain without the start gate and hard deadline | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `test_tick_budget_defers_drain_entries` |
| sync not deferred before T - 2 s | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `test_tick_budget_defers_drain_entries` |
| sync run with no deadline (a day) | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `no_tick_exceeds_25_s_with_every_bd_call_timing_out` |
| custom phase list ignored | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `test_custom_phases_limits_sync_and_stop` |
| malformed phase entry skipped instead of refused | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule`, `test_malformed_custom_phases_is_refused` |
| unreadable root stops the schedule | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule` |
| configuration the tick refuses can stop the schedule | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule` |
| broken phase file does not hold the stop rule | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `configuration_the_tick_would_refuse_never_ends_the_schedule` |
| busy: stop check stops under a running task | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `should_stop_returns_continue_when_busy` |
| busy tick not reported (runs as an empty pass-through) | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `a_second_tick_while_a_blocking_sync_is_running_is_skipped_and_reported_busy` |
| poison not cleared (reported on every tick) | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick`, `a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply` |
| poisoned mutex not reported | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick`, `a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply` |
| a refused tick reported as ok to the scheduler | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`, `an_undelivered_escalation_is_recorded_by_the_scheduler_as_refused_and_retried`, `job::tests::a_failed_run_carries_the_poison_note_and_outcomes_are_clipped` |
| schedule started without a team | `crates/bd-cache-sync/src/job.rs` | 101 | no | `test_missing_team_is_refused` |
| atm address not validated | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts`, `phase_file_problems_have_their_own_codes`, `test_missing_team_is_refused`, `atm_process (process test)` |
| atm message not on stdin | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `a_large_message_is_not_limited_by_the_argument_list`, `delivers_the_body_on_stdin_with_the_reference_invocation`, `atm_process (process test)` |
| compose ignores the [tick] table | `crates/bd-cache-sync/src/compose.rs` | 101 | no | `a_relative_phases_dir_and_the_default_are_rooted_at_the_repository` |
| announcements ignore the tick budget | `crates/bd-cache-sync/src/control.rs` | 101 | no | `a_sender_that_ignores_its_timeout_stops_the_tick_after_the_send_that_overran`, `control::tests::a_send_is_given_the_time_left_minus_the_reserve_and_nothing_below_the_floor`, `many_escalations_and_a_slow_sender_stay_within_the_tick_budget` |
| announcements ignore the stop flag | `crates/bd-cache-sync/src/control.rs` | 101 | no | `announce::tests::a_stopped_tick_sends_nothing_and_counts_the_message_unsent`, `control::tests::a_send_is_given_the_time_left_minus_the_reserve_and_nothing_below_the_floor` |
| tick ignores the stop flag between phases | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `a_stop_that_arrives_mid_tick_ends_it_between_phases` |
| atm kills only the direct child | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `the_stop_flag_kills_the_child_and_its_descendants`, `the_whole_process_tree_is_killed_at_the_timeout_not_just_the_child`, `atm_process (process test)` |
| atm ignores the time the tick passes | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `the_time_a_tick_passes_bounds_the_send`, `atm_process (process test)` |
| atm ignores the stop flag | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `the_stop_flag_kills_the_child_and_its_descendants`, `atm_process (process test)` |
| atm invocation without the reference environment | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `delivers_the_body_on_stdin_with_the_reference_invocation`, `atm_process (process test)` |
| atm truncated output not marked | `crates/bd-cache-sync/src/atm.rs` | 101 | no | `output_beyond_the_limit_is_dropped_and_the_error_says_so`, `atm_process (process test)` |
| poison met by the stop check is lost | `crates/bd-cache-sync/src/job.rs` | 101 | no | `a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick`, `a_poisoned_mutex_is_recovered_and_reported_in_the_tick_reply` |
| empty phase list accepted | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `an_empty_phase_list_is_refused_not_a_schedule_that_ends_at_once` |
| phase file size not capped | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `a_phase_file_over_the_size_cap_and_a_directory_over_the_file_cap_are_bounded` |
| phase file count not capped | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `a_phase_file_over_the_size_cap_and_a_directory_over_the_file_cap_are_bounded` |
| compose does not root the phases directory | `crates/bd-cache-sync/src/compose.rs` | 101 | no | `a_relative_phases_dir_and_the_default_are_rooted_at_the_repository` |
| embedded error text not clipped | `crates/bd-cache-sync/src/text.rs` | 101 | no | `announce::tests::embedded_error_text_is_clipped`, `job::tests::a_failed_run_carries_the_poison_note_and_outcomes_are_clipped`, `text::tests::short_text_is_kept_and_long_text_is_cut_on_a_character_boundary` |
| recorded outcome not clipped | `crates/bd-cache-sync/src/job.rs` | 101 | no | `job::tests::a_failed_run_carries_the_poison_note_and_outcomes_are_clipped` |
| get ignores a pending entry (fast path always) | `crates/bd-cache/src/ready.rs` | 101 | no | `a_pending_entry_makes_get_adjust_the_whole_base_and_still_agree`, `a_pending_parent_child_dep_add_carries_a_later_parent_block_to_the_child`, `a_pending_parent_child_dep_add_is_released_when_its_blocked_parent_is_unblocked`, `an_entry_applied_since_the_fetch_makes_get_adjust_the_whole_base` (+7) |
| get fast path ignores entries applied since the fetch | `crates/bd-cache-store/src/schema.rs` | 101 | no | `an_entry_applied_since_the_fetch_makes_get_adjust_the_whole_base`, `has_entries_since_rules`, `the_entries_since_check_uses_indexes_and_ignores_a_long_history` |
| get loads the whole base again | `crates/bd-cache/src/ready.rs` | 101 | no | `a_cached_get_touches_the_same_rows_at_10_100_and_1000_beads`, `a_cached_read_of_one_bead_touches_the_same_rows_at_10_and_1000_beads` |
| store index not replaced with the base | `crates/bd-cache-store/src/store.rs` | 101 | no | `a_base_replaced_by_another_writer_is_read_from_its_blob_not_from_a_stale_index`, `base_entry_rules`, `test_e2e_blocker_close_and_reopen_update_ready`, `the_ready_base_index_follows_the_base_through_replace_prune_and_rebuild` |
| store index survives a prune | `crates/bd-cache-store/src/store.rs` | 101 | no | `the_ready_base_index_follows_the_base_through_replace_prune_and_rebuild` |
| store index keeps duplicate blockers | `crates/bd-cache-store/src/codec.rs` | 101 | no | `base_entry_rules` |
| get fast path ignores a pending entry (store check) | `crates/bd-cache-store/src/schema.rs` | 101 | no | `a_pending_entry_makes_get_adjust_the_whole_base_and_still_agree`, `has_entries_since_rules`, `the_entries_since_check_uses_indexes_and_ignores_a_long_history` |
| announcing goes on after the budget is spent (S17) | `crates/bd-cache-sync/src/announce.rs` | 101 | no | `a_sender_that_ignores_its_timeout_stops_the_tick_after_the_send_that_overran`, `many_escalations_and_a_slow_sender_stay_within_the_tick_budget` |
| tick starts although it was asked to stop (RSH-014) | `crates/bd-cache-sync/src/tick.rs` | 101 | no | `a_stopped_tick_does_nothing_and_the_next_tick_does_it_all` |
| send gets no reserve before T (QA2-007) | `crates/bd-cache-sync/src/control.rs` | 101 | no | `control::tests::a_send_is_given_the_time_left_minus_the_reserve_and_nothing_below_the_floor` |
| a phase file that is not a regular file is opened (RSH-017) | `crates/bd-cache-sync/src/phases.rs` | 101 | no | `a_phase_file_that_is_not_a_regular_file_is_reported_not_read` |
| a cached root is trusted without its scope having synced | `crates/bd-cache-sync/src/active.rs` | 101 | no | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule` |
| base_entry trusts the index of a base another writer replaced | `crates/bd-cache-store/src/store.rs` | 101 | no | `a_base_replaced_by_another_writer_is_read_from_its_blob_not_from_a_stale_index`, `a_file_from_before_the_ready_base_index_opens_and_migrates_on_the_next_replace` |
| store index not marked on replace | `crates/bd-cache-store/src/store.rs` | 101 | no | `a_file_from_before_the_ready_base_index_opens_and_migrates_on_the_next_replace` |

Every one of the 65 bugs fails at least one test, none by a compile error, and `git status` was clean after the
run.

### Re-run at the final code commit (fix-a-7-6)

The script was brought up to the code after rounds 4 and 5 (S26 to S30) and run in full on `9f3ceb5`
(94 mutants, macOS arm64, `CARGO_INCREMENTAL=0`): **90 of 94 killed** (88 by a failing test, 2 by a
compile-time assertion), **4 survive**, each with its reason below. Results:
`docs/plans/phase-a/evidence/a-7-planted-bugs.json` (`survived` carries the reason; `const_assert` marks a
build-time kill). The runner now counts `error[E0080]` (a broken `const _: () = assert!(..)`) as a kill.

Patterns that no longer matched after rounds 3 to 5 and were rewritten (the mutated code moved, the rule did not):

| Mutant | File | Killed by |
|---|---|---|
| a cached root is trusted without its scope having synced | `crates/bd-cache-sync/src/active.rs` | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule`, `a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs` |
| unreadable root stops the schedule | `crates/bd-cache-sync/src/tick.rs` | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule`, `a_root_bd_does_not_return_never_ends_the_schedule`, `a_root_that_bd_does_not_know_is_no_open_root_and_is_reported` |
| applied_since misses entries bd_cache.py applied (S18) | `crates/bd-cache-store/src/schema.rs` | `a_pending_entry_makes_get_adjust_the_whole_base_and_still_agree`, `a_scheduled_tick_with_a_slow_bd_ends_within_its_budget`, `a_small_burst_is_walked_and_not_dated_until_past_the_threshold` (+12 more) |
| a missing hinted index is an error (QA3-007) | `crates/bd-cache-store/src/store.rs` | `the_entries_since_check_survives_a_dropped_index` |
| a descendant holding the pipes is waited for in full (QA3-003) | `crates/bd-cache-sync/src/atm.rs` | `a_failing_atm_with_a_descendant_holding_its_pipes_does_not_wait_for_it`, `atm::tests::the_read_deadline_is_clamped_to_the_send_deadline_with_a_floor`, `atm_process (process test)` |
| open does not date the entries bd_cache.py applied | `crates/bd-cache-store/src/store.rs` | `entries_bd_cache_py_applied_count_by_their_updated_at_and_unreadable_ones_count_as_since`, `store::tests::the_open_finds_the_applied_entries_it_could_not_date` |

Mutants for the round 4 and 5 fixes:

| Mutant | File | Killed by |
|---|---|---|
| S26: threshold dropped, every pass dates (count skipped) | `crates/bd-cache-store/src/store.rs` | `a_small_burst_is_walked_and_not_dated_until_past_the_threshold`, `an_unreadable_entry_does_not_hide_earlier_undated_ones_from_the_dating`, `entries_python_applies_while_open_are_dated_by_the_next_check` |
| S26: dating never runs (reads always walk the undated entries) | `crates/bd-cache-store/src/store.rs` | `a_cached_get_dates_entries_python_applied_while_open`, `a_small_burst_is_walked_and_not_dated_until_past_the_threshold`, `an_unreadable_entry_does_not_hide_earlier_undated_ones_from_the_dating` (+3 more) |
| S26: threshold off by one (dates at the threshold) | `crates/bd-cache-store/src/store.rs` | `a_small_burst_is_walked_and_not_dated_until_past_the_threshold` |
| QA5-002: the undatable floor rises onto the first dateable entry | `crates/bd-cache-store/src/store.rs` | **survived**: GAP (off by one, minor): the floor then hides the first dateable entry from the undated count, so the dating starts one entry later than the threshold says; reached only after a skipped dating (write lock held). No test pins floor == first - 1. |
| QA5-002: the undatable floor rises past a dateable entry | `crates/bd-cache-store/src/store.rs` | `an_unreadable_entry_does_not_hide_earlier_undated_ones_from_the_dating` |
| QA5-003: the busy timeout is not restored after the no-wait dating | `crates/bd-cache-store/src/store.rs` | `a_write_after_a_skipped_dating_waits_for_the_lock_again` |
| QA5-003: the dating waits for the write lock (no NoWait) | `crates/bd-cache-store/src/store.rs` | **survived**: GAP (timing only): with the lock held by the same thread the wait ends with BUSY and the answer is the same; telling the two apart needs an elapsed-time assertion, which FTQ-020 removed as stall-sensitive. The restore of the timeout is killed (a_write_after_a_skipped_dating_waits_for_the_lock_again). |
| S27: the tree kill and the reaping do not share one deadline | `crates/bd-cache-sync/src/atm.rs` | **survived**: UNREACHABLE: the two deadlines differ only when the child is still alive after SIGKILL (an uninterruptible state), which no portable test can make. The tree-kill bound, the KILL_BOUND value and SEND_RESERVE are each killed. |
| S27: the kill bound is far too long | `crates/bd-cache-sync/src/atm.rs` | compile-time assertion (`const _: () = assert!`) fails the build |
| S27: the tree kill is not given up at the deadline | `crates/bd-cache-sync/src/atm.rs` | `atm::tests::a_child_that_ignores_term_is_killed_within_the_reserve_despite_a_stuck_tree_kill`, `atm::tests::a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped` |
| S28: a child not reaped in time is dropped, not handed to the reaper | `crates/bd-cache-sync/src/atm.rs` | **survived**: UNREACHABLE: the call site runs only for a child that outlives SIGKILL by KILL_BOUND. adopt() and the reaper are killed (the helper path and the reaper loop). |
| S28: the reaper lets go of every process at once | `crates/bd-cache-sync/src/atm.rs` | `atm::tests::a_child_that_ignores_term_is_killed_within_the_reserve_despite_a_stuck_tree_kill`, `atm::tests::a_child_that_is_not_reaped_in_time_is_reaped_by_the_reaper`, `atm::tests::a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped` |
| S28: the reaper never looks at what it holds | `crates/bd-cache-sync/src/atm.rs` | `atm::tests::a_child_that_ignores_term_is_killed_within_the_reserve_despite_a_stuck_tree_kill`, `atm::tests::a_child_that_is_not_reaped_in_time_is_reaped_by_the_reaper`, `atm::tests::a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped` |
| S27: SEND_RESERVE shrunk below the read grace plus the kill bound | `crates/bd-cache-sync/src/control.rs` | compile-time assertion (`const _: () = assert!`) fails the build |
| RBP-F039: a root bd did not return is not refused | `crates/bd-cache-sync/src/tick.rs` | `a_root_that_bd_does_not_know_is_no_open_root_and_is_reported` |
| RBP-F039: a root bd did not return is never recorded | `crates/bd-cache-sync/src/active.rs` | `a_root_that_bd_does_not_know_is_no_open_root_and_is_reported` |
| QA5-004: scope_synced says every scope has synced | `crates/bd-cache/src/lib.rs` | `a_phase_whose_root_cannot_be_read_never_ends_the_schedule`, `a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs` |

Survivors, none an equivalent mutant; each is a gap named for the lead:

- **QA5-002 off by one**: the floor rising onto the first dateable entry (`first`, not `first - 1`) is not caught;
  the effect is that the dating starts one entry past the threshold, after a skipped dating. A test pinning the
  floor would kill it.
- **QA5-003 no `NoWait`**: telling a read that waits from one that does not needs an elapsed-time assertion, which
  FTQ-020 removed as stall-sensitive. The restore of the timeout is killed.
- **S27 shared deadline** and **S28 `adopt` at the `kill_tree_with` call site**: reachable only by a child that
  outlives `SIGKILL` for `KILL_BOUND` (an uninterruptible state), which no portable test can make. The tree-kill
  bound, `KILL_BOUND`, `SEND_RESERVE` and the reaper itself are killed.

## bd-compat workflow, real run

`docs/plans/phase-a/evidence/a-7-bd-compat.md`. `workflow_dispatch` ran for real on this branch (pinned version
[40083096528](https://github.com/randlee/beads-cache/actions/runs/40083096528) passed; the failure path with the
`expect_schema_sha` hook, [40083225647](https://github.com/randlee/beads-cache/actions/runs/40083225647), opened one issue,
#12, closed with a note). `repository_dispatch` and `workflow_dispatch` on `main` are **phase close-out gates owned by
bc-lead, due at the `develop` -> `main` merge** (S9), not a-7 sprint gates.

## Testbed

Run on `m5-atmbench` at the last code-changing commit `9f3ceb5`: a-7 live 2/2, incremental sync 380 ms against the 1 s
limit, a-2 probes 7/7 with `SC_BD_PINNED_ANY=1`. `docs/plans/phase-a/evidence/a-7-testbed.md` has the commands, the
figures and the developer-machine smoke run. bc-lead's own run at the merge head, due 2026-10-31 (S8, S16), is still the
owner's to record.

## QA round 1 findings and their fixes

| Finding | Fix | Evidence |
|---|---|---|
| ARCH-001..005 | Approved ADR-SYNC-0001..0005 (lead ruling S1) with the round's corrections | docs/bd-cache-sync/architecture.md |
| ARCH-006, REQ-QA-008, F-2, Deviation 1 | `Cache::clock()` added (S2); `tick`, `stop_reason`, `SyncJob::new` lose the clock parameter; four-parameter `tick` restored | `tick` doctest; `tests/*` call `tick(cache, cfg, sender, end)`; ADR-SYNC-0001 |
| ARCH-007 | ADR-SYNC-0006 records the third-party and dev dependencies (Approved) | docs/bd-cache-sync/architecture.md |
| ARCH-008, REQ-QA-011 | ADR-SYNC-0007: phase-file schema and `atm send` contract checked against `bd_cache_cron.py` at atm-core 05831df; the invocation was wrong (`--as cron`, no `--json`, no env) and is fixed; real `phase-bd.toml`/`phase-bk.toml` ported as fixtures | `tests/fixtures/`, `the_phase_files_atm_core_keeps_are_read`, `delivers_the_body_on_stdin_with_the_reference_invocation` |
| ARCH-009 | Outage targets = active + unreadable phases, never known-closed | `an_outage_is_not_told_to_the_lead_of_a_phase_known_to_be_closed`; planted 'outage not told to every configured lead...' |
| ARCH-010 | ADR-BDC-0003 added to `adrs` | sprint doc frontmatter |
| ARCH-011, FTQ-003 | One clock source (`Cache::clock`); the e2e fixture's `Bridge` clock puts the cron and cache timelines on one origin; deterministic budget e2e | `tests/common/real.rs` `Bridge`; `a_scheduled_tick_with_a_slow_bd_ends_within_its_budget` |
| ARCH-012 | docs/architecture.md configuration paragraph now points at ADR-SYNC-0007 | docs/architecture.md |
| ARCH-013 | ADR table cites ADR-SYNC-0003 for the outage | docs/architecture.md |
| ARCH-014 | bd-cache-py link replaced by an a-8 note | docs/architecture.md |
| ARCH-015, REQ-QA-014 | `bd-cache-store`, `sc-bd-cli`, `sc-cron` added to `[workspace.dependencies]` (S7); crate uses `workspace = true`; plan's Shared files table updated | Cargo.toml, phase-a-plan.md |
| ARCH-016, REQ-QA-002, REQ-QA-003 | Ruled deferral S8: testbed run by the lead after this fix; `a-7-testbed.md` is a fill-in table; both run-live scripts checked | docs/plans/phase-a/evidence/a-7-testbed.md |
| ARCH-017, REQ-QA-009 | Ruled deferral S9: owner bc-lead, due at the develop -> main merge | a-7-bd-compat.md |
| REQ-QA-001, QA-003, F-1 | Cached `get` reads the bead's own entry (`CacheStore::base_entry`, indexed by `ready_base_blocked`); superlinear dedupe removed; release medians flat at 14 us for 10..1000 beads | `a_cached_get_touches_the_same_rows_at_10_100_and_1000_beads`; `base_entry_rules` contract rule; table below |
| FTQ-001 | The gate counts rows touched (deterministic); wall clock is an ignored informational test and the bench | `tests/perf.rs`, `tests/common/counting.rs` |
| REQ-QA-004 | Phase plan ownership: a-7 row lists REQ-SYNC-0001..0013 and the 0905 real-run evidence; e2e row re-rooted under REQ-SYNC-0010; 0904 stays in `constrained_by` | phase-a-plan.md; acceptance table |
| REQ-QA-005 | `constrained_by` adds REQ-BDC-0006, 0105, 0201, 0403, 0406, 0501, 0504, 0601, 0902; NFR-BDC-0002 named as deferred | sprint doc, Requirement scope notes |
| REQ-QA-006 | REQ-BDC-0701, 0704, 0706 amended (S3) | docs/requirements.md |
| REQ-QA-007 | REQ-BDC-0707 and 0702 amended (S5) | docs/requirements.md |
| REQ-QA-010 | project-plan row set to in review | docs/project-plan.md |
| REQ-QA-012 | Mutation script and results committed; atm rows name their cases | docs/plans/phase-a/evidence/a-7-planted-bugs.py, .json |
| REQ-QA-013 | Validation table is the quality-policy command | sprint doc, Validation |
| RSH-001, RSH-002, QA-001 | Messages are sent inside the tick budget with `send_within(min(timeout, time left))`; unsent ones stay unmarked; atm doc claim now true | `many_escalations_and_a_slow_sender_stay_within_the_tick_budget`; planted 'announcements ignore the tick budget' |
| RSH-003 | `StopFlag` checked between phases and before each message, kills the atm child; shutdown order documented | `the_stop_flag_ends_a_tick_between_phases`, `a_stop_that_arrives_mid_tick_ends_it_between_phases`, `the_stop_flag_kills_the_child_and_its_descendants`, `a_stopped_tick_does_nothing_and_the_next_tick_does_it_all` |
| RSH-005, QA-006 | `should_stop` has a 3 s bound; roots are read from the cache first so the second read is cheap (ActiveSet not shared: not simple) | `STOP_CHECK_BUDGET`, ADR-SYNC-0002 |
| QA-002 | Poison remembered in a shared flag; reported by the next tick whichever call recovered it | `a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick`; planted 'poison met by the stop check is lost' |
| QA-007 | Failed outcome names the poison | `a_failed_run_carries_the_poison_note_and_outcomes_are_clipped` |
| QA-004, RSH-004 | atm child leads a process group (Unix) / `taskkill /T` (Windows); whole tree killed | `the_whole_process_tree_is_killed_at_the_timeout_not_just_the_child`; planted 'atm kills only the direct child' |
| QA-005 | Kill and reap on a `try_wait` error | `atm.rs` `kill_tree` on every exit path |
| QA-009 | Hang test proves the child is gone with a proof-of-life marker | `an_atm_that_hangs_is_killed_and_gone_at_the_timeout` |
| FTQ-002 | Capture threads waited for with the remaining budget (100 ms floor); truncation marked | `output_beyond_the_limit_is_dropped_and_the_error_says_so`; planted 'atm truncated output not marked' |
| QA-008 | `compose_with` roots a relative or default `phases_dir` at the repo root | `tests/compose.rs`; planted 'compose does not root the phases directory' |
| FTQ-004, FTQ-005, FTQ-006 | Live setup calls bounded (`run_bounded`, 60 s); `timeout` around cargo in run-live.sh; per-test cache file + static mutex; figure reported with PASS/OVER, gate on rows stored < 50, fixed sleeps removed | `tests/live.rs`, `tests/run-live.sh`; developer smoke run |
| RSH-006, RBP-F009 | `# Blocking` docs and `try_with_cache` | `try_with_cache_does_not_wait_for_a_running_tick` |
| RSH-007 | Phase file size cap (64 KiB) and count cap (256) | `a_phase_file_over_the_size_cap_and_a_directory_over_the_file_cap_are_bounded`; planted caps |
| RSH-008 | Refused outcome clipped to 1 KiB | unit test in job.rs; planted 'recorded outcome not clipped' |
| RSH-009 | Embedded error text clipped to 2 KiB on a char boundary | `embedded_text_is_clipped_on_a_character_boundary`; planted 'embedded error text not clipped' |
| RSH-010 | `SyncJob::new` validates and returns `Result` | `a_job_with_an_unusable_config_is_refused_at_construction` |
| RSH-011 | Empty `phases` list refused | `an_empty_phase_list_is_refused_not_a_schedule_that_ends_at_once`; planted 'empty phase list accepted' |
| RSH-012 | `with_timeout` returns `Result` (zero and >= tick budget refused) | `a_timeout_that_no_message_could_use_is_refused` |
| RSH-013 | `schedule_spec(config, repo, Some(&atm))` checks the phases directory and that atm resolves | `AtmSender::resolve`; `a_missing_binary_is_a_send_error_and_does_not_resolve` |
| RBP-F001, F007 | `Refusal { code, detail, fix }` in `TickReply` | `RefusalCode`, tests assert codes and non-empty fix |
| RBP-F002, F010 | `PhaseFileProblem` enum; no message matching; directory entries carry validated names | `phases.rs` |
| RBP-F003 | `MemberName` newtype, one validator (used by settings, phase files, the sender) | `MemberName` doctest; planted 'atm address not validated' |
| RBP-F004, S11 | `SendErrorKind` + `fix()`, `SendError::new/with_kind`; `EscalationSender::send_within` provided method overridden by `AtmSender` | bd-cache-model `sender.rs`; atm tests assert kinds |
| RBP-F005, F011 | Binary shown lossily; capture loop is total (no `unwrap_or_default` fallbacks) | `atm.rs` |
| RBP-F006 | One Display/source convention; `code()` on `ComposeError` and `ScheduleError` | `compose_errors_have_codes_and_sources` |
| RBP-F008 | `AlertCause` enum | `alert_causes_have_stable_keys` |

## QA round 2 findings and their fixes

| Finding | Fix | Evidence |
|---|---|---|
| REQ-QA-101 | Crate ids listed (REQ-STORE-0001..0008, REQ-MODEL-0001..0005, REQ-CORE-0003/0004); REQ-STORE-0003 amended; DDL listing and open.rs updated; compat tests committed | `docs/bd-cache-store/requirements.md`, `open.rs::v3_tables_are_unchanged_and_extensions_are_additive`, store tests below |
| ARCH-101 | ADR-MODEL-0003 a-7 amendment (Approved) for `base_entry`, `has_entries_since`, `send_within`, `SendErrorKind` and the breaking `SendError.kind`; 'frozen' reworded | docs/bd-cache-model/architecture.md |
| ARCH-102, QA2-002 | `base_entry` runs in one read transaction | `SqliteStore::base_entry`; ADR-STORE-0004; planted 'base_entry trusts the index...' |
| ARCH-103 | ADR-STORE-0004 amends ADR-STORE-0001/0002; DDL block updated; records bd_cache.py behaviour | docs/bd-cache-store/architecture.md |
| ARCH-104 | adrs list completed (ADR-CORE-0002/0102/0103/0201/0203/0204/0205, ADR-STORE-0001/0002, ADR-MODEL-0001..0004) | sprint doc frontmatter |
| ARCH-105 | Files added to owned_paths with S14 comments | sprint doc frontmatter |
| ARCH-106 | ADR-SYNC-0006 lists `rusqlite` (dev) and tokio `test-util` | docs/bd-cache-sync/architecture.md |
| ARCH-107 | ADR table cites ADR-CORE-0206 and ADR-STORE-0004 | docs/architecture.md |
| ARCH-108 | read-path.md header and file table updated (`cached`, `blocked_state`); bd-cache architecture lists `clock()` | docs/bd-cache/ |
| ARCH-109, REQ-QA-103 | One smoke record at the final code commit; testbed due 2026-10-31, owner bc-lead (S16) | a-7-testbed.md |
| REQ-QA-102 | Deliverable 4 and its acceptance row amended by ruling S13; gate = row count + large-journal check + plan assertion | sprint doc; `perf.rs`; `the_entries_since_check_uses_indexes_and_ignores_a_long_history` |
| REQ-QA-104 | The seven atm planted rows carry case names: the process test now reports each failing case (catch_unwind) | a-7-planted-bugs.json |
| QA2-001 | `has_entries_since`: two EXISTS lookups through `journal_pending` and the `applied_epoch` column with plain partial indexes `journal_applied_epoch` and `journal_applied_unknown` (ADR-STORE-0004); get no longer lists the journal | `has_entries_since_rules`, `a_cached_get_reads_the_same_with_thousands_of_old_applied_entries`, plan test |
| QA2-003 | Pre-index file migration test committed | `a_file_from_before_the_ready_base_index_opens_and_migrates_on_the_next_replace` |
| QA2-004 | atm spawns the resolved program | `atm.rs` (`resolve` then `Command::new(program)`) |
| QA2-005, RSH-015 | Failure path waits for output at most 250 ms; success path neither waits nor kills (the reaped pid may be reused) | `atm.rs` docs; `collect` |
| QA2-006, RSH-016 | Tree kill status checked and logged; falls back to killing the child; absolute `/bin/kill` | `kill_tree` |
| QA2-007 | Sends keep a 500 ms reserve before T | `control.rs` unit test; planted 'send gets no reserve' |
| FTQ-007, 008, 009 | Process tests use readiness (pid files with a deadline), `Reaper` cleanup, pid-liveness polls; no fixed sleeps; stop test fires after both processes started | `atm_process.rs` |
| FTQ-010 | Live test renamed, deferral reported as inconclusive | `live.rs` |
| FTQ-011 | run-live.sh runs cargo in its own process group and kills the group and the watchdog's sleep | tested in the scratch harness: timeout case 2 s, no orphans |
| RSH-014 | Stop flag checked before the drain and after it | planted 'tick starts although it was asked to stop'; `a_stopped_tick_does_nothing_and_the_next_tick_does_it_all` |
| RSH-017 | Phase files read only if regular, with a read bound | `a_phase_file_that_is_not_a_regular_file_is_reported_not_read` |
| RSH-018 | The tick reads a root with `Cache::cached` (no scope scan, no bd call) | `a_cached_read_of_one_bead_touches_the_same_rows_at_10_and_1000_beads` |
| RSH-019 | Escalations stop at the first spent budget; the rest are counted unsent | `a_sender_that_ignores_its_timeout_stops_the_tick_after_the_send_that_overran` |
| RSH-020, S17 | Budget checked after every send; a sender that ignores the timeout costs at most the send in flight | same test; planted 'announcing goes on after the budget is spent' |
| RBP-F012 | One total `clip` helper | `text.rs` + test |
| RBP-F013, F014 | `AlertCause` (with `from_key`, `Serialize`) in outage and `SyncFailure.cause`; `Announcement.lead` is a `MemberName` | `alert_causes_round_trip_through_their_keys` |
| RBP-F015 | Port keeps `&str`; contract documented; defensive guard commented (S15) | sender.rs docs |
| RBP-F016 | One `rooted_phases_dir`; `rooted_at` removed | compose.rs, phases.rs |
| RBP-F017 | `bd_cache::CLOSED_STATUS` used by the cache and the sync crate | active.rs |
| RBP-F018 | `PhaseFileProblem` has per-variant `code()`/`fix()` and implements `Error`; eight refusal codes | `phase_file_problems_have_their_own_codes` |
| RBP-F019 | `Refusal.send_kind` (stable name) kept from the sender | `Refusal::undelivered` |
| RBP-F020, F021 | A wait error is `Other`; an invalid address says lead or team | `atm.rs` |
| RBP-F022, F023 | ScheduleError/ComposeError: message says what failed, cause is the source, `fix()`; one lower-case code namespace, documented | lib.rs docs; `compose_errors_have_codes_and_sources` |
| RBP-F024, F025, F026 | No `write!` into a String; one `MEMBER_RULE` text; join failure outcome has a code and hint | phases.rs, announce.rs, job.rs |
| RBP notes | EXISTS for pending/applied emptiness (QA2-001); duplicated `doc(inline)` removed | lib.rs |

## QA round 4 findings and their fixes

Reviewed at `bd44f85` (layer `sprint/a-7-fix3-sync`). Fixes: PR #22 (`sprint/a-7-fix4-store`), PR #18
(`sprint/a-7-fix4-atm`), PR #21 (`sprint/a-7-fix4-sync`, top of stack #17). Test names are those in the PR bodies.

| Finding | Fix | Evidence |
|---|---|---|
| ARCH-301, ARCH-302 | ADR-SYNC-0006 (libc) and ADR-SYNC-0007 (atm output) amendments Approved (S22) | `docs/bd-cache-sync/architecture.md` |
| ARCH-303 | Crate Layout row lists `libc (Unix only: O_NONBLOCK)` | `docs/architecture.md` |
| ARCH-304 | A cached root is trusted only when the phase's own scope has `last_ok_at` (S24) | `a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs` |
| ARCH-305 | A root bd does not return is unknown and reported; `stop_reason` answers Continue (S23) | `a_root_bd_does_not_return_never_ends_the_schedule`, `a_root_that_bd_does_not_know_is_no_open_root_and_is_reported` |
| ARCH-306 | atm.rs comment no longer says there is no libc dependency | `atm.rs` |
| ARCH-307 | REQ-STORE-0003 lists all four extension indexes | `docs/bd-cache-store/requirements.md` |
| QA4-001 (important) | Undated entries `bd_cache.py` applied while open are dated before the journal check asks (S25); best effort, no busy wait; get median 16.0 us at 20k rows (was 1.73 ms) | store tests measure SQLite VM steps |
| QA4-002 | `perf.rs` seeds `applied_epoch`; a separate undated case measures SQL work in VM steps | `perf.rs` |
| QA4-003 | `applied_since` is a `UNION ALL` over the two partial indexes, ordered by seq; plan and VM-step assertions | `bd-cache-store` tests |
| QA4-004 | Open warns with the seqs of applied entries it could not date; ADR-STORE-0004 gives the operator fix | `bd-cache-store` open tests |
| QA4-005 | A missing index is matched on the `SQLITE_ERROR` code and message; warned once, then unhinted queries until the next open | `the_entries_since_check_survives_a_dropped_index` |
| QA4-006 | The stop flag is checked before the root `bd list --id` | `active::tests::a_set_stop_flag_skips_the_root_refresh` |
| RSH-021, RSH-025, RSH-026 | atm send bounded (read grace clamped to the send deadline; kill path polled with a bound); collect tested with a shared deadline | `atm.rs`, `atm_process.rs` |
| FTQ-016, FTQ-017, FTQ-018 | `run-live.sh` cleanup on EXIT/INT/TERM; deferral reported as inconclusive; pids tracked as discovered | `run-live.sh`, `live.rs`, `atm_process.rs` |
| RBP-F030 | `ComposeError::Cache` fix returns `e.fix()` | `compose_errors_have_codes_and_sources` |
| RBP-F031 | Outage targets use `is_outage_alert` | `an_outage_alert_in_bd_cache_pys_form_is_not_announced_again` |
| RBP-F032 | InvalidAddress fix in operator wording | `the_invalid_address_fix_states_the_rule_without_rust_identifiers` |
| RBP-F033 | `SYNC_CAUSES` replaced by an exhaustive `next_sync_cause` match | `every_sync_cause_round_trips_through_its_key` |
| RBP-F034 | Code and fix per config-load variant | `compose_errors_have_codes_and_sources` |
| RBP-F035 | `FakeDrainLock` uses `AtomicBool` | existing drain-lock tests |
| req-qa | PASS, no findings | |

## QA round 5 findings and their fixes

Reviewed at `9990a10` (`sprint/a-7-fix4-sync`). Fixes: PR #28 (`sprint/a-7-fix5-store`), PR #29
(`sprint/a-7-fix5-sync`), and fix-a-7-6 (PR #30: the Windows temp-dir fix and the evidence).

| Finding | Fix | Evidence |
|---|---|---|
| ARCH-401 (blocking) | Logging goes through `sc_beads::logging::tracing`; the direct `tracing` dependency is removed | `crates/bd-cache-store/Cargo.toml` |
| ARCH-402, REQ-QA-502 | S25/S26 Approved in ADR-STORE-0004; ADR-STORE-0002 Concurrency exception note; port docs allow a write that neither waits nor changes the answer | `docs/bd-cache-store/architecture.md`, `bd-cache-model/src/store.rs` |
| ARCH-403 | New ADR-STORE-0005 records the store dependencies (incl. `rusqlite` `functions`, dev) | `docs/bd-cache-store/architecture.md` |
| REQ-QA-501, RSH-027 | Tree kill and child reap share `KILL_BOUND` (1 s); `SEND_RESERVE` 1.5 s; const asserts tie the constants to `HARD_MARGIN`; REQ-SYNC-0011 and ADR-SYNC-0007 amended (S27) | `a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped`, `a_child_that_ignores_term_is_killed_within_the_reserve_despite_a_stuck_tree_kill`; planted S27 |
| RSH-028 | One detached reaper thread (`bd-cache-atm-reaper`) takes a child or helper given up on (S28) | `a_child_that_is_not_reaped_in_time_is_reaped_by_the_reaper`; planted S28 |
| REQ-QA-503 | The message-prefix reliance of the missing-index test is recorded in ADR-STORE-0004 | ADR-STORE-0004 |
| QA5-001 | Read-path dating only above `READ_DATING_THRESHOLD` = 2000, never waits (S26); measurements in ADR-STORE-0004 | `a_small_burst_is_walked_and_not_dated_until_past_the_threshold`; planted S26 |
| QA5-002 | One warning per new highest undatable seq; a floor keeps known unreadable entries out of the count and never rises past a dateable one | `an_unreadable_entry_does_not_hide_earlier_undated_ones_from_the_dating`, `undatable_entries_written_while_open_are_warned_once_and_not_walked_again`; planted QA5-002 |
| QA5-003 | `NoWait` drop guard restores `BUSY_TIMEOUT` | `a_write_after_a_skipped_dating_waits_for_the_lock_again`; planted QA5-003 |
| QA5-004 | `Cache::scope_synced` reads only `sync_state` (S30) | `a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs`; planted QA5-004 |
| QA5-005 | Dead `allow` removed | `store.rs` tests |
| FTQ-019 | The collect test uses an already-passed Instant; no upper bound on the clock | `atm.rs` tests |
| FTQ-020 | Elapsed-time assertion removed from the store test | `tests/store.rs` |
| RBP-F036 | Member rule and InvalidAddress fix built from one text (`macro_rules!` + `concat!`) | `sender.rs` |
| RBP-F037 | `PhaseName::RULE` used by the validator and every fix that states the rule; fix-a-7-6 also uses it in `ConfigLoadError::Scope` | `compose.rs`, `reply.rs` tests; `location.rs` |
| RBP-F038 | Compile-time check enforces the declaration order of `next_sync_cause`; hand count removed (S29) | `announce.rs` |
| RBP-F039 | `phase_root_not_returned` refusal, separate from a read failure; S23 unchanged | `a_root_that_bd_does_not_know_is_no_open_root_and_is_reported`; planted RBP-F039 |

### CI fix on #28 (windows-latest)

| Problem | Fix | Evidence |
|---|---|---|
| `store::tests::undatable_entries_written_while_open_are_warned_once_and_not_walked_again` panicked with `Os { code: 267, NotADirectory }`: the temp dir name held the `{:?}` of `SystemTime`, whose text Windows rejects in a path | The name uses nanos since the epoch; no other `{:?}` time in a path exists in the workspace | `9f3ceb5`; PR #30 CI green on ubuntu, macos and windows |

## Validation

At `9f3ceb5` (the last code-changing commit; later commits change documents and the evidence script only), locally
(macOS arm64, Rust 1.98.1), each gate under `bash -c 'set -o pipefail; ...'`:

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features` | pass |
| `cargo test --workspace --all-features` | pass (779 passed, 0 failed, 16 ignored) |
| Planted bugs, 94 mutants | 90 killed (2 by a compile-time assertion), 4 survive with reasons (see Evidence: planted bugs) |
| Testbed (`m5-atmbench`, bd 1.3.2, `9f3ceb5`) | a-7 live 2/2; incremental sync 380 ms < 1 s; a-2 7/7 with `SC_BD_PINNED_ANY=1` (`evidence/a-7-testbed.md`) |

CI (per the CI-evidence convention in the phase plan): run
[40118172977](https://github.com/randlee/beads-cache/actions/runs/40118172977) of PR #30 is green on ubuntu, macos and
windows at `9f3ceb5`, the last code-changing commit. It fixes the windows-latest failure of run 40117695818 on PR #28
(`Os { code: 267, NotADirectory }`). Earlier green run on the sprint's first final code commit:
[40102640467](https://github.com/randlee/beads-cache/actions/runs/40102640467) at `bc1e4ab`.

## QA round 3 findings and their fixes

Reviewed at `aec3a12`. Code fixes landed at `a0ba471` (layer `sprint/a-7-fix3`
base); the store ADR documents are in layer `sprint/a-7-fix3-store`, the sync
ADR documents in layer `sprint/a-7-fix3-sync`, and this evidence in
`sprint/a-7-fix3-evidence`. The test names are those in the code at `a0ba471`.

| Finding | Fix | Evidence |
|---|---|---|
| ARCH-201, REQ-QA-201, QA3-001 | No index holds an expression: `unixepoch()` is not in any index, so a `bd_cache.py` on SQLite < 3.38 (Ubuntu 22.04: 3.37.2) writes the journal. The epoch is a stored column with plain and partial indexes; an earlier build's expression index is dropped on open; ADR-STORE-0002 / REQ-STORE-0003 amended (S18) | `crates/bd-cache-store/src/schema.rs`; `no_index_holds_a_function_and_journal_writes_work_without_unixepoch`, `the_expression_index_of_an_earlier_build_is_dropped_on_open`, `entries_bd_cache_py_applied_count_by_their_updated_at_and_unreadable_ones_count_as_since` (store tests); docs in `sprint/a-7-fix3-store` |
| QA3-007 | `INDEXED BY` failure (index dropped by another writer) falls back to the unhinted query, correct and slower | `the_entries_since_check_survives_a_dropped_index` |
| QA3-006 | `Cache::cached` doc states the O(1) cost holds while no write is in flight; with one in flight the scope base is adjusted as `get` does (ADR-CORE-0206) | `crates/bd-cache/src/lib.rs`; ADR-SYNC-0005 amendment in layer `sprint/a-7-fix3-sync` |
| ARCH-202 | ADR-SYNC-0005 reads roots with `Cache::cached` | `docs/bd-cache-sync/architecture.md` (layer `sprint/a-7-fix3-sync`, PR #15) |
| ARCH-203 | `vertical_rationale` reworded: contract defined in a-1, amended by the ADR-MODEL-0003 a-7 amendment; "frozen" dropped | this document's frontmatter |
| ARCH-204 | Every path fix-a-7-3 touched is in `owned_paths` with S4/S14/S18 comments | this document's frontmatter |
| ARCH-205 | Testbed record says the incremental-sync time is reported (PASS/OVER), not asserted | `evidence/a-7-testbed.md` |
| FTQ-012 | Handshake-driven kill: the kill waits for the fake to start; the 250 ms test checks the pids it started | `atm_process.rs` (`assert_started_ones_are_gone`, `the_whole_process_tree_is_killed_at_the_timeout_not_just_the_child`) |
| FTQ-013 | `Reaper` is created first, before any assertion that can fail, and the fakes have a bounded lifetime, so none is orphaned | `atm_process.rs` (`Reaper`) |
| FTQ-014 | `run-live.sh` sends KILL to the group after `wait` (the watchdog is not killed during the TERM grace) and traps INT/TERM | `crates/bd-cache-sync/tests/run-live.sh` |
| FTQ-015 | Inconclusive handling on every timed tick: a deferral is reported as inconclusive, not as a failure | `crates/bd-cache-sync/tests/live.rs` |
| RBP-F017 | One `CLOSED_STATUS` constant in bd-cache-model, re-exported from `bd_cache` | `bd-cache-model/src/op.rs`, `lib.rs`; `bd-cache/src/lib.rs`, `ready.rs` |
| RBP-F025 | Member-rule fix texts say the whole rule, from one `MEMBER_RULE` constant | `bd-cache-sync/src/phases.rs` (`MEMBER_RULE`), `reply.rs`; `bd-cache-model/src/sender.rs`, `lib.rs` |
| RBP-F027 | Stable `code()` on the remaining error types, including `InvalidTimeout` | `bd-cache-sync/src/atm.rs`, `phases.rs`, `job.rs`; `bd-cache-model/src/sender.rs` |
| RBP-F028 | `ComposeError::fix()` | `crates/bd-cache-sync/src/compose.rs` (`compose_errors_have_codes_and_sources`) |
| RBP-F029 | No `let _ = write!` into a `String` in the three files | `bd-cache-store/src/error.rs`, `bd-cache-model/src/pyjson.rs`, `bd-cache-model/src/testing/store.rs` |
| QA3-002, RSH-021 | `collect` reads the streams against one shared deadline `min(now + grace, until)`, so the send keeps its reserve; doc corrected | `crates/bd-cache-sync/src/atm.rs` |
| QA3-003 | Test for a failing exit with a descendant holding the pipes | `a_failing_atm_with_a_descendant_holding_its_pipes_does_not_wait_for_it` |
| QA3-004 | Windows resolution tries `PATHEXT` extensions first; a bare name only if it already ends in one (npm shim) | `atm.rs` (`resolve`, `DEFAULT_PATHEXT`) |
| QA3-008 | Unix resolution requires a regular file with an execute bit | `atm.rs`; `tests/atm_process.rs::a_file_without_an_execute_bit_does_not_resolve` |
| QA3-005 | Shutdown delay documented: the drain keeps its own budget and ignores the flag; worst case `T - 2 s`; no phase starts and nothing is sent afterwards | `control.rs` docs; `tests/tick_budget.rs::a_stop_that_arrives_mid_drain_is_seen_within_the_drains_hard_deadline`, `tests/common/blocking.rs` (S20); ADR note in layer `sprint/a-7-fix3-sync` |
| RSH-022 | `try_wait` runs before the stop check, so a send whose child already exited is judged by its status, not reported Cancelled and sent again | `atm.rs`; `an_atm_that_exits_without_reading_still_counts_as_delivered_by_its_status` |
| RSH-023 | Detached reader/writer threads bounded and documented (at most three per send) | `atm.rs` docs; ADR-SYNC-0007 amendment in layer `sprint/a-7-fix3-sync` |
| RSH-024 | Phase files open with `O_NONBLOCK` and are checked on the open handle (no metadata-then-open race); a pipe is reported, not read | `crates/bd-cache-sync/src/phases.rs`; `tests/tick_phases.rs::a_phase_file_that_is_a_pipe_does_not_block_the_tick`; `Cargo.toml` (libc on unix) |
| S21 (not a QA id) | An alert `bd_cache.py` stored (`"<code>: <detail>"`) is read by `AlertCause::from_stored`; a generic `BD_CACHE.SYNC_FAILED` covers a failure it announced | `announce.rs` (unit test `an_alert_in_bd_cache_pys_format_is_read_and_covers_the_failure_it_announced`); the two tick-level tests in `tests/announce.rs` and the ADR note are in layer `sprint/a-7-fix3-sync` |
| (planted-bug script) | `a-7-planted-bugs.py` extended for the round-3 fixes; run in phase 2 | `evidence/a-7-planted-bugs.py` |

### CI fix on a0ba471 (windows-latest)

| Problem | Fix | Evidence |
|---|---|---|
| Opening a directory as a phase file on Windows fails with Unreadable rather than NotAFile, so the NotAFile case differs by platform | `phases.rs` maps that open failure to NotAFile on Windows | layer `sprint/a-7-fix3-sync` (PR #15); green windows-latest run cited in phase 2 |
