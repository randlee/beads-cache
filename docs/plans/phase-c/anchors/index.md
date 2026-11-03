# Phase C inherited work bounds

Source of authority: [phase-B anchor record](../../phase-b/anchors/anchors.json).
The user authorized reuse of relevant prior-sprint anchors on 2026-11-02. This
phase reuses the records listed below without changing their statements, quotes,
scopes, fixtures, status, question overrides or tracker identities. Phase-B item
exclusions are not inherited: they refer to different sprint IDs. This membership
selection does not widen an old anchor's scope or manufacture new approval.

The rendered statement record is [anchors.md](anchors.md); anchors.json is the
machine-readable phase selection. Existing approved REQs and ADRs remain binding
in addition to this selected set. In particular, technical compatibility choices
and planning instructions are not substituted for product bounds.

| Work boundary | Inherited constraints | Most directly constrains |
|---|---|---|
| Cache and backend boundary | REQ-BDC-0001, REQ-BDC-0006, ADR-BDC-0005 | c-1/c-2 must retain local reads and backend access only through bd, with outages not fabricated failures |
| Persistence boundary | ADR-STORE-0001 | c-1's scheduler/store migration cannot rewrite the v3 file contract |
| Ingest correctness | REQ-BDC-0007, REQ-BDC-0008, REQ-BDC-0502 | c-1/c-2 must preserve strict validation, all-or-nothing ingestion and the newer-row guard |
| Scheduling responsibility | ANC-002, ANC-003, ANC-004, ANC-008, ANC-009 | c-1/c-2 must preserve table-driven sync, Python replacement, hook results/errors, one built-in row and filter scopes |
| Front-end contract | ADR-BDC-0009, REQ-BDC-0812 | c-1/c-2 cannot use framework migration to redesign bdc surfaces or the Python 0.1 contract |
| Runtime and test limits | NFR-BDC-0004, NFR-BDC-0005 | work that changes bd/hooks must retain hard deadlines; unit tests remain independent of live bd/Dolt |
| Product independence | ANC-001 | every sprint's source, tests and docs remain free of prohibited consumer concerns |

No phase-wide requirement is inferred from a casual remark. ANC-005..007 are not
selected as new upstream-scheduler implementation obligations: this phase consumes
the published scheduler and retains the existing cache guarantees. ANC-010's lint
exemption is outside the current work fence. ANC-011 explicitly names phase-B
sprint plans, so its scope is not silently rewritten to phase C; the current sprint
planning guidelines already require deliverable-to-REQ/ADR traceability.

## Sprint fences

- c-1 may adapt scheduler dependencies, the cache-owned schedule store/worker and
  necessary compatibility/CI wiring. It must not add journal operations, redesign
  cache reads/writes or change the existing CLI/Python wire contracts.
- c-2 may install/adapt application assembly and host the existing sync/readiness
  behavior. It must not move domain logic into transport handlers, add arbitrary
  HTTP/MCP operations, introduce a second scheduler database or make local reads
  depend on a daemon. Its optional daemon surface remains proposed scope.
- c-3 may record template inputs and compare pristine generated baselines. It must
  not build a general installer/merge engine, change runtime domain behavior or
  begin the deferred sc-lint work.
- c-4 may qualify supported artifacts and release configuration. It must not claim
  publication from a dry run or bypass upstream release blockers. Artifact tests
  verify the preserved contracts; they do not redefine them.

Applicability follows each inherited record's existing scope. A future gate uses
complete sprint text and records exclusions only when an anchor does not govern
that sprint. Selecting prior records is not a passing gate; no new phase-C anchor
review is claimed by this correction.
