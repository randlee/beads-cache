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
parallel_safe: [a-3, a-4, a-5]
baseline_sections:
  - "BD-CACHE.md: Goals 4-5 (backend-neutral, strict contract)"
  - "BD-CACHE.md: Storage (backend check)"
  - "BD-CACHE.md: Journal operations (bd command column)"
  - "BD-CACHE.md: Performance (per-call timeouts)"
adrs: [ADR-BDC-0002, ADR-BDC-0004, ADR-BDC-0005, ADR-BDC-0007]
owned_paths:
  - "crates/sc-bd-cli/**"
---

# Sprint a-2 — sc-bd-cli: bd CLI implementation

## Outcome and closure

Implement the `sc-beads` traits over the `bd` binary in a separate crate,
`sc-bd-cli` (a later `sc-bd-http` implements the same traits over
`bd serve`). Port btit-cli (invoker, runner, probe, locks, ops) and btit-bd
(`BdCli`, dolt-mode detection), then add the cache extensions frozen in a-1. Closes when the adapter passes a-1's
contract tests through a recording invoker and the live probes pass in a
scratch workspace.

## must_follow rationale

Consumes the `BeadsBackend` extension signatures and strict row parsing from
a-1.

## Deliverables

1. Port of btit-cli and btit-bd into `sc-bd-cli`, keeping btit's parity
   and API-freeze tests (logging moved to `tracing`).
2. Extensions: `fingerprint` (`bd version` plus sha256 of `bd schema`, no
   `bd context`), `list_by_ids` (one `bd list --id a,b,... --status <all 7>
   -n 0 --json`, chunked), `list_since` (`-l <label> --updated-after`),
   guarded `close --reason` and `update` (`--if-status`, `--if-assignee`),
   `dep add --type`, all with `--actor`.
3. Exit 13 maps to `Refusal::Precondition`; timeout, cannot-start and
   connection errors map to `BeadsError::Unavailable` (ADR-BDC-0005); every
   call is capped at its `Deadline`.
4. Patch to argv: labels, `--set-metadata` with bools as `true`/`false`,
   `--unset-metadata`, `--append-notes`.
5. Live probes (testbed only, `#[ignore]` unless `SC_BEADS_LIVE=1`, scratch
   `bd init` workspace, never a shared database): that `--id` and
   `--updated-after` combine as an intersection, and that `bd close` and
   `bd reopen` bump the bead's own `updated_at` (ADR-BDC-0002).

## Acceptance criteria

| Criterion | Root |
|---|---|
| a-1's contract tests pass against `BdCli` with a recording invoker | ADR-BDC-0004 |
| `test_backend_check_does_not_need_bd_context` | BD-CACHE.md Storage |
| The recorded argv for every op matches the BD-CACHE.md command column | BD-CACHE.md Journal operations |
| Exit 13 → `Refusal::Precondition`; timeout → `Unavailable`; no call outlives its deadline | ADR-BDC-0005 |
| btit's parity and API-freeze tests pass in their new home | ADR-BDC-0007 |
| `cargo publish --dry-run -p sc-bd-cli` passes | ADR-BDC-0007 |
| Live probes pass and their results are recorded in ADR-BDC-0002 | ADR-BDC-0002 |

## This sprint does not close

- Use of the adapter by the cache or a scheduled drain (a-6).

## Required validation

Crate tests and lints; `cargo build --workspace`; live probes run once on
the testbed and recorded.
