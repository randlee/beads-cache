---
id: a-2
phase: a
status: in-review
closure_type: boundary
target_boundary: sc-bd-cli
track: backend
wave: 2
branch: sprint/a-2-sc-bd-cli
worktree: sprint/a-2-sc-bd-cli
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [a-1]
parallel_safe: [a-3, a-4, a-5, a-6]
requirements:
  owns: [REQ-BDC-0006, REQ-BDC-0106, REQ-BDC-0201, REQ-BDC-0303, REQ-BDC-0905, NFR-BDC-0004, NFR-BDC-0006, REQ-BDCLI-0001, REQ-BDCLI-0002, REQ-BDCLI-0003, REQ-BDCLI-0004, REQ-BDCLI-0005, REQ-BDCLI-0006, REQ-BDCLI-0007, REQ-BDCLI-0008]
  constrained_by: [REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0105, REQ-BDC-0301, REQ-BDC-0302, REQ-BDC-0304, REQ-BDC-0904, REQ-BDC-0906, NFR-BDC-0005]
adrs: [ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0007, ADR-BEADS-0001, ADR-BEADS-0002, ADR-BEADS-0003, ADR-BEADS-0004, ADR-BDCLI-0001, ADR-BDCLI-0002, ADR-BDCLI-0003]
owned_paths:
  - "crates/sc-bd-cli/**"
  - ".github/workflows/bd-compat.yml"
  - "scripts/bd-compat/**"
  - "docs/sc-bd-cli/**"
---

# Sprint a-2 — sc-bd-cli: bd CLI implementation

## Outcome and closure

Implement btit's traits and `CacheBackend` from a-1 over the `bd` binary, in its own
crate (a later `sc-bd-http` implements the same traits over `bd serve`), and
add the bd-release compatibility workflow. Closes when `BdCli` passes a-1's
backend contract suite through a recording invoker and the workflow passes a
manual run. The live probes are written and runnable here but do not gate
closure; their recorded run is a-7's evidence.

## must_follow rationale

Implements `CacheBackend`, strict `Bead` parsing and `BeadsError` defined in
a-1.

## Deliverables

1. Port btit-cli (invoker, runner, probe, locks, ops) and btit-bd (`BdCli`,
   dolt-mode detection) into `sc-bd-cli`, with btit's parity and API-freeze
   tests; logging through `tracing` (REQ-BDC-0906 constraint).
2. Cache extensions over `bd` (REQ-BDC-0006, 0201): `fingerprint` (`bd
   version` plus sha256 of `bd schema`; never `bd context`, REQ-BDC-0106);
   `list_by_ids` (one `bd list --id a,b,... --status <all 7> -n 0 --json`,
   chunked); `list_scope` (`-l <label>`, `--updated-after` when a cursor is
   given); `list_blocked` (`bd blocked -l <label> --json`, strict `BlockedRow`);
   `list_ready` (`bd ready -l <label> -n 0 --json`); `close_guarded`
   (`--reason`), `update_guarded` (`--if-status`, `--if-assignee`),
   `dep_add_typed` (`--type`), all with `--actor`.
