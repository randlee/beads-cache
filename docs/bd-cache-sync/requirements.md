# bd-cache-sync Requirements

Status: Approved (lead ruling, bc-lead, 2026-10-24), with the corrections of QA round 1 (fix-a-7-1). Crate-local requirements (domain code `SYNC`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Created by sprint a-7.

Test files are under `crates/bd-cache-sync/tests/`. The ported cases of
`test_bd_cache_cron.py` keep their Python names.

## REQ-SYNC-0001: The tick is a library entry point

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by ADR-SYNC-0008

`tick(&mut Cache, &TickConfig, &dyn EscalationSender, Deadline)`
runs one tick that must finish by the deadline `T`, on the cache's own clock, and returns a `TickReply`:
`pass-through`, or `refused` with the list of what was refused. It is
independent of the scheduler: `sc-cron` drives it through `SyncJob`, and an
external scheduler (the first consumer's cron module through the later
`bdc tick`) can call it. Only one ticker may run per cache file. The `[tick]` keys come from
`RepoConfig.tick` (`load_repo_config`, bd-cache-store).

*Amended by ADR-SYNC-0008 (Rand, 2026-10-26): the built-in run is a library function that takes the cache, the row's `TickBudget` and a held `TickLease`, and returns a reply recording each step (lease, drain, each scope's result, hook) with no `refused` or `pass-through`; there is no sender and no `[tick]` key. `cron run`, `cron run --once`, `bdc cache tick` and `Cache.tick()` call it (REQ-BDC-0720).*

**Refines:** REQ-BDC-0701, 0703
**Acceptance evidence:** `test_missing_team_is_refused`,
`test_tick_syncs_only_phases_with_an_open_root` (`tests/tick_phases.rs`);
the doctests of `tick` and `schedule_spec`.

*Implementation note (sprint b-8):* `run_builtin` (`src/run.rs`); evidence `a_run_syncs_a_label_scope_and_an_all_scope_by_their_filters_and_frees_the_lease`, `a_live_lease_skips_the_run_and_an_expired_one_is_taken_over` (`tests/builtin_run.rs`). The phase-a evidence above was removed with the tick.*

## REQ-SYNC-0002: Phases come from phase files and the configured list

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); **Superseded** by REQ-BDC-0721 (scopes are configured bd filters; no phase files are read) (Rand, 2026-10-26); its code and tests are removed by sprint b-8

A phase is a phase file, named for its phase, in `[tick] phases_dir` (default:
the first consumer's own directory, relative to the working directory;
`compose` resolves it against the repository root) with a `root` bead and an
optional recipient key; other keys are ignored. `[tick] phases` limits the phases
(names with or without the `phase-` prefix). The schema was confirmed against the baseline Python implementation's cron module at the baseline at `05831df`, and two real phase files of the first consumer are test fixtures (ADR-SYNC-0007). A malformed entry, or an empty list, refuses the
tick before any work; an unusable file, an unreadable directory or a
configured phase with no file is listed as refused while the phases that are
usable still run. A phase is active while its root is not closed: a root the
cache holds in the phase's scope is read from the cache, every other root in
one `bd list --id` under the miss-fill timeout. A root that cannot be read, or
that bd does not know, makes its phase inactive for the tick; both are
reported as unreadable roots, and neither counts as closed for the stop rule
(lead ruling S23, ADR-SYNC-0004).

**Refines:** REQ-BDC-0702
**Acceptance evidence:** `test_tick_syncs_only_phases_with_an_open_root`,
`test_custom_phases_limits_sync_and_stop`,
`test_malformed_custom_phases_is_refused`,
`a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase`,
`a_configured_phase_without_a_file_and_a_broken_file_are_refused_but_the_rest_runs`,
`a_root_that_bd_does_not_know_is_no_open_root_and_is_reported`,
`a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs`
(`tests/tick_phases.rs`).

