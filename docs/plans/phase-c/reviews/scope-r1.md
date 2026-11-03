```json
{
  "status": "FAIL",
  "mode": "plan-scope-review",
  "reviewer": "plan-scope-reviewer",
  "round_id": "SCOPE-R1",
  "round_index": 1,
  "reviewed_commit": "e3ef2d259e774c937962e8c7d7f87e84d96f3b24",
  "previous_reviewed_commit": "",
  "findings_hash": "3c178b5c4e5de8f2af585907853de0b48ca96c902b74258c860dd1a3a311c3fb",
  "scope": {
    "phase": "c",
    "sprint": null
  },
  "parallelism": {
    "waves": 3,
    "critical_path": 3,
    "width": 2,
    "sprint_count": 4,
    "tracks": 4,
    "must_follow_edges": 3,
    "parallel_safe_edges": 1
  },
  "sprint_scores": [
    {
      "sprint": "c-1",
      "closure_type": "integration",
      "target_boundary": "existing scheduler composition",
      "status": "FAIL",
      "blocking_count": 0,
      "important_count": 1,
      "minor_count": 0
    },
    {
      "sprint": "c-2",
      "closure_type": "integration",
      "target_boundary": "generated application assembly",
      "status": "FAIL",
      "blocking_count": 0,
      "important_count": 2,
      "minor_count": 0
    },
    {
      "sprint": "c-3",
      "closure_type": "boundary",
      "target_boundary": "framework template upkeep tooling",
      "status": "FAIL",
      "blocking_count": 0,
      "important_count": 1,
      "minor_count": 0
    },
    {
      "sprint": "c-4",
      "closure_type": "integration",
      "target_boundary": "artifact distribution",
      "status": "FAIL",
      "blocking_count": 0,
      "important_count": 1,
      "minor_count": 0
    }
  ],
  "docs_read": [
    ".claude/agents/plan-scope-reviewer.md",
    ".claude/skills/plan-hardening/sprint-planning-guidelines.md",
    ".claude/project/quality-policy.md",
    "docs/plans/phase-c/phase-c-plan.md",
    "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
    "docs/plans/phase-c/sprint-c-2-framework-application.md",
    "docs/plans/phase-c/sprint-c-3-template-upkeep.md",
    "docs/plans/phase-c/sprint-c-4-release-qualification.md",
    "docs/plans/phase-c/intent.md",
    "docs/requirements.md",
    "docs/architecture.md",
    "docs/bd-cache-py/architecture.md",
    "docs/bd-cache-api/requirements.md",
    "docs/bd-cache-cli/architecture.md",
    "docs/project-plan.md",
    "docs/team-protocol.md"
  ],
  "findings": [
    {
      "id": "PLAN-SCOPE-001",
      "severity": "Important",
      "category": "DROP-RISK",
      "classification": "structural",
      "affects_ac": true,
      "target_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:75",
        "docs/bd-cache-py/architecture.md:236"
      ],
      "issue": "c-1 must amend the Python transitive sc-command exclusion and records that governed docs change as a deliverable, but docs/bd-cache-py/architecture.md is outside every sprint owned_paths. That document still says Python takes no sc-command and that CI rejects it. The permitted contract/logging exception would therefore land with either an out-of-fence edit or a contradictory governed boundary statement.",
      "required_correction": "Assign docs/bd-cache-py/architecture.md to c-1 and explicitly include its narrow ADR-BDC-0016 amendment in the existing dependency-exception deliverable; preserve the no-clap/no-server restriction. Do not add a new sprint or report artifact."
    },
    {
      "id": "PLAN-SCOPE-002",
      "severity": "Important",
      "category": "MISSING-CODE-SAMPLE",
      "classification": "structural",
      "affects_ac": true,
      "target_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md:63",
        "docs/plans/phase-c/sprint-c-2-framework-application.md:134"
      ],
      "issue": "c-3 closes a helper that consumes an adopted-path mapping, while c-2 later creates runtime/adoption.toml, but neither plan defines that shared input shape or how generated paths map to application paths. The required negative test for a suppressed changed adopted file cannot be implemented from the sprint doc without choosing an undocumented cross-sprint interface.",
      "required_correction": "In c-3, fix a minimal TOML mapping example and its meaning (generated path to adopted path, and handling of added/deleted/unknown paths), with the helper acceptance rooted in that format. Have c-2 reference that one definition when writing runtime/adoption.toml. Keep the interface in the existing sprint doc rather than adding a manifest or new contract sprint."
    },
    {
      "id": "PLAN-SCOPE-003",
      "severity": "Important",
      "category": "SERIAL-RISK",
      "classification": "structural",
      "affects_ac": false,
      "target_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md:218",
        "docs/plans/phase-c/sprint-c-4-release-qualification.md:108"
      ],
      "issue": "The must_follow edges name real consumed artifacts, but their execution relation is unspecified: the sprint docs do not state that parent development push triggers merge-forward before every child dev/fix round, parent QA must not be awaited, and parent PR merge gates child PR completion. A dispatcher consuming these docs can incorrectly serialize all development behind parent QA.",
      "required_correction": "Add the guideline-prescribed push/merge-forward and PR-completion triggers to the existing c-2 and c-4 dependency sections (or one phase dependency rule explicitly referenced by both). Preserve the computed CP3/width2; do not add a new process artifact."
    }
  ],
  "minor_wording": [
    {
      "id": "PLAN-SCOPE-M1",
      "category": "VAGUE",
      "affects_ac": false,
      "target_refs": [
        "docs/plans/phase-c/phase-c-plan.md:249"
      ],
      "issue": "The phase plan says the planning branch does not change CI and the Python runner repair is still required, but the reviewed commit already pins the three Python Linux matrices to Ubuntu 24.04.",
      "suggested_cleanup": "Update the CI-status note to distinguish the landed planning-branch repair from the still-running validation; retain the observed defect and avoid claiming green checks prematurely."
    },
    {
      "id": "PLAN-SCOPE-M2",
      "category": "VAGUE",
      "affects_ac": false,
      "target_refs": [
        "docs/plans/phase-c/phase-c-plan.md:164"
      ],
      "issue": "The feature-closure table column is labelled sole acceptance owner but the preservation-validation row names c-1/c-2. Their actual acceptance scenarios are distinguishable (migration compatibility versus daemon-present/absent qualification), so this is table wording rather than duplicated feature implementation.",
      "suggested_cleanup": "Identify c-1 as the preservation owner and describe c-2 as application qualification of that preserved surface, or move the shared validation row out of the feature-ownership table."
    }
  ],
  "ready_for_next_step": false,
  "errors": [],
  "review_notes": [
    "Genuine developer STEP1 handoff is valid and matches the reviewed planning text by recorded hashes.",
    "User-approved ANC-001 deterministic lint and ADR-BDC-0009 non-applicability exceptions retained; no anchor verdict changed.",
    "Mechanically checked c-1/c-3 owned globs: no literal/prefix intersection and no tracked-file overlap.",
    "No re-cut is requested: the atomic scheduler migration and initial service/daemon boundary rationale are acceptable, with no thin layer sprint.",
    "Executed as the Codex inherited-model role adaptation; no Sonnet execution claim."
  ]
}
```