3. Error mapping and deadlines (NFR-BDC-0004): exit 13 to
   `Refusal::Precondition`; timeout, cannot-start and connection errors to
   `BeadsError::Unavailable` (`timed_out` set for a timeout); every call
   stops at the `Deadline` it is handed. The 30 s cap and the cut/outage
   classification belong to the Cache (a-1's `call` helper).
4. Patch to argv (REQ-BDC-0303): labels, `--set-metadata` with bools as
   `true`/`false`, `--unset-metadata`, `--append-notes`.
5. Live probes (NFR-BDC-0006): `#[ignore]` unless `SC_BD_LIVE=1`, run by the
   team-lead on the `m5-atmbench` testbed account in a scratch `bd init`
   workspace, never a shared database. They check that `bd close` and `bd reopen` bump the bead's own
   `updated_at`, and that `bd list --id` returns a closed bead only with the
   status list. They repeat the P-6 manual probe (`docs/research/bd-blocked-state-probe.md`)
   as code: no `is_blocked` in list rows, `bd blocked` shape and inheritance,
   `--id` needing the status list for closed beads, and that the cache's
   `ready` predicate (open, no future `defer_until`, not in `bd blocked`)
   equals `bd ready -l <label> --json` for a probe scope. a-7 records the
   full run.
6. bd compatibility workflow `.github/workflows/bd-compat.yml`
   (REQ-BDC-0905), triggered by `repository_dispatch` (type `beads-release`,
   payload `{"version": "<bd version>"}`, see P-5) and `workflow_dispatch`.
   For the given bd version it installs bd on a clean runner, compares
   `bd version` and the `bd schema` sha with the pinned fingerprint
   (attaching the schema diff on a mismatch), runs the `sc-beads` contract
   suite and the `sc-bd-cli` tests, then the live probes in a scratch
   workspace. On failure it opens or updates one issue per bd version.
   Regenerating the strict rows and cutting a semver-major release stay
   maintainer steps. Process-artifact justification: consumers are the
   beads-cache maintainers and, through the shared crates, btit; it gates
   adopting a bd release; it prevents silent row misreads and
   `BD_CONTRACT` failures on running caches after a bd upgrade; it is
   permanent while bd is upstream.

7. `BdCli` is constructed with the shared `Arc<dyn Clock>`; `list_by_ids` always passes all seven statuses (closed beads are omitted
   otherwise) and is chunked.
8. Per-crate docs `docs/sc-bd-cli/requirements.md` and `architecture.md`
   (ADR-BDCLI-0001: argv mapping and error classification).

## Acceptance criteria

| Criterion | Root |
|---|---|
| a-1's backend contract suite passes against `BdCli` with a recording invoker (REQ-BDC-0006) | ADR-BDC-0004 |
| `test_backend_check_does_not_need_bd_context` (REQ-BDC-0106) | boundary:sc-bd-cli |
| `test_metadata_values_round_trip_through_bd_coercion`: values accepted by a-1's validator reach `bd` unchanged; bools are `true`/`false` (REQ-BDC-0303) | boundary:sc-bd-cli |
| The recorded argv for every op and read matches the baseline's command forms (REQ-BDC-0201) | boundary:sc-bd-cli |
| Exit 13 → `Refusal::Precondition`; timeout or refused connection → `Unavailable`; every call stops at the `Deadline` it is handed with `timed_out: true` (NFR-BDC-0004) | ADR-BDC-0005 |
| btit's parity and API-freeze tests pass in `sc-bd-cli` | ADR-BDC-0007 |
| `cargo package -p sc-bd-cli --list` succeeds and the crates.io metadata (license, description, repository, readme) is complete; a full `cargo publish --dry-run` waits until `sc-beads` is published | ADR-BDC-0007 |
| The live probes compile and run under `SC_BD_LIVE=1` and are skipped otherwise (NFR-BDC-0006) | ADR-BDC-0002 |
| A `workflow_dispatch` run at the pinned bd version passes; a run whose schema sha differs fails and opens an issue with the diff (REQ-BDC-0905) | ADR-BDC-0007 |
| Planted bug: bools passed as `True` fails a test | boundary:sc-bd-cli |

## This sprint does not close

Use of `BdCli` by the cache or a scheduled drain (a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`; one manual workflow run.

## Public contract

Normative. The signatures below are those of the code at the evidence SHA; the
`tests/cache_api_freeze.rs` compile-time pin fails if one changes. Everything else
public in the crate is btit's, unchanged (REQ-BDCLI-0001), and pinned by the ported
`tests/cli_api_freeze.rs` and `tests/bd_api_freeze.rs`.

```rust
// sc_bd_cli::bounded (re-exported: sc_bd_cli::BoundedError)
#[non_exhaustive]
pub enum BoundedError {
    TimedOut,
    Failed(BeadsError),
}
pub fn run_bounded(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &str,
    argv: &[String],
    timeout: Duration,
) -> Result<CliOutput, BoundedError>;

// sc_bd_cli::CliInvoker: btit's trait, five required methods unchanged, one provided method added
fn run_bounded(
    &self,
    _project: &ProjectRef,
    _argv: &[String],
    _timeout: Duration,
) -> Result<CliOutput, BoundedError>;

// sc_bd_cli::BdCli: btit's constructors unchanged, two added
pub fn new_with_clock(
    binary: impl Into<String>,
    locks: Arc<ProjectLocks>,
    clock: Arc<dyn Clock>,
) -> Self;
pub fn with_clock(self, clock: Arc<dyn Clock>) -> Self;
// impl CacheBackend for BdCli: a-1's contract, unchanged

// sc_bd_cli::argv: the argv of every cache call
pub const ALL_STATUSES: &str = "open,in_progress,blocked,deferred,closed,pinned,hooked";
pub const IDS_PER_CALL: usize = 100;
pub fn version() -> Vec<String>;
pub fn schema() -> Vec<String>;
pub fn list_by_ids(ids: &[&BeadId]) -> Vec<String>;
pub fn list_scope(filter: &ScopeFilter, since: Option<&SyncCursor>) -> Vec<String>;
pub fn blocked(filter: &ScopeFilter) -> Vec<String>;
pub fn ready(filter: &ScopeFilter) -> Vec<String>;
pub fn close(id: &BeadId, reason: Option<&str>, actor: &Actor) -> Vec<String>;
pub fn update(id: &BeadId, patch: &Patch, guards: &sc_beads::Guards, actor: &Actor) -> Vec<String>;
pub fn dep_add(id: &BeadId, on: &BeadId, kind: DepType, actor: &Actor) -> Vec<String>;

// sc_bd_cli::cache
pub const UNREACHABLE_MARKERS: &[&str];

// sc_bd_cli::testing (feature `test-support`)
impl RecordingInvoker {
    pub fn reply_bounded(self, reply: Result<CliOutput, BoundedError>) -> Self;
    pub fn timeouts(&self) -> Vec<Duration>;
}
impl FakeBd {
    pub fn new(clock: Arc<ManualClock>) -> Self;
    pub fn over(backend: FakeBackend) -> Self;
    pub fn backend(&self) -> &FakeBackend;
    pub fn set_schema(&self, schema: impl Into<String>);
    pub fn set_bd_version(&self, version: impl Into<String>);
    pub fn calls(&self) -> Vec<Vec<String>>;
}
```

## Requirement scope notes

- REQ-BDC-0007 (strict rows), REQ-BDC-0008 (a bad row stores nothing), REQ-BDC-0105
  (backend check), REQ-BDC-0301 (patch order), REQ-BDC-0302 (guards), REQ-BDC-0304
  (dependency types), REQ-BDC-0904 (blocked state) and NFR-BDC-0005 (unit tests
  without bd) are listed in `constrained_by` because this sprint's code serves them
  while another sprint owns the behaviour: a-1 the strict rows and their validation,
  the rebuild rule's inputs (`Fingerprint`) and `DepType`; a-5 the write-time
  precondition and the drain; a-6 the blocked-state use. `sc-bd-cli` returns whole
  strict rows or a `Contract` error (a bad row fails the whole call, so nothing is
  stored from it), applies the patch fields in a-1's declared order, and passes
  `DepType` by its wire name.
- ADR-BDC-0006 is listed only because this crate adds dependencies; they are
  `serde_json` and `tracing`, both already workspace dependencies of `sc-beads`. No
  workspace dependency was added, and `Cargo.toml` `[workspace.dependencies]` is untouched.
- ADR-BEADS-0001..0004 are binding contract decisions this crate implements against:
  the verbatim port and `tracing` logging, `BeadsCacheError`, the strict rows and the
  recorded P-6 probe.
- Crate ids `REQ-BDCLI-0001..0008` and `ADR-BDCLI-0001..0003` are Approved (lead ruling, bc-lead, 2026-10-24; `docs/sc-bd-cli/`).

## Deviations (all 7 accepted, lead ruling bc-lead 2026-10-24)

Item 3: `run_bounded` is a provided method; note it for btit adoption. Item 4: not atomic; the drain's precondition re-check on the fresh pre-read row is the safety net. Item 7 (btit paths unbounded, cache calls bounded) is accepted as ADR-BDCLI-0002 states.

1. **Cannot-start is `Btit(Spawn)`, not `Unavailable`.** Deliverable 3 says
   cannot-start maps to `Unavailable`. `BeadsCacheError::Unavailable` carries only a
   `reason` string, so the `io::Error` cause would be lost; a-1's ADR-BEADS-0002
   already defines `is_unavailable()` to cover btit's `Spawn` ("bd cannot start"), and
   `bd-cache` classifies on `is_unavailable()`. `BdCli` therefore returns
   `Btit(BeadsError::Spawn { .. })` for a bd that cannot start, with the source chain.
   No change to a-1's contract. Ruling requested; the alternative is a text-only
   `Unavailable`.
2. **"Connection errors" are recognised from stderr.** bd exits 1 for an unreachable
   database exactly as for any failure; there is no typed signal (ADR-BDCLI-0001).
   The recognised fragments are one named constant, `UNREACHABLE_MARKERS`, with a
   live probe that re-checks them on each bd release.
3. **`CliInvoker` gains one provided method (`run_bounded`).** btit's five required
   methods and all signatures are unchanged; the ported API-freeze tests pass
   unmodified. Needs the btit side's agreement when btit adopts the crate
   (ADR-BDC-0007 consequence).
4. **`bd close` has no `--if-*` flags (bd 1.3.0/1.3.1)**, so a guarded close is a
   read-then-close (ADR-BDCLI-0001). The cache's drain checks the guards on the row
   it reads first, so the window is the same one a-5 already accepts.
5. **The metadata round-trip test restates the validator's accepted set** instead of
   calling `bd_cache_model::op::meta_value`: a dev-dependency on `bd-cache-model`
   breaks the CI boundary check (`sc-bd-cli` may depend on `sc-beads` only).
6. **Sprint-doc frontmatter `status: in-review`**, per the lead's hand-off rule, not
   the `complete` of the assignment's gate (`complete` follows QA and merge).

## Evidence

Code evidence SHA: `bdda20714e0dbcca9f6138acb3ffe20b08fa3c4d`, the merge of
`integrate/phase-a` (`aabdbf8`, a-3 merged) into `74874d2`, the last commit that
changes `crates/sc-bd-cli`. Documentation commits after it do not change code.

### Local gates (macOS arm64, Rust 1.98.1)

```text
cargo fmt --all --check                                          ok
cargo clippy --workspace --all-targets --all-features -- -D warnings   ok
cargo clippy --workspace --lib -- -D warnings                    ok (default features)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps       ok
cargo build --workspace                                          ok
cargo test --workspace --all-features                            ok
cargo package -p sc-bd-cli --list                                ok (34 files; license, description, repository, readme set)
```

`sc-bd-cli` runs 148 tests and 7 ignored live probes: 47 unit tests, 9 backend
contract rules through `BdCli` over `FakeBd`, 21 cache-call tests, 11 real-process
tests, 5 API-pin tests, the ported btit tests (7 + 3 API-freeze, 20 argv table, 19
behaviour, 3 dolt mode, 1 parity, 1 working-dir), and 1 doctest. The CI boundary
check (`docs/architecture.md`, Boundaries) passes locally: `sc-bd-cli` depends on
`sc-beads` only.

### Planted bugs (NFR-BDC-0005)

Each mutation was applied to the working tree at the evidence SHA, the whole
`sc-bd-cli` suite was run, and the file was restored (`git checkout -- <file>`):

```text
cargo test -p sc-bd-cli --all-features --no-fail-fast
```

| # | Planted bug | File | Exit | Compile error | Failing tests |
|---|---|---|---|---|---|
| 1 | `metadata_pair` renders a bool as `True` / `False` | `crates/sc-bd-cli/src/argv.rs` | 101 | no | `argv::tests::update_lists_every_field_in_order_then_guards_then_actor`, `recorded_argv_of_every_write_matches_the_baseline_forms`, `test_metadata_values_round_trip_through_bd_coercion`, `writes_apply_their_effect` |
| 2 | `check_exit` compares the exit status with `-13`, so exit 13 is `Failed` | `crates/sc-bd-cli/src/cache.rs` | 101 | no | `cache::tests::exit_13_is_a_precondition_refusal_whatever_stderr_says`, `exit_13_is_a_precondition_refusal`, `fake_bd_failures_reach_the_caller_as_the_errors_they_stand_for`, `unmet_guards_are_refused` |
| 3 | `ALL_STATUSES` drops `closed` | `crates/sc-bd-cli/src/argv.rs` | 101 | no | `argv::tests::reads_are_the_baseline_forms`, `argv_signatures_and_constants`, `close_and_reopen_bump_only_the_beads_own_updated_at`, `list_by_ids_is_chunked_deduplicated_and_names_all_seven_statuses`, `list_by_ids_returns_every_status`, `list_scope_filters_by_label_and_cursor`, `recorded_argv_of_every_read_matches_the_baseline_forms` |
| 4 | `time_left` never reports an expired deadline | `crates/sc-bd-cli/src/cache.rs` | 101 | no | `an_expired_deadline_starts_nothing`, `every_call_stops_at_its_deadline`, `every_call_stops_at_the_deadline_it_is_handed` |
| 5 | `timed_out()` builds `Unavailable { timed_out: false }` | `crates/sc-bd-cli/src/cache.rs` | 101 | no | `a_timeout_is_unavailable_with_timed_out_set`, `an_expired_deadline_starts_nothing`, `every_call_stops_at_its_deadline`, `every_call_stops_at_the_deadline_it_is_handed`, `fake_bd_failures_reach_the_caller_as_the_errors_they_stand_for`, `timeout_kills_a_running_child` |
| 6 | `check_exit` recognises no unreachable-server text | `crates/sc-bd-cli/src/cache.rs` | 101 | no | `a_refused_connection_is_unavailable_and_not_a_timeout`, `an_unreachable_server_is_unavailable`, `cache::tests::unreachable_server_is_unavailable_not_timed_out`, `fake_bd_failures_reach_the_caller_as_the_errors_they_stand_for` |
| 7 | `run_bounded` does not kill the child at the deadline | `crates/sc-bd-cli/src/bounded.rs` | 101 | no | `timeout_kills_a_running_child` |
| 8 | `list_scope` drops `--updated-after` | `crates/sc-bd-cli/src/argv.rs` | 101 | no | `argv::tests::reads_are_the_baseline_forms`, `list_scope_filters_by_label_and_cursor`, `recorded_argv_of_every_read_matches_the_baseline_forms` |
| 9 | `push_actor` adds no `--actor` | `crates/sc-bd-cli/src/argv.rs` | 101 | no | `a_guarded_close_reads_the_bead_first_and_closes_only_when_the_guards_hold`, `argv::tests::unassign_is_an_empty_assignee_word`, `argv::tests::update_lists_every_field_in_order_then_guards_then_actor`, `argv::tests::writes_are_the_baseline_forms`, `blocked_and_ready_match_the_p6_probe`, `close_and_reopen_bump_only_the_beads_own_updated_at`, `fake_bd_failures_reach_the_caller_as_the_errors_they_stand_for`, `recorded_argv_of_every_write_matches_the_baseline_forms`, `test_metadata_values_round_trip_through_bd_coercion`, `unmet_guards_are_refused`, `writes_apply_their_effect` |
| 10 | `IDS_PER_CALL` is 1 000 000 (one huge chunk) | `crates/sc-bd-cli/src/argv.rs` | 101 | no | `argv_signatures_and_constants`, `list_by_ids_is_chunked_deduplicated_and_names_all_seven_statuses` |
| 11 | `list_by_ids` keeps repeated ids | `crates/sc-bd-cli/src/cache.rs` | 101 | no | `list_by_ids_is_chunked_deduplicated_and_names_all_seven_statuses` |
| 12 | `close_guarded` skips the guard pre-read | `crates/sc-bd-cli/src/cache.rs` | 101 | no | `a_guarded_close_of_an_unknown_bead_fails_and_is_not_a_refusal`, `a_guarded_close_reads_the_bead_first_and_closes_only_when_the_guards_hold`, `unmet_guards_are_refused` |
| 13 | `fingerprint` also runs `bd context` | `crates/sc-bd-cli/src/cache.rs` | 101 | no | `a_failing_check_step_is_named`, `fingerprint_reports_version_and_schema`, `test_backend_check_does_not_need_bd_context` |

Every bug fails at least one test, none by a compile error (exit 101 is a test
failure; the output has `test ... FAILED` lines and no `could not compile`), and the
tree was clean after the run. The bool bug (1) is the assignment's planted bug
"bools passed as `True`".

### Live probes (NFR-BDC-0006)

`scripts/bd-compat/run-live.sh` (scratch Dolt server 2.3.5 on a free loopback port, own
`DOLT_ROOT_PATH`, `bd init --server --external` in a temp directory; both removed on
exit) at the evidence SHA, macOS arm64:

- bd 1.3.1 (the pinned release, installed by `scripts/bd-compat/install-bd.sh 1.3.1`,
  checksum-verified): 7 passed. The P-6 probe holds as code: no `is_blocked` in list
  rows, `bd blocked` shape with outside blockers and parent-child inheritance,
  `related` and a closed blocker do not block, and the cache's `ready` predicate
  equals `bd ready -l`.
- bd 1.3.0 (the machine's own): 6 passed, 1 failed, as intended:
  `fingerprint_reports_what_bd_prints` insists on the pinned bd unless
  `SC_BD_PINNED_ANY=1` (the workflow sets it and compares the pin itself).

Without `SC_BD_LIVE=1` the seven tests are ignored; with `--ignored` but no
`SC_BD_LIVE` they return at once. The a-7 run on the `m5-atmbench` testbed account is
a-7's evidence.

### bd-compat workflow (REQ-BDC-0905)

`workflow_dispatch` runs only for a workflow file on the repository's default branch
(`gh workflow run` answers `HTTP 404: workflow bd-compat.yml not found on the default
branch`), and the file is not on `main` until this work is released. The job steps were
therefore proved on a throwaway branch, `proof/a-2-bd-compat` (deleted afterwards),
whose only difference from `.github/workflows/bd-compat.yml` was a `push` trigger,
plus, for the second run, a forced expected schema sha and one line deleted from the
`bd-1.3.1/bd-schema.json` fixture so that the diff is not empty:

| Run | Scenario | Result |
|---|---|---|
| [40073075092](https://github.com/randlee/beads-cache/actions/runs/40073075092) | pinned bd 1.3.1 (no version given) on ubuntu-latest | passed: fingerprint, `cargo test -p sc-beads -p sc-bd-cli`, 7 live probes (58 s) |
| [40073178515](https://github.com/randlee/beads-cache/actions/runs/40073178515) | expected schema sha differs, fixture differs | failed as intended; opened issue #8 with the report and the `bd schema` diff |
| same run, re-run | same failure | commented on issue #8 (one issue per bd version) |

Issue #8 was closed with a note that it came from the proof. A first real
`workflow_dispatch` run and a `repository_dispatch` run (P-5) are possible once the
workflow is on `main`; the lead decides whether that gates closure.

### CI

PR [#9](https://github.com/randlee/beads-cache/pull/9). CI run
[40075405492](https://github.com/randlee/beads-cache/actions/runs/40075405492) at
`04453d8` is green on ubuntu, macos and windows. The first run (`b9eb355`) failed
on Windows only: the schema copy used by `FakeBd` was checked out with CRLF and its
sha differed. The copy moved to `crates/sc-bd-cli/tests/fixtures/`, which
`.gitattributes` pins to LF. Code evidence above is unchanged by that move apart from
this path (re-run: all sc-bd-cli tests pass).

### Deferred

The real `workflow_dispatch` (and `repository_dispatch`) run happens after develop to main at phase close-out; a-7 / close-out owns it. The proof-branch runs above are accepted as evidence.