Amendment (phase-end fix, PE-RSH-010): the caps on the phases directory
cannot hide a configured phase. With `[tick] phases` set, the directory is
opened (so a missing one is the same `phases_dir_unreadable`) but not scanned:
each configured phase's file is looked up by name, however many files the
directory holds. Without it, the scan visits at most `MAX_PHASES_DIR_ENTRIES`
(1024) entries of any kind and reads at most `MAX_PHASE_FILES` (256) phase
files; past the entry cap it reports `PhaseFileProblem::TooManyEntries`
(`phase_files_too_many`). Evidence:
`every_configured_phase_is_read_from_a_directory_over_the_file_cap` (300 phase
files, all configured, all read),
`the_scan_of_the_phases_directory_visits_a_bounded_number_of_entries`
(`tests/tick_phases.rs`).

## REQ-SYNC-0003: One schedule per repository, with its message-routing settings

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); **Superseded** by REQ-BDC-0720 (the built-in sync row; no message-routing settings) (Rand, 2026-10-26); its code and tests are removed by sprint b-8

`schedule_spec` builds the repository's one schedule (`bd-cache:<cache file>`,
every 30 s, 25 s budget). The consumer's two message-routing settings in `[tick]`
(its recipient group and its fallback recipient) are both required, non-blank
and addressable (no whitespace, no `@`, no leading `-`);
without them `schedule_spec` and `tick` refuse, and a refused tick makes no
`bd` call and sends nothing.

**Refines:** REQ-BDC-0701
**Acceptance evidence:** `test_missing_team_is_refused` (`tests/tick_phases.rs`).

Amendment (phase-end fix, PE-QA-003): a missing phases directory has one
code on both surfaces: `ScheduleError::PhasesDir` reports the tick's
`phases_dir_unreadable` (it was `phases_dir_missing`) with the same fix.
Evidence: `a_missing_phases_directory_has_one_code_for_the_schedule_and_the_tick`
(`tests/tick_phases.rs`).

## REQ-SYNC-0004: The tick budget

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by REQ-BDC-0703's phase-B amendment

A tick drains with `DrainBudget { start_gate: T - 5 s, hard: T - 2 s }`, then
syncs each active phase with deadline `T`. A phase whose sync has not started
by `T - 2 s` is deferred, which is not a refusal. With every `bd` call timing
out, no tick takes longer than 25 s.

*Amended (Rand, 2026-10-26): the budget is the built-in row's `TickBudget`; the drain and the scope syncs get the sync deadline and the hook keeps `hook_reserve_ms` (ADR-SYNC-0008, `Budget`).*

**Refines:** REQ-BDC-0703, NFR-BDC-0004 (constraint)
**Acceptance evidence:** `test_tick_budget_defers_drain_entries`,
`no_tick_exceeds_25_s_with_every_bd_call_timing_out`,
`the_drain_runs_before_the_sync` (`tests/tick_budget.rs`).

*Implementation note (sprint b-8):* `Budget` (`src/control.rs`); evidence `a_sync_that_uses_its_whole_share_leaves_the_hook_its_reserve`, `a_hook_past_its_deadline_is_killed_with_its_tree_and_the_run_finishes` (`tests/hook_runner.rs`).*

## REQ-SYNC-0005: Threading of the job

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`SyncJob` holds the `Cache` in an `Arc<Mutex<_>>` and runs every tick and
stop check in `tokio::task::spawn_blocking`, with the run's deadline as `T`.
A tick that finds the mutex held (including by a blocking task left over from
a timed-out tick) is skipped, reported busy, and the scheduler records it as
refused; `should_stop` answers `Continue` when busy. A poisoned mutex is
recovered, cleared, and reported once in the tick reply.

**Refines:** REQ-BDC-0703, ADR-CORE-0001 (constraint)
**Acceptance evidence:**
`a_second_tick_while_a_blocking_sync_is_running_is_skipped_and_reported_busy`,
`a_blocking_task_left_over_from_a_timed_out_tick_makes_the_next_tick_busy`,
`should_stop_returns_continue_when_busy`,
`a_poisoned_mutex_drops_the_cache_and_the_next_tick_recomposes`
(`tests/job_threading.rs`).

