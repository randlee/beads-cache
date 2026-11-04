```json
{
  "success": true,
  "data": {
    "source": "d8242dbe6e246a7c69a2d9a892451c3080364050",
    "worktree_path": "/Users/randlee/github/beads-cache-worktrees/sprint/c-1-runtime-migration",
    "sprint_doc": "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
    "scope": "Proposals only; ceremony assessments below apply only to incremental process-artifact demands, never to the underlying defect, original assertions, existing QA/anchor/CI obligations or finding disposition; lead adjudication is required.",
    "screened": [
      {
        "id": "ARCH-001",
        "verdict": "not_applicable",
        "missing": [],
        "existing_coverage": "The existing isolated-consumer graph gate rejects the unauthorized edge.",
        "reason": "The remedy changes the public logging composition and removes a dependency edge, adding no process artifact; the actual open boundary violation is outside this ceremony screen."
      },
      {
        "id": "REQ-QA-004",
        "verdict": "concern_valid_remedy_ceremony",
        "missing": [
          "retirement_condition"
        ],
        "existing_coverage": "The sprint already requires existing process/shutdown suites, independent implementation QA, development anchors and three-OS CI (lines 220-221 and 378-391); its QA10 coverage table (lines 1109-1117), source history and existing reviewer/correction artifacts support bounded old/new review.",
        "reason": "The actual composition and native qualification failures remain valid, but requiring additional coverage ledgers duplicates existing source/test/QA evidence without a named retirement condition; this proposal concerns only that incremental artifact demand."
      },
      {
        "id": "RBQA-F001",
        "verdict": "not_applicable",
        "missing": [],
        "existing_coverage": "The existing graph gate and binding ADR-BDC-0016 govern the introducer; the historical carry remains independently open.",
        "reason": "Removing the direct dependency through an approved public port is a code/boundary correction; the contingent separately authorized ADR route adds no presently required process artifact and grants no policy change."
      },
      {
        "id": "QA2-001",
        "verdict": "not_applicable",
        "missing": [],
        "existing_coverage": "The unchanged exact-head isolated Python graph guard is already failing on the direct introducer.",
        "reason": "The remedy preserves an existing guard and requires correcting and qualifying the actual consumer, rather than creating a new artifact or gate."
      },
      {
        "id": "RQA9-001",
        "verdict": "concern_valid_remedy_ceremony",
        "missing": [
          "retirement_condition"
        ],
        "existing_coverage": "Existing store negative-qualification assertions and private phase protocol retain the five-second ownership, 30ms cancellation, 500ms observation and real return/stall/reap evidence; git history and the owning reviewer/correction artifact can document the bounded before/after assessment.",
        "reason": "The localized Windows setup-versus-lifetime failure still needs a supported fixture correction, but a separate mandatory coverage ledger has no named retirement condition and duplicates existing tests, phase evidence and source review."
      },
      {
        "id": "RQA11-001",
        "verdict": "concern_valid_remedy_ceremony",
        "missing": [
          "retirement_condition"
        ],
        "existing_coverage": "The retained delayed-inventory positive, timeout/ownership controls and sprint QA10 old/new coverage table already identify the STOP/80ms/CONT, complete census, original 250ms budget and actual wait/reap obligations; source history and existing owner/correction artifacts support review of any redesign.",
        "reason": "The separate actual macOS delayed-positive failure remains open, but an additional mandatory old/new coverage ledger has no named retirement condition and duplicates the existing coverage table, tests and bounded owner review."
      }
    ]
  },
  "error": null
}
```
