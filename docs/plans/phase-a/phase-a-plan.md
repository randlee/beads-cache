# Phase A — cache core, shared beads backend and cron prototype

Status: planned (scope review round 1, critical review rounds 1 and 2
applied). P-1 and P-6 are done; P-2, P-3 and P-5 are open, with fallbacks below.

Phase id: `a`. Plan branch: `plan/phase-a` (PR to `develop`). Phase branch:
`integrate/phase-a`, cut from `develop`. Sprint branches
`sprint/a-<n>-<slug>`, cut from `integrate/phase-a`.

## Intent

Build every bd-cache crate that does not depend on an sc-runtime crate
(sc-config, sc-transport, sc-command, sc-runtime are being built in
parallel), so the most deliverables are done before integration: the cache
model, the shared beads backend crates (`sc-beads` traits, `sc-bd-cli`
implementation; ADR-BDC-0007), the SQLite store, the core library, a cron
prototype `sc-cron` (ADR-BDC-0006), and the sync task on it.

Baseline contract: [`docs/baseline/BD-CACHE.md`](../../baseline/BD-CACHE.md),
vendored from randlee/atm-core#2219 at `015bdfb`. Requirements are drawn from
it into [`docs/requirements.md`](../../requirements.md); sprint docs cite
requirement ids, not the baseline.

## Non-goals

- The `bdc` / `bd-cache` binary (needs sc-command and sc-config).
- The MCP server, the `sc-bd-http` adapter, DoltHub-specific adapters.
- Python bindings (maturin / PyO3) for the CLI operations; planned after
  sc-runtime integration (see Target Design in `docs/architecture.md`).
- Bead creation and comments in the cache journal (out of BD-CACHE v1).

- Moving `sc-cron` to sc-runtime; publishing `sc-beads` / `sc-bd-cli` to
  crates.io. Phase A makes them publish-ready (`cargo publish --dry-run`).

## Prerequisites

| Id | Prerequisite | Blocks | Fallback if open at dispatch |
|---|---|---|---|
| P-1 | Done: requirement ids drawn from the baseline and planning. | — | — |
| P-2 | Agreement on randlee/atm-bd-orchestration#557 for the additive v3 extensions (ADR-BDC-0001, 0002, 0003, 0005). fenix (2026-10-24): fine with all four; proposes the ADR-BDC-0005 outage rule for v3 and would move the Python schema to match once 0001 and 0005 are agreed. Rand decides. | a-4 schema freeze | a-4 ships the extensions as additive tables at `user_version` 3 anyway and tolerates rows the Python implementation leaves in them; no version bump (it would break REQ-BDC-0804). |
| P-3 | beads-task-issue-tracker agreement on the shared API: sync calls, `BeadsError` as the shared error type, tolerant and strict rows. btit's traits stay unchanged either way; the cache methods are on `CacheBackend: BeadsBackend`. | a-1 error and row freeze | a-1 keeps btit's `BeadsError` and adds the cache variants; recorded in ADR-BDC-0007. |
| P-4 | sc-runtime team told about `sc-cron` (name, eventual home). | nothing | — |
| P-6 | Done 2026-10-24 (`docs/research/bd-blocked-state-probe.md`): list rows carry no `is_blocked`; `bd blocked -l` gives the blocked set; `--id` ∩ `--updated-after` works; closed beads need the status list. Set ADR-BDC-0002. | — | — |
| P-5 | Owner to confirm (not fenix; Rand or the hermes fork-maintenance owner). The beads-release trigger that drives the hermes team's fork maintenance also sends `repository_dispatch` (`beads-release`, `{"version": ...}`) to randlee/beads-cache. | proving a-2's workflow with a real release | a-2 proves the workflow with `workflow_dispatch`. |

## Boundary map

No crate exists yet. Each crate is one boundary; no boundary manifests are
declared, so boundaries are reviewed against `docs/architecture.md`.

