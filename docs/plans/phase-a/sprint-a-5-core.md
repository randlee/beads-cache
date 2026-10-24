---
id: a-5
phase: a
status: planned
closure_type: boundary
target_boundary: bd-cache
track: cache
wave: 3
branch: sprint/a-5-core
worktree: sprint/a-5-core
recommended_agent: rust-developer
recommended_model: opus
must_follow: [a-4]
parallel_safe: [a-2, a-3]
requirements:
  - REQ-BDC-0001
  - REQ-BDC-0002
  - REQ-BDC-0003
  - REQ-BDC-0004
  - REQ-BDC-0005
  - REQ-BDC-0008
  - REQ-BDC-0105
  - REQ-BDC-0202
  - REQ-BDC-0203
  - REQ-BDC-0302
  - REQ-BDC-0401
  - REQ-BDC-0402
  - REQ-BDC-0403
  - REQ-BDC-0404
  - REQ-BDC-0405
  - REQ-BDC-0406
  - REQ-BDC-0501
  - REQ-BDC-0502
  - REQ-BDC-0503
  - REQ-BDC-0504
  - REQ-BDC-0601
  - REQ-BDC-0602
  - REQ-BDC-0603
  - REQ-BDC-0802
  - REQ-BDC-0903
  - REQ-BDC-0904
  - NFR-BDC-0001
  - NFR-BDC-0005
adrs: [ADR-BDC-0001, ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0004, ADR-BDC-0005]
owned_paths:
  - "crates/bd-cache/**"
---

# Sprint a-5 — core library

## Outcome and closure

The `Cache` facade over the store and the `BeadsBackend` trait, tested only
against `FakeBackend`. Closes when every assigned ported case passes and the
planted-bug checks from BD-CACHE.md fail at least one test each.

## must_follow rationale

Calls the store's transactional API from a-4 (journal insert with
precondition check; read-back store and applied mark in one transaction).

## Deliverables

1. `get` (overlay of pending entries, miss-once fetch, out-of-scope rules
   from the target design), `list` (refused outside every scope), `write`,
   `resolve`, `prune`, `status` (with `last_ok_at`, stale rows, outage).
2. `drain`: single writer (`flock` on the drain lock), deadline, one batched
   pre-read, achieved / precondition / write / read-back, per-bead hold,
   `MAX_ATTEMPTS` for failures from a reachable backend only, stop at the
   first `Unavailable` without counting attempts (ADR-BDC-0005).
3. `sync` per scope: incremental cursor, newer-row guard, stale re-read, then
   the targeted depender refresh (ADR-BDC-0002): cached blockers whose status
   changed, one small query for outside blockers, fixpoint over parent-child.
4. `ready` from cached `is_blocked`, with the refresh above keeping it right.

## Acceptance criteria

| Criterion | Root |
|---|---|
| The 27 remaining `test_bd_cache.py` cases pass (a-1's ownership table) | BD-CACHE.md Tests |
| Each of the 32 planted bugs listed in BD-CACHE.md fails at least one test | BD-CACHE.md Tests |
| Closing or reopening a blocker (cached or outside) flips its cached dependers' `is_blocked` after one sync, re-reading only those dependers | ADR-BDC-0002 |
| An outage of N ticks leaves entries pending with unchanged `attempts`; a reachable-backend failure still escalates at 5 | ADR-BDC-0005 |
| Error replies carry the `BD_CACHE.*` code, fix text and `/v0` `reason` | ADR-BDC-0003 |

## This sprint does not close

- Scheduled drain and sync, escalation delivery, the real bd adapter in the
  loop (a-6).

## Required validation

Crate tests and lints; `cargo build --workspace`.