Amendment (phase-end fix, S55, PE-RBP-F007, PE-RSH-004): a poisoned mutex's
cache is dropped, never reused; the next tick recomposes it from the same
composition inputs (`SyncJob::with_recompose`) and reports the poison once
(ADR-SYNC-0002 amendment). Shutdown waits for the tick in flight, with a
bound: `SyncJob::join_idle(deadline)` after `WorkerHandle::shutdown`.
Evidence: `a_tick_that_panics_in_the_backend_is_not_reused_and_the_next_run_recomposes`,
`a_job_without_a_recompose_function_reports_the_cache_lost_after_a_panic`,
`a_recompose_that_fails_is_tried_again_by_the_next_tick`,
`join_idle_waits_for_the_tick_in_flight_up_to_its_deadline`
(`tests/job_threading.rs`).

## REQ-SYNC-0006: Announcements are sent through the sender port and retried until delivered

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); **Superseded** by REQ-BDC-0727 (the cache sends nothing; the post-sync hook gets the results and marks what it handled) (Rand, 2026-10-26); its code and tests are removed by sprint b-8

Every message goes through `EscalationSender`. A sync failure whose cause
differs from the phase's `sync_alert` is sent to the phase's recipient (the
fallback recipient when the phase has none) and recorded; a success clears the
alert; a cause that is unreachability is the outage's to announce. Every
escalated entry with no `announced_at` is sent to its phase's recipient with the
`resolve` command, then marked; an entry with no scope goes to the fallback
recipient; an entry whose phase has no recipient is refused. A message not delivered
leaves its marker unset, makes the reply `refused` with the list, and is sent
again on the next tick.

**Refines:** REQ-BDC-0704, 0705, 0706
**Acceptance evidence:** `test_sync_failure_is_announced_once_per_cause`,
`test_escalation_is_sent_once_to_the_phase_lead`,
`test_undelivered_escalation_is_retried_and_the_tick_refuses`,
the no-scope escalation case (an entry with no scope goes to the fallback recipient and the stop rule still fires; its test name is in git history),
`an_entry_whose_phase_has_no_lead_is_refused_every_tick_until_it_has_one`,
`an_undelivered_sync_failure_is_refused_and_retried_not_recorded` (`tests/announce.rs`).

## REQ-SYNC-0007: An outage is announced once per scope through the sync alert

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); **Superseded** by REQ-BDC-0727 and REQ-BDC-0728 (an outage is recorded per scope and given to the hook) (Rand, 2026-10-26); its code and tests are removed by sprint b-8