| Boundary | Produced by | Contract | Consumed by |
|---|---|---|---|
| `sc-beads` | a-1 | backend traits, rows, `BeadsError`, `FakeBackend` | a-2, a-4, a-5, a-6, a-7, btit |
| `bd-cache-model` | a-1 | journal and cache types, `CacheStore` port and `FakeStore`, `EscalationSender` port, `CacheError` | a-4, a-5, a-6, a-7 |
| `bd-cache` facade | a-1 (skeleton) | `Cache` struct and module layout; method bodies filled by a-5 and a-6 | a-7 |
| `sc-bd-cli` | a-2 | implements `sc-beads` traits over `bd` | a-7, btit |
| `sc-cron` | a-3 | scheduler, schedule-store port, rusqlite store | a-7 |
| `bd-cache-store` | a-4 | implements `CacheStore` over SQLite | a-7 |
| `bd-cache-sync` | a-7 | sync task | later `bdc` and daemon |

Normative signatures: [a-1](sprint-a-1-contracts.md#public-contract) and,
for `sc-cron`, [a-3](sprint-a-3-sc-cron.md#public-contract).

## Shared files

| File | Owner | Rule |
|---|---|---|
| `Cargo.toml` (workspace, `members = ["crates/*"]`, `[workspace.dependencies]` for every third-party crate the phase uses) | a-1 | Wave-2 sprints use `workspace = true` and edit only their own crate's `Cargo.toml`. A missing workspace dependency is an amendment to a-1, merged before the sprint that needs it. |
| `Cargo.lock` | a-7 | a-1 adds `/Cargo.lock` to `.gitignore`; a-7 removes that line and commits the lock. |
| `.gitignore`, `rust-toolchain.toml` | a-1 | a-7 edits `.gitignore` once (the lock line). |
| `docs/architecture.md` | a-1 (wave 1), a-7 (wave 3) | Wave-2 sprints never edit it. Crate-local decisions go in the crate's own `docs/<crate>/architecture.md` (policy: per-crate docs); a-7 updates ADR-BDC statuses and Crate Layout from them. |
| `docs/<crate>/**` | the sprint that creates the crate | a-1 owns `docs/bd-cache/requirements.md` and `architecture.md` (index); a-5 owns `docs/bd-cache/write-path.md`, a-6 owns `docs/bd-cache/read-path.md`, each carrying that path's REQ-CORE and ADR-CORE entries. |
| `.github/workflows/ci.yml` | a-1 | Workspace gates; a-2 adds only `bd-compat.yml`. |
| `.claude/project/quality-policy.md` | a-7 | Adds the phase-end validation command. |
| `crates/bd-cache/Cargo.toml`, `src/lib.rs`, `src/view.rs`, `src/backend_check.rs`, `src/miss.rs`, `src/outage.rs`, `tests/common/**` | a-1 | a-5 and a-6 own only their module files and test files. A defect found in an a-1 helper is an amendment to a-1, merged first. |
| `docs/plans/phase-a/evidence/**` | a-7 | Testbed evidence; an unavailable testbed is recorded as deferred evidence with an owner and date. |
| `docs/requirements.md` | plan branch only | Sprints do not edit requirements. |

## Sprints and waves

| Sprint | Closure | Target boundary | Wave | Track | must_follow |
|---|---|---|---|---|---|
| [a-1](sprint-a-1-contracts.md) | contract | `sc-beads`, `bd-cache-model`, `bd-cache` skeleton | 1 | contracts | — |
| [a-2](sprint-a-2-sc-bd-cli.md) | boundary (implementer) | `sc-bd-cli` | 2 | backend | a-1 |
| [a-3](sprint-a-3-sc-cron.md) | boundary | `sc-cron` | 1-2 (no inbound edge) | cron | — |
| [a-4](sprint-a-4-store.md) | boundary (implementer) | `bd-cache-store` | 2 | store | a-1 |
| [a-5](sprint-a-5-core-write.md) | boundary (consumer) | `bd-cache` write path | 2 | core | a-1 |
| [a-6](sprint-a-6-core-read.md) | boundary (consumer) | `bd-cache` read path | 2 | core | a-1 |
| [a-7](sprint-a-7-sync-integration.md) | integration | `bd-cache-sync` and composition | 3 | integration | a-2, a-3, a-4, a-5, a-6 |

- Critical path: 3 sprints (a-1 → any wave-2 sprint → a-7).
- Width: 5 (a-2, a-3, a-4, a-5, a-6 together).
- Sprint count: 7. Tracks: 6. `must_follow` edges: 9. Every wave-2 pair is
  `parallel_safe`; a-3 has no inbound edge and may start in wave 1.
- Testbed-only evidence never gates a boundary sprint; it is a-7's, with a
  deferred-evidence outcome when the testbed is unavailable.

## Requirement ownership

Each requirement has exactly one owner. Sprint docs list `owns` and
`constrained_by`.

| Owner | Requirements |
|---|---|
| a-1 | REQ-BDC-0007, 0105, 0301, 0304, 0305, 0802, 0803, 0906; NFR-BDC-0005 |
| a-2 | REQ-BDC-0006, 0106, 0201, 0303, 0905; NFR-BDC-0004, 0006 |
| a-3 | none (governed by ADR-BDC-0006) |
| a-4 | REQ-BDC-0101, 0102, 0103, 0104, 0107, 0108, 0109, 0604, 0804, 0901; NFR-BDC-0007 |
| a-5 | REQ-BDC-0002, 0004, 0005, 0302, 0401, 0402, 0403, 0404, 0405, 0406 |
| a-6 | REQ-BDC-0001, 0008, 0202, 0203, 0501, 0502, 0503, 0504, 0601, 0602, 0603, 0902, 0904 |
| a-7 | REQ-BDC-0003, 0701, 0702, 0703, 0704, 0705, 0706, 0707, 0903; NFR-BDC-0001, 0003 |
| deferred | REQ-BDC-0801, 0805, 0806; NFR-BDC-0002 (the `bdc` binary, after sc-runtime) |

## Ported Python cases

The one authoritative list. Rust tests keep the Python name.

| Python case | Owner |
|---|---|
| test_pinned_model_matches_recorded_schema | a-1 |
| test_recorded_bd_rows_validate | a-1 |
| test_bad_ops_are_typed | a-1 |
| test_schema_change_rebuilds_rows_and_keeps_the_journal | a-1 |
| test_model_current_when_live_schema_matches_pin | a-1 |
| test_first_backend_check_records_schema_without_dropping_cached_rows | a-1 |
| test_backend_check_does_not_need_bd_context | a-2 |
| test_metadata_values_round_trip_through_bd_coercion | a-2 |
| test_cache_file_is_named_for_the_dolt_database | a-4 |
| test_missing_metadata_is_typed | a-4 |
| test_beads_dir_env_names_the_database | a-4 |
| test_file_from_another_project_is_refused | a-4 |
| test_newer_cache_schema_is_refused | a-4 |
| test_dependents_use_the_edge_index | a-4 |
| test_escalations_are_unannounced_until_marked | a-4 |
| test_sync_alert_round_trip | a-4 |
| test_pending_work_counts_as_open | a-4 |
| test_write_is_visible_to_reads_but_cache_row_is_untouched | a-5 |
| test_precondition_checked_at_write_time | a-5 |
| test_duplicate_idem_key_is_one_entry_and_a_conflicting_reuse_is_refused | a-5 |
| test_dep_add_overlays_then_lands_as_a_real_edge | a-5 |
| test_drain_writes_in_order_reads_back_and_marks_applied | a-5 |
| test_drain_pre_reads_all_pending_beads_in_one_call | a-5 |
| test_transient_write_failure_stays_pending_and_holds_later_entries_of_that_bead_only | a-5 |
| test_failed_pre_read_retries_every_entry | a-5 |
| test_retries_exhausted_escalate_and_never_silently_drop | a-5 |
| test_concurrent_backend_change_escalates_instead_of_overwriting | a-5 |
| test_bd_guard_refusal_exit_13_escalates | a-5 |
| test_drain_on_absent_bead_escalates_not_found | a-5 |
| test_write_that_landed_before_a_crash_is_not_repeated | a-5 |
| test_drain_is_single_writer | a-5 |
| test_drain_deadline_defers_unstarted_entries_without_attempts | a-5 |
| test_contract_violation_is_typed_and_sync_stores_nothing | a-6 |

| test_miss_fetches_once_then_serves_from_cache | a-6 |
| test_miss_on_unknown_bead_is_typed | a-6 |
| test_sync_is_incremental_and_never_replaces_a_newer_row | a-6 |
| test_sync_failure_is_typed_and_recorded | a-6 |
| test_prune_refuses_with_unapplied_writes | a-6 |
| test_resolve_closes_only_escalated_entries_and_unblocks_prune | a-6 |
| test_resolve_keeps_the_note_when_the_entry_had_no_error | a-6 |
| test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync | a-7 (spans drain and sync; a-5 and a-6 each test their half) |
| test_tick_syncs_only_phases_with_an_open_root | a-7 |
| test_escalation_is_sent_once_to_the_phase_lead | a-7 |
| test_undelivered_escalation_is_retried_and_the_tick_refuses | a-7 |
| test_sync_failure_is_announced_once_per_cause | a-7 |
| test_missing_team_is_refused | a-7 |
| test_stop_only_when_no_active_phase_and_no_open_work | a-7 |
| test_tick_budget_defers_drain_entries | a-7 |
| test_custom_phases_limits_sync_and_stop | a-7 |
| test_malformed_custom_phases_is_refused | a-7 |

Counts: 41 + 9 = 50 (a-1 6, a-2 2, a-4 9, a-5 15, a-6 8, a-7 10).

## Planted-bug checks

Each planted bug from the baseline's Tests section must fail at least one
test in its owner's suite. The baseline says 19 + 13 = 32 but lists 31
distinct bugs; the missing one is to be raised on #557.

| Planted bug | Owner |
|---|---|
| no overlay; no metadata coercion guard; skip validation; no dep-type check; skip rebuild; rebuild on first check | a-1 |
| bools passed as `True` | a-2 |
| no newer-row guard; skip project-id check; rebuild deletes journal; no `mark_announced`; no set/clear sync alert | a-4 |
| no write precondition; no drain precondition; no per-bead hold; no achieved check; no retry cap; no stale flag; no drain lock; per-entry pre-read; no not-found escalation; deferred not reported; drain without deadline; plus the added check: dedupe after the precondition | a-5 |
| prune ignores escalated; resolve accepts any state | a-6 |
| unannounced filter dropped; open-work ignores announcement; refusal dropped; team check dropped; closed roots active; stop ignores open work | a-7 |

## Items to raise on #557

Collected for one comment on randlee/atm-bd-orchestration#557 (not posted).
fenix replied on 2026-10-24 to the first summary:

- bd settles `is_blocked` without bumping `updated_at`, and list rows never
  carry it. Confirmed live; does **not** affect v3 (it never serves blocked
  state); relevant only to a cache that serves `ready` (ADR-BDC-0002).
- An outage spends retry attempts and escalates every pending write within
  about 5 ticks. fenix: a real v3 defect; proposing ADR-BDC-0005's rule for
  v3.
- Two-deadline drain rule (REQ-BDC-0402) and the additive schema extensions
  (ADR-BDC-0001, 0004). fenix: will state the 25 s budget is a start gate and
  name the hard deadline on #2219.
- Planted-bug lists name fewer items than their counts, and the `unmet`
  shape is `{key: {expected, actual}}` in code. fenix: will fix both on
  #2219; re-vendor the baseline after.

## bd release compatibility

beads-cache validates rows strictly and keeps cache files across bd upgrades,
so it is more sensitive to bd changes than btit. a-2 adds a CI workflow that
runs on every beads release (P-5): it checks the new bd against the pinned
fingerprint, the contract tests and the live probes, and opens an issue on
any break. The fix is one upgrade of the shared crates, released as a
semver-major version that every consumer takes (ADR-BDC-0007).

## QA consumption

Each sprint doc lists `requirements.owns`, `requirements.constrained_by`,
`adrs`, deliverables tagged with the requirement they serve, acceptance
criteria with roots (`boundary:<crate>` or `ADR-BDC-nnnn` in boundary
sprints; `req:<id>` in a-7), and `owned_paths`. Reviewers per
`.claude/project/quality-policy.md`.
