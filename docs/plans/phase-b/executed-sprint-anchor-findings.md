# Planning-gate findings on the executed phase-B sprints

The planning gate (plan-anchoring, 2026-10-26, Jev `jev-1.14.0`, threshold 0.6) was run on
every phase-B sprint doc on this branch, against the 23 anchors in
`docs/plans/phase-b/anchors/anchors.json`. b-1..b-6 are executed and merged, so their docs are not
changed; each failing cell is recorded here with where its fix lands. Run files are in
`docs/plans/phase-b/anchors/runs/` (run files: refreshed after the anchor rewording).

| Sprint | Result | Failing cells |
|---|---|---|
| b-1 | fail | ADR-BDC-0009, ANC-003, ANC-009, NFR-BDC-0004, NFR-BDC-0005, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502, REQ-BDC-0703, REQ-BDC-0812 |
| b-2 | fail | ADR-BDC-0009, ANC-009, NFR-BDC-0005, REQ-BDC-0001, REQ-BDC-0007, REQ-BDC-0502, REQ-BDC-0812 |
| b-3a | fail | ADR-BDC-0005, ADR-BDC-0009, NFR-BDC-0004, NFR-BDC-0005, REQ-BDC-0006 |
| b-3 | fail | ADR-BDC-0005, ANC-002, ANC-009, NFR-BDC-0004, NFR-BDC-0005, REQ-BDC-0006, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502, REQ-BDC-0703 |
| b-4 | fail | ADR-BDC-0005, ADR-BDC-0009, ANC-009, NFR-BDC-0004, NFR-BDC-0005, REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502, REQ-BDC-0703 |
| b-5 | pass | — |
| b-6 | pass | — (the sc-cron generality review found b-6 findings; see below) |

## Findings and where they are fixed

| Anchor | Sprints | Finding | Fix |
|---|---|---|---|
| ANC-009 scopes are bd filters | b-1, b-2, b-3, b-4 | The delivered scope ids treat a `phase-<x>` label as the scope form; the docs never say a scope is a bd filter. | b-7 deliverable 2 (the hard-coded scope prefix in `ScopeId` removed; a scope is a name plus a filter); b-8 deliverable 9 (PyO3 scope arguments take any scope name); b-9 deliverable 4 (contract and api text). |
| ANC-002 sync on the cron table | b-3 | `cache tick` runs the old tick with `refused` / `pass-through` replies. | b-8 deliverables 1, 2 and 11. |
| ANC-003 Python sync replaces the built-in one | b-1 | Not addressed by `compose_with_config`. | b-7 deliverable 2, b-8 deliverable 4. |
| ADR-BDC-0005 reads during an outage | b-3, b-3a, b-4 | The CLI and Python docs never show a read answered from the cache while bd is unreachable. | b-7 deliverable 7 and acceptance 4 (core); b-9 deliverable 3 (a CLI scenario: bd unavailable, `issues get` answers from the cache and the outage is recorded). b-3a only supplies the fault, so it needs no fix of its own. |
| ADR-BDC-0009 two command surfaces | b-1, b-2, b-3a, b-4 | The docs do not name the beads and cache-management surfaces. The delivered contract and CLI do have them. | b-9 deliverable 4 states it in the contract and maps every verb, including the Python namespaces (`cache.beads`, `cache.manage`), to a surface. |
| NFR-BDC-0004 kill at the deadline | b-1, b-3, b-3a, b-4 | The docs do not state the timeout kill for bd calls. | b-8 (every tick, method and hook process; acceptance 4); b-9 deliverable 3 (`refresh scopes --timeout-ms`). b-1 and b-4 start no process of their own. |
| NFR-BDC-0005 tests without bd or Dolt | b-1, b-2, b-3a, b-3, b-4 | The docs do not say the tests run with no bd and no Dolt. | b-9 deliverable 5: CI asserts that `bd` and `dolt` are not on PATH in the unit-test jobs. |
| REQ-BDC-0006 backend only through bd | b-3a, b-3 | The docs do not say the CLI reaches the backend only through bd. | b-9 deliverable 5: CI checks that no crate depends on a Dolt or MySQL client. |
| REQ-BDC-0007, 0008, 0502 strict, all-or-nothing, newer-row guard | b-1, b-2, b-3, b-4 | Not stated for the paths these sprints touch; they add no store path of their own. | b-7 deliverable 3 and b-8 deliverable 4 state and test them for every store path (sync and ingest). |
| REQ-BDC-0703 tick budget | b-1, b-3, b-4 | Not stated. None of these sprints builds the tick. | b-8 deliverable 3. |
| REQ-BDC-0812 Python 0.2 is additive | b-1, b-2 | Not stated. | b-8 deliverable 9 and b-9 deliverable 1 (the 0.1 call corpus). |
| REQ-BDC-0001 reads start no bd | b-2 | b-2 changes no read path. | No fix needed. |

b-6's doc passes the gate. The separate sc-cron generality review found that its hook runner and docs carry
caller-specific shape (one target per worker, the `Stop` alias, caller requirement ids in rustdoc and docs, and an
overstated isolation claim).
Fixed by REQ-CRON-0016 and ADR-CRON-0009 (this PR: a per-row Rust-or-Python target, no
caller-shaped API) and b-8 deliverable 6; Amendment A7 in `phase-b-boundary.md` records Rand's
ruling.
