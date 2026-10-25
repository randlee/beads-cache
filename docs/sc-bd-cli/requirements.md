# sc-bd-cli Requirements

Status: approved (lead ruling, bc-lead, 2026-10-24). Crate-local decisions (domain code `BDCLI`).
Crate-local requirements (domain code `BDCLI`) refining the repo-level ids in
[`../requirements.md`](../requirements.md). Created by sprint a-2.

## REQ-BDCLI-0001: btit's CLI transport and bd backend are kept

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`sc-bd-cli` holds `btit-cli` (invoker, runner, probe, per-project locks, issue
operations) and `btit-bd` (`BdCli`, dolt-mode detection) as they are at
randlee/beads-task-issue-tracker `8d8e82ae9f85`: the same public items, the same
argv for every btit operation, the same parity and API-freeze tests. The
differences are the imports (`sc_beads::`), logging through `tracing`, the
feature name `test-support` kept as btit has it, and the additions listed in
ADR-BDCLI-0002.

**Refines:** REQ-BDC-0906, ADR-BDC-0007
**Acceptance evidence:** the ported `tests/cli_api_freeze.rs`,
`tests/bd_api_freeze.rs`, `tests/parity.rs`, `tests/ops_argv.rs`,
`tests/ops_behaviour.rs`, `tests/dolt_mode.rs`, `tests/resolve_working_dir.rs`
and the unit tests of the ported modules.

## REQ-BDCLI-0002: The cache's calls use the baseline command forms

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`BdCli` implements `sc_beads::CacheBackend` over the `bd` CLI. Reads are
`bd version`, `bd schema` (never `bd context`), `bd list --id a,b,... --status
<all seven> -n 0 --json` in chunks of at most 100 ids, `bd list -l <label>
--status <all seven> -n 0 [--updated-after <cursor>] --json`, `bd blocked -l
<label> --json` and `bd ready -l <label> -n 0 --json`; writes are `bd close <id>
[--reason R] --actor A`, `bd update <id>` with the patch fields in declaration
order, `--if-status` / `--if-assignee` and `--actor`, and `bd dep add <id> <on>
--type T --actor A`. Metadata values are passed as `MetaValue::to_bd_arg` text
(bools as `true` / `false`). `bd show` is never called.

**Refines:** REQ-BDC-0006, REQ-BDC-0106, REQ-BDC-0201, REQ-BDC-0303
**Acceptance evidence:** `recorded_argv_of_every_read_matches_the_baseline_forms`,
`recorded_argv_of_every_write_matches_the_baseline_forms`,
`list_by_ids_is_chunked_deduplicated_and_names_all_seven_statuses`,
`test_backend_check_does_not_need_bd_context`,
`test_metadata_values_round_trip_through_bd_coercion` (all `tests/cache_calls.rs`);
the `argv` unit tests; a-1's backend contract suite (`tests/backend_contract.rs`).

## REQ-BDCLI-0003: Every cache call stops at its deadline

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A cache call starts no `bd` when its `Deadline` has passed. Otherwise the child
runs for at most the time the deadline leaves, counting the wait for the
per-project lock; at the deadline the child's whole process group (on Windows, its
job object) is killed and, after that, the child reaped, both within `KILL_BOUND` and under the project lock (a child not
reaped by then goes to a reaper thread), so no process it started outlives the lock,
and the call returns `Unavailable { timed_out: true }` whose reason names the phase (lock, child,
output). Standard output and error are drained while the child runs, so large output
cannot stall it, and each is capped at 64 MiB (over the cap is an error). The crate applies no cap
of its own (the cache owns the 30 s cap).

**Refines:** NFR-BDC-0004, ADR-BDC-0005
**Acceptance evidence:** `every_call_stops_at_the_deadline_it_is_handed`
(`tests/cache_calls.rs`); `a_running_child_is_cut_at_the_deadline`,
`waiting_for_the_project_lock_is_bounded`, `an_expired_deadline_starts_nothing`,
`output_larger_than_a_pipe_is_drained`, `output_over_the_cap_is_an_error`,
`kill_and_reap_ends_a_running_child`, `standard_input_is_closed`,
`a_timed_out_child_takes_its_process_group_with_it`,
`an_output_timeout_takes_the_left_behind_helper_with_it` (RCR-001),
`two_spellings_of_a_project_share_its_lock`
(`tests/process.rs`, real processes); the unit tests
`a_child_still_running_at_the_bound_is_handed_to_the_reaper`, `exited_by_is_bounded`,
`the_exit_check_leaves_the_child_unreaped` (RCR-001)
(`bounded.rs`), `spellings_of_one_existing_directory_share_a_lock`,
`a_missing_directory_keys_on_its_lexical_form`, `a_non_utf8_canonical_path_is_refused`
(`locks.rs`); the contract rule
`every_call_stops_at_its_deadline`.

## REQ-BDCLI-0004: Failures are typed

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

A cache call's failure is one of: `Refused(Precondition)` (bd exit 13),
`Unavailable` (timeout with `timed_out`, or bd's report that the database
server is unreachable), `Btit(Spawn)` (bd cannot be started; counted by
`is_unavailable()`, with the `io::Error` in the `source` chain), `Btit(SchemaMigration)`,
`CheckFailed { step }` (a failed `version` or `schema`), `Failed { code, stderr }`
(any other nonzero exit), or `Contract` (output that breaks the strict row
contract). A guarded close is a read then a close, not atomic (ADR-BDCLI-0001). A guarded close of a bead that does not exist is `Failed`, not a
refusal.