While the cache records an outage, each active scope whose alert does not
read `backend_unavailable` has its recipient told once (one message per distinct
recipient, covering that recipient's scopes) and the alert set. When no phase is known
active (an uncached root that cannot be read), every configured phase is
told. A scope without a recipient uses the fallback recipient. When the outage ends the
alerts that read `backend_unavailable` are reset and no message is sent. An
undelivered message sets no alert and is retried. The outage is cleared only
by a list that answers (a-1's `call`), never by the backend check.

**Refines:** REQ-BDC-0704, 0903, ADR-BDC-0005
**Acceptance evidence:** `test_backend_outage_is_announced_once_and_sync_failures_are_not`,
`test_outage_with_uncached_roots_still_tells_every_configured_lead`,
`test_outage_is_not_cleared_while_only_the_backend_check_answers`,
`test_undelivered_outage_announcement_is_retried`,
the no-recipient outage case (a scope without a recipient uses the fallback recipient, and one recipient hears once; its test name is in git history),
`during_an_outage_of_several_ticks_reads_report_it_writes_wait_and_recovery_drains`,
`an_outage_alert_in_bd_cache_pys_form_is_not_announced_again`
(`tests/outage.rs`); `an_outage_of_several_ticks_through_the_real_store_and_cli` (`tests/e2e.rs`).

## REQ-SYNC-0008: The stop rule never stops on uncertainty

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); **Superseded** by REQ-CRON-0005 (no consumer stop rule; sc-cron's stop hook and `enabled` flag) (Rand, 2026-10-26); its code and tests are removed by sprint b-8

`stop_reason` stops the schedule when no phase is active and no journal entry
is pending or escalated-and-unannounced. It answers "keep going" when the
`[tick]` table would be refused, the phases directory or a phase file cannot be
read, a root cannot be read or bd does not return it (lead ruling S23), or the
store fails.

**Refines:** REQ-BDC-0707
**Acceptance evidence:** `test_stop_only_when_no_active_phase_and_no_open_work`,
`a_phase_whose_root_cannot_be_read_never_ends_the_schedule`,
`configuration_the_tick_would_refuse_never_ends_the_schedule`,
`a_root_bd_does_not_return_never_ends_the_schedule` (`tests/stop_rule.rs`).

## REQ-SYNC-0009: The consumer's send command is bounded and carries the message on stdin

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); **Superseded** by REQ-BDC-0723 (no send command; the hook runs isolated) (Rand, 2026-10-26); its code and tests are removed by sprint b-8

The sender runs the first consumer's send command, addressed to a
recipient in the configured group, sending as the consumer's cron identity
(the reference's invocation, ADR-SYNC-0007; the exact command line is in git
history), and writes the message to stdin. An address that could be read as an option, or
split in two, is refused before a process starts. A child still running after
the timeout (10 s) is killed and the send fails; a nonzero exit fails with the
child's stderr.

**Refines:** REQ-BDC-0704..0706
**Acceptance evidence:** the sender's process tests (their own fake send command, 13 cases; the file name is in git history):
`delivers_the_body_on_stdin_with_the_reference_invocation`,
`a_large_message_is_not_limited_by_the_argument_list`,
the failing-send-command case (a send command that fails is a send error that says why),
`output_beyond_the_limit_is_dropped_and_the_error_says_so`,
`an_address_that_could_be_read_as_an_option_is_refused_before_any_process_starts`,
`a_missing_binary_is_a_send_error_and_does_not_resolve`,
`a_timeout_that_no_message_could_use_is_refused`.

## REQ-SYNC-0010: The end-to-end flow runs through the real composition

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by ADR-SYNC-0008

A write journaled through the core lands in `bd` via the drain a scheduler
runs, verified by read-back, through `SyncJob`, `Cache`, `SqliteStore`,
`FlockDrainLock` and `BdCli` (the `bd` process replaced by `FakeBd` in CI,
a scratch `bd` workspace on the testbed). A failed read-back marks the entry
applied and the row stale and holds the bead until a sync. A journaled close or
reopen of a blocker, inside or outside every scope, changes its depender's
readiness in the same tick's sync.

*Amended by ADR-SYNC-0008 (Rand, 2026-10-26): the scheduler runs the built-in row through the per-row resolver, and the end-to-end flow also covers the Python row run by `cron run --once` (sprint b-8).*

**Refines:** REQ-BDC-0003, 0903, 0904
**Acceptance evidence:**
`a_write_journaled_through_the_core_lands_in_bd_via_the_scheduled_drain`,
`test_failed_read_back_marks_applied_and_stale_and_holds_the_bead_until_sync`,
`test_e2e_blocker_close_and_reopen_update_ready` (`tests/e2e.rs`); `tests/live.rs`
(testbed, composed with `compose_with(..., None)` like `tests/common/real.rs`,
ARCH-807).

Amendment (fix-a-7-10 and fix-a-7-12, S34, S37, S38, RBP-F044, QA8-003, REQ-QA-709; **Status:** Approved (lead rulings S34, S37 and S38, bc-lead, 2026-10-24)):
a `.beads/bd-cache.toml` that cannot be loaded is a `ComposeError::Config`
whose code is this crate's, mapped from the `ConfigLoadError` variant:
`Read` is `sync_config_unreadable`, `Parse` `sync_config_invalid_toml`,
`Scope` `sync_config_bad_scope` (S34). The loader keeps its own
store-owned codes, `store_config_unreadable` / `store_config_invalid` /
`store_config_bad_scope` (S37), and its fix text, which `ComposeError::fix`
returns. The mapping is an exhaustive match with no wildcard arm, since
`ConfigLoadError` is not `#[non_exhaustive]` (S38): a variant the store adds
fails to compile here until it has a code. Evidence:
`compose_errors_have_codes_and_sources` (`tests/compose.rs`: each of the
three variants, its `sync_config_*` code, the loader's `store_config_*` code
and fix, and a missing `metadata.json` as the cache error's
`BD_CACHE.NO_DATABASE`), and
`a_relative_phases_dir_and_the_default_are_rooted_at_the_repository`.

Amendment (fix-a-7-12, S40, RSH-033, RBP-F046; **Status:** Approved (lead ruling S40, bc-lead, 2026-10-24)):
`compose_with` rebinds the backend to the clock it is given
(`BdCli::with_clock`), so no precondition on how the caller built it remains,
and gives every `bd` the backend spawns the `$BEADS_DIR` the cache resolved
from: set to the explicit value, or removed when it is `None`
(`BdCli::with_child_var`, ADR-BDCLI-0004). `compose` passes the
environment's value, so its children see what they always saw. Evidence
(`tests/compose.rs`, Unix: the backend runs `/bin/sh`, which prints the
`BEADS_DIR` it was given):
`the_bd_child_sees_the_explicit_beads_dir_not_the_environments` (an explicit
directory that is not the environment's) and
`the_bd_child_has_no_beads_dir_when_the_explicit_value_is_none_though_the_environment_sets_one`
(runs in a child process of the test binary whose environment sets
`BEADS_DIR`).

Amendment (fix-a-7-14, S44, RSH-035, QA9-003, FTQ-031; **Status:** Approved (lead ruling S44, bc-lead, 2026-10-24)):
`compose_with` normalizes the explicit `$BEADS_DIR` once and gives that one
value to the store (`SqliteStore::open_with`) and to every `bd` child
(`BdCli::with_child_var`): an empty value is `None` (unset for both; the
child no longer gets `""`), and a relative value is made absolute against the
process's working directory, so the store and the child, which runs in the
repository, name one directory. For `compose`, which passes the
environment's value, this changes what a `bd` child sees only for an empty or
relative `$BEADS_DIR` (S40's "what they always saw" holds for an absolute or
unset one). Evidence (`tests/compose.rs`, Unix):
`an_empty_explicit_beads_dir_is_unset_for_the_bd_child` and
`a_relative_explicit_beads_dir_names_one_absolute_directory_for_the_store_and_the_bd_child`
(a child process of the test binary whose working directory is not the
repository: the store reads `metadata.json` there and the child sees the
absolute path); the unit tests `an_unset_or_empty_beads_dir_is_no_override` and
`a_relative_beads_dir_is_made_absolute_against_the_working_directory`, which
were in `src/compose.rs` and moved with the rule to `bd-cache-store`'s
`src/location.rs` (S49 amendment below). The two child-process cases drain the child's output on
reader threads, bound it by 120 s and print it on failure (FTQ-031). The
clock rebinding of S40 is pinned by
`the_composed_backend_measures_deadlines_on_the_composition_clock` (a
backend on the system clock, composed with a manual clock an hour ahead: a
deadline passed on the manual clock is refused without a spawn).

Amendment (fix-a-7-15, S48, S49, QA10-001, QA10-003, RSH-038, RBP-F047, RBP-F048; **Status:** Approved (lead rulings S48 and S49, bc-lead, 2026-10-24)):
the normalization of S44 lives in one validated type,
`bd_cache_store::BeadsDirOverride` (ADR-STORE-0003 amendment, S48/S49):
`compose_with` builds it once from its `beads_dir_env` and passes the same
value to `SqliteStore::open_with` and its `as_os_str()` to
`BdCli::with_child_var`. It fails closed (S48): a relative value that cannot
be made absolute (the working directory cannot be read) fails the
composition with `ComposeError::BeadsDir` (the store's `BeadsDirError`
whole since the phase-end fix, RBP-F050) before the
configuration is read or the store opened, with the stable code
`sync_beads_dir_unresolvable`, the fix "set an absolute BEADS_DIR, or run
from a readable working directory", and the `io::Error` as `source()`; the
earlier fallback to the relative value is gone. Evidence:
`an_unresolvable_beads_dir_fails_the_composition_before_the_store_opens`
(`src/compose.rs`: the error injected through the private seam
`compose_with_override`; no cache file is created) and the store's
`a_beads_dir_that_cannot_be_made_absolute_is_an_error_with_its_cause` (the
`absolute` step injected to fail). A child-process case whose output
reader does not finish within 10 s after the child ends prints
`<stdout reader did not finish within 10s>` (or the stderr one) in that
stream's place, not an empty output (QA10-003, `tests/compose.rs`).

Amendment (phase-end fix, S54, PE-RSH-003): `compose` builds every `BdCli`
over one process-wide `ProjectLocks` (a `OnceLock` static in `compose.rs`, as
`bd-cache-py` keeps its own), so two compositions of one project (two cache
files, or a cache recomposed after a panic) never run `bd` at once.
`compose_with` takes the set as an `Arc<ProjectLocks>` and the backend as a
function of it, so a host shares its own set; `sc-bd-cli` normalizes the
lock key. The `ProjectRef` text is the repository root unchanged: a root that
is not UTF-8 is `ComposeError::RepoRootNotUtf8` (`sync_repo_root_not_utf8`),
never a lossy copy. Evidence: `two_compositions_of_one_project_never_run_bd_at_once`
(two cache files of one repository, each `bd` child a script that marks an
overlap when another holds its directory), `compose_hands_out_one_lock_set`
(`Arc::ptr_eq`), `a_repository_root_that_is_not_utf8_is_refused`
(`src/compose.rs`).

*Implementation note (sprint b-8):* the real composition with `CacheScheduleStore` and sc-cron's `ManualClock` (`tests/common/repo.rs`): `tests/schedule_switch.rs`, `tests/cron_rows.rs`; with a real interpreter, `crates/bd-cache-py/tests/test_run.py` and `test_switch.py`.*

## REQ-SYNC-0011: Messages are sent inside the tick's budget

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); **Superseded** by REQ-BDC-0727 (no messages; the hook keeps `hook_reserve_ms`) (Rand, 2026-10-26); its code and tests are removed by sprint b-8

Before each message the tick checks its stop flag and the time left before
`T`; the sender is given that time (`send_within`) and never runs longer. With
less than half a second left the message is not started: it is counted in
`TickReply::unsent`, stays unmarked and the next tick sends it. A tick whose
sender takes 9 s per message ends within 25 s.

Amendment (fix-a-7-5, RSH-027, REQ-QA-501; **Status:** Approved (lead ruling S27, bc-lead, 2026-10-24)):
"the time left" is the time before `T` less the send reserve `SEND_RESERVE`,
1.5 s, so a message is not started with less than 2 s before `T`. The reserve
covers the most a send can run past its deadline: the read grace
(250 ms) plus one shared kill bound (1 s) in which the tree kill and the
reaping of the child are both done or given up on. Const assertions in
the sender module and `control.rs` tie `SEND_RESERVE` to `READ_GRACE`, `READ_FLOOR`,
`KILL_BOUND` and the poll intervals, and keep `MIN_SEND_BUDGET + SEND_RESERVE`
within the sync step's `HARD_MARGIN` (2 s). Evidence:
`a_child_that_ignores_term_and_its_descendant_are_killed_by_the_real_tree_kill`
(the group kill, generous deadline; fix-a-7-10, FTQ-025) and
`a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped` (the deadline)
(the sender module),
`a_send_is_given_the_time_left_minus_the_reserve_and_nothing_below_the_floor`
(`src/control.rs`).

Amendment (fix-a-7-8, RSH-029; **Status:** Approved (lead ruling S33, bc-lead, 2026-10-24)):
the marker written after a delivered message (`announced_at`, a sync alert)
waits for the store's write lock at most the time left before `T`, never the
store's full 5 s busy timeout. A marker not written in time fails the tick
with the store's error and leaves the message unmarked; the next tick sends it
again (at least once, ADR-SYNC-0003). Evidence: `tests/marker_wait.rs`,
`a_marker_write_may_wait_the_time_left_before_t` (`src/control.rs`),
`a_bounded_marker_write_gives_up_on_a_held_lock_within_its_wait`
(`bd-cache-store/tests/store.rs`).

Amendment (fix-a-7-10, S35, RSH-030, RSH-031, ARCH-705, QA7-002, QA7-003; **Status:** Approved (lead ruling S35, bc-lead, 2026-10-24); restated for QA8-001/RBP-F045 and lead ruling S41 (fix-a-7-12)):
the alert clears without a message (after a passing sync, at the end of an
outage) are bounded writes too: they wait at most the time left before `T`,
like the markers. The bound applies to the bounded writes only, these markers
and clears; the tick's other store writes keep the store's own busy timeout.
A clear that fails does not fail the tick: the alert stays set and the next
tick clears it. A marker write that fails after a delivered send logs a
warning and then fails the tick as above. What the bound is in time is stated
by the S39 amendment below. Evidence (`tests/marker_wait.rs`), one
failing-write test per marker and per clear (the last three added by
fix-a-7-12, QA8-001):
`a_failed_marker_write_fails_the_tick_and_the_next_tick_sends_again` (the
escalation marker),
`a_failed_sync_failure_alert_write_fails_the_tick_and_the_next_tick_sends_again`,
`a_failed_outage_alert_write_fails_the_tick_and_the_next_tick_sends_again`
(the first phase's alert of an outage message),
`a_failed_alert_clear_does_not_fail_the_tick_and_the_next_tick_clears_it`
(the clear after a passing sync) and
`a_failed_outage_over_clear_does_not_fail_the_tick_and_the_next_tick_clears_it`;
`an_alert_clear_after_a_passing_sync_waits_the_time_left_before_t` (the
clear's wait). The warnings are not asserted: the workspace has no tracing
subscriber to capture them, and the test names do not claim the logging
(lead ruling S41, QA8-002).

Amendment (fix-a-7-12, S39, RSH-032; **Status:** Approved (lead ruling S39, bc-lead, 2026-10-24); corrected by lead ruling S46 (fix-a-7-14, RSH-034)):
a bounded write (a marker after a send, an alert clear) ends by `T` plus the
bounded write's own run time (statement, commit, fsync, any automatic
checkpoint) plus scheduler jitter. The lock wait is capped at the time left
before `T` and may overshoot it by the busy handler's sleep slack, which
counts as scheduler jitter (S46). This replaces the "`T` plus the busy
handler's sleep slack (tens of ms)" of S35, which bounded only the lock wait.

Amendment (fix-a-7-14, S45, RSH-037; **Status:** Approved (lead ruling S45, bc-lead, 2026-10-24)):
when an outage is over, the tick clears its alerts one scope at a time and
stops once the time left before `T` is zero or the stop flag is set; the
alerts it did not clear stay set and the next tick clears them, silently.
Evidence:
`an_outage_over_clear_is_not_started_after_t_and_the_next_tick_clears_it`
(`tests/marker_wait.rs`) and
`a_stopped_tick_clears_no_outage_alert_and_the_next_tick_does`
(`src/announce.rs`).

Amendment (fix-a-7-14, S46, RSH-034; **Status:** Approved (lead ruling S46, bc-lead, 2026-10-24)):
"the lock wait itself never extends past `T`" (S39) is withdrawn: the busy
handler sleeps in steps, so a wait capped at the time left can end up to one
sleep step after `T`. That slack is scheduler jitter; the bound is unchanged.

**Refines:** REQ-BDC-0703, 0706, NFR-BDC-0004 (constraint)
**Acceptance evidence:** `many_escalations_and_a_slow_sender_stay_within_the_tick_budget`,
`a_stopped_tick_does_nothing_and_the_next_tick_does_it_all` (`tests/announce.rs`);
the S45 outage-over clears: `an_outage_over_clear_is_not_started_after_t_and_the_next_tick_clears_it`
(`tests/marker_wait.rs`) and `a_stopped_tick_clears_no_outage_alert_and_the_next_tick_does`
(`src/announce.rs`).

## REQ-SYNC-0012: Cooperative shutdown

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by REQ-BDC-0730

A `StopFlag` ends a running tick after the drain, before the one
`bd list --id` of the uncached phase roots, between phases and before every message;
given to the sender it kills the child and its whole process tree. A
send command that outlives its timeout takes its descendants with it.

Amendment (phase-end fix, PE-RSH-004): the host waits for the tick in flight
before it drops the job: set the flag, await `WorkerHandle::shutdown`, then
`SyncJob::join_idle(deadline)` (bounded; `false` when a tick still held the
cache at the deadline), then drop the job. Evidence:
`join_idle_waits_for_the_tick_in_flight_up_to_its_deadline`
(`tests/job_threading.rs`).

*Amended (Rand, 2026-10-26): `cron run` stops on `SIGINT` / `SIGTERM` (Ctrl-C and Ctrl-Break on Windows) through the `StopFlag`, waits up to `--shutdown-timeout-ms` and then kills the runs still going with their process trees (REQ-BDC-0730, ADR-SYNC-0008).*

**Refines:** REQ-BDC-0703
**Acceptance evidence:** `the_stop_flag_ends_a_tick_between_phases`
(`tests/job_threading.rs`); `a_set_stop_flag_skips_the_root_refresh` (`src/active.rs`);
`the_stop_flag_kills_the_child_and_its_descendants`,
the hanging-send-command case (kill and reap, stop-flag form: the
stop comes once the fakes' pid files are written) and
`the_time_a_tick_passes_bounds_the_send` (the one timer-driven kill: `TimedOut`
and its detail only) (the sender's process tests; fix-a-7-10, FTQ-026, FTQ-027).
The stop-flag tree test asserts the child reaped, not the descendant gone
(fix-a-7-12, FTQ-030): its tree kill runs under the production 1 s
`KILL_BOUND`, which a stalled host can exceed; the descendant's death is
`a_child_that_ignores_term_and_its_descendant_are_killed_by_the_real_tree_kill`
(the sender module, injected 30 s deadline).

*Implementation note (sprint b-8):* `StopFlag` (with `wait`) and the run's `stop` / `kill` flags; evidence `a_stopped_run_syncs_no_scope_and_frees_the_lease` (`tests/builtin_run.rs`), `cron_run_stops_on_its_flag_kills_the_tree_and_releases_the_lease` (`tests/cron_rows.rs`), `join_idle_is_false_while_a_step_holds_the_cache_and_true_once_it_returns` (`tests/job_threading.rs`).*

## REQ-SYNC-0013: A cached `get` costs the same at any scope size

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Owned by a-7 through NFR-BDC-0001, implemented in `bd-cache` and
`bd-cache-store` by lead ruling S4 (bc-lead, 2026-10-24): a cached `get` reads the
bead's own entry of the stored ready base (`CacheStore::base_entry`) and
adjusts the whole base only when a pending or recently applied entry could
have changed it.

**Refines:** NFR-BDC-0001
**Acceptance evidence:** `a_cached_get_touches_the_same_rows_at_10_100_and_1000_beads`,
`a_pending_entry_makes_get_adjust_the_whole_base_and_still_agree` (`tests/perf.rs`);
`base_entry_rules` (the store contract suite); release-profile medians in the sprint doc.
