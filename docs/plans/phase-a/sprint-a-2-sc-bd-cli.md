---
id: a-2
phase: a
status: planned
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
  owns: [REQ-BDC-0006, REQ-BDC-0106, REQ-BDC-0201, REQ-BDC-0303, REQ-BDC-0905, NFR-BDC-0004, NFR-BDC-0006]
  constrained_by: [REQ-BDC-0007, REQ-BDC-0301, REQ-BDC-0904, REQ-BDC-0906]
adrs: [ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007]
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
   given); `list_changed` (`--id ... --updated-after`); `close_guarded`
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
   workspace, never a shared database. They check that `--id` and
   `--updated-after` combine as an intersection, and that `bd close` and
   `bd reopen` bump the bead's own `updated_at`. P-6 is a manual run of the intersection probe before a-1 freezes; this
   coded probe repeats it. a-7 records the full run.
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

7. `BdCli` is constructed with the shared `Arc<dyn Clock>`; `list_changed`
   is chunked like `list_by_ids`.
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