**Refines:** NFR-BDC-0004, REQ-BDC-0007, ADR-BDC-0005
**Acceptance evidence:** `exit_13_is_a_precondition_refusal`,
`a_refused_connection_is_unavailable_and_not_a_timeout`,
`a_timeout_is_unavailable_with_timed_out_set`, `a_bd_that_cannot_start_keeps_its_cause`,
`other_failures_are_failed_with_code_and_stderr`, `a_failing_check_step_is_named`,
`a_row_that_breaks_the_contract_fails_the_whole_call_naming_bead_and_field`
(`tests/cache_calls.rs`); the same cases through real processes in `tests/process.rs`;
the `cache` unit tests.

## REQ-BDCLI-0005: Test doubles need no bd

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

With the `test-support` feature the crate offers `RecordingInvoker` (scripted
replies, recorded argv) and `FakeBd`, an invoker that parses the argv as `bd`
does (unknown flags are usage errors, `--actor` is required, `--set-metadata`
values are coerced as bd coerces them) and applies it to
`sc_beads::testing::FakeBackend`. A `BdCli` over `FakeBd` passes a-1's contract
suite.

**Refines:** NFR-BDC-0005, REQ-BDC-0006
**Acceptance evidence:** `tests/backend_contract.rs`; the `fake_bd` unit tests
(`coercion_matches_what_bd_stores`, `schema_copy_is_the_pinned_schema`).

## REQ-BDCLI-0006: Live probes run only on a scratch workspace

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`tests/live.rs` is `#[ignore]`d and returns at once unless `SC_BD_LIVE=1`; with it,
it refuses a workspace that lacks the marker `scripts/bd-compat/setup-scratch.sh`
writes. The probes check: close and reopen bump the bead's own `updated_at`;
`bd list --id` returns a closed bead only with the status list; no `is_blocked`
in list rows; the `bd blocked` shape, outside blockers and parent-child
inheritance; the cache's `ready` predicate equals `bd ready`; exit 13 on a stale
guard; metadata round trip; an unreachable server; the fingerprint.

**Refines:** NFR-BDC-0006
**Acceptance evidence:** `cargo test -p sc-bd-cli --test live` lists seven ignored
tests that pass when skipped; `scripts/bd-compat/run-live.sh` runs them (run
record in the sprint document).

## REQ-BDCLI-0007: A bd release is checked by workflow

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`.github/workflows/bd-compat.yml`, triggered by `repository_dispatch`
(`beads-release`, payload `{"version": ...}`) or `workflow_dispatch`, installs the
named bd on a clean runner (checksum-verified), compares `bd version` and the sha256
of `bd schema` with the pinned fingerprint (attaching the schema diff on a mismatch),
runs the `sc-beads` contract suite and the `sc-bd-cli` tests, then the live
probes in a scratch workspace; a failure opens or updates one issue per bd version.

**Refines:** REQ-BDC-0905
**Acceptance evidence:** proof runs 40073075092 (pinned version passes) and
40073178515 (a differing schema sha fails and opens an issue with the diff) are a-2's
closure evidence (sprint document). The real `workflow_dispatch` / `repository_dispatch`
run needs the file on `main` and is owned by a-7 (see the a-7 sprint doc).

## REQ-BDCLI-0008: Publish-ready

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

`cargo package -p sc-bd-cli --list` succeeds and the crate carries license,
description, repository and readme. A full `cargo publish --dry-run` waits until
`sc-beads` is published.

**Refines:** ADR-BDC-0007
**Acceptance evidence:** `cargo package -p sc-bd-cli --list` (sprint document).

## REQ-BDCLI-0009: A runner gives its children the variables it was told to

**Status:** Approved (lead ruling S47, bc-lead, 2026-10-24; fix-a-7-14, ARCH-903; the hook is lead ruling S40); forwarding rule narrowed by lead ruling S52 and the name rule stated by lead ruling S50 (fix-a-7-15)

A variable set (`Some`) or removed (`None`) through `CliInvoker::set_child_var`,
or the `CliRunner::with_child_var` / `BdCli::with_child_var` builders, is set
on or removed from every child `CliRunner` spawns for a project: `run_json`,
`run_raw` and `run_bounded`, on every platform. A later call for the same name
replaces the earlier one: the runner dedups names exactly on Unix and ASCII
case-insensitively on Windows, names are ASCII (asserted in debug builds), and
`std::process::Command` applies the platform's full rule, so the last call still
wins (S50). The spawn's own `PATH`
and `BEADS_PATH` still win, and the `--version` probe does not get them.
Without such a call the child inherits the environment, as in btit. An invoker
that wraps one which spawns processes or acts on child variables forwards the
call; a wrapper over a no-op invoker (one that spawns nothing and keeps the
provided no-op body, such as the test wrapper `Shared(Arc<RecordingInvoker>)`)
is exempt (S52). btit's public spawn functions and the
API-freeze tests are unchanged. Design: ADR-BDCLI-0004, which amends
ADR-BDCLI-0002.

**Refines:** REQ-BDCLI-0001, REQ-BDC-0101 (the `$BEADS_DIR` the cache file is located from reaches `bd` too; S40, S44). It does not refine REQ-SYNC-0010: a lower crate's requirement does not refine a consumer's (S52)
**Acceptance evidence:** `the_child_environment_reaches_run_raw_and_run_bounded`,
`the_child_environment_reaches_run_json`,
`the_child_environment_reaches_every_spawn_on_every_platform`,
`without_a_child_variable_the_environment_is_inherited` (`src/runner.rs`);
`a_later_set_of_a_name_replaces_the_earlier_one`,
`names_compare_as_the_platform_environment_does` (the dedup),
`a_non_ascii_name_is_refused_in_debug_builds` (S50) (`src/command.rs`); the
unchanged `crates/sc-bd-cli/tests/cli_api_freeze.rs`,
`crates/sc-bd-cli/tests/bd_api_freeze.rs` and
`crates/sc-bd-cli/tests/cache_api_freeze.rs`.
