# sc-bd-cli Requirements

Status: draft (awaiting lead approval; new crate ids need a ruling before merge).
Crate-local requirements (domain code `BDCLI`) refining the repo-level ids in
[`../requirements.md`](../requirements.md). Created by sprint a-2.

## REQ-BDCLI-0001: btit's CLI transport and bd backend are kept

**Status:** Draft

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

**Status:** Draft

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

**Status:** Draft

A cache call starts no `bd` when its `Deadline` has passed. Otherwise the child
runs for at most the time the deadline leaves, counting the wait for the
per-project lock; at the deadline the child is killed and reaped and the call
returns `Unavailable { timed_out: true }`. Standard output and error are drained
while the child runs, so large output cannot stall it. The crate applies no cap
of its own (the cache owns the 30 s cap).

**Refines:** NFR-BDC-0004, ADR-BDC-0005
**Acceptance evidence:** `every_call_stops_at_the_deadline_it_is_handed`
(`tests/cache_calls.rs`); `timeout_kills_a_running_child`,
`waiting_for_the_project_lock_is_bounded`, `an_expired_deadline_starts_nothing`,
`output_larger_than_a_pipe_is_drained`, `standard_input_is_closed`
(`tests/process.rs`, real processes); the contract rule
`every_call_stops_at_its_deadline`.

## REQ-BDCLI-0004: Failures are typed

**Status:** Draft

A cache call's failure is one of: `Refused(Precondition)` (bd exit 13),
`Unavailable` (timeout with `timed_out`, or bd's report that the database
server is unreachable), `Btit(Spawn)` (bd cannot be started; counted by
`is_unavailable()`, with the `io::Error` in the `source` chain), `Btit(SchemaMigration)`,
`CheckFailed { step }` (a failed `version` or `schema`), `Failed { code, stderr }`
(any other nonzero exit), or `Contract` (output that breaks the strict row
contract). A guarded close of a bead that does not exist is `Failed`, not a
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

**Status:** Draft

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

**Status:** Draft

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

**Status:** Draft

`.github/workflows/bd-compat.yml`, triggered by `repository_dispatch`
(`beads-release`, payload `{"version": ...}`) or `workflow_dispatch`, installs the
named bd on a clean runner (checksum-verified), compares `bd version` and the sha256
of `bd schema` with the pinned fingerprint (attaching the schema diff on a mismatch),
runs the `sc-beads` contract suite and the `sc-bd-cli` tests, then the live
probes in a scratch workspace; a failure opens or updates one issue per bd version.

**Refines:** REQ-BDC-0905
**Acceptance evidence:** a `workflow_dispatch` run at the pinned version passes; a
run with a differing schema sha fails and opens an issue with the diff (run links in
the sprint document).

## REQ-BDCLI-0008: Publish-ready

**Status:** Draft

`cargo package -p sc-bd-cli --list` succeeds and the crate carries license,
description, repository and readme. A full `cargo publish --dry-run` waits until
`sc-beads` is published.

**Refines:** ADR-BDC-0007
**Acceptance evidence:** `cargo package -p sc-bd-cli --list` (sprint document).
