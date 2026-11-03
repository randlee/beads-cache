```json
{
  "agent": "arch-qa",
  "branch": "plan/phase-c",
  "scope": {
    "phase": "c",
    "sprint": null
  },
  "commit": "0beef1a8e213b9b55af85d55ca014e85fde52782",
  "verdict": "FAIL",
  "blocking": 3,
  "important": 0,
  "findings": [
    {
      "id": "ARCH-001",
      "rule": "RULE-ADR-PLAN",
      "adr": null,
      "severity": "BLOCKING",
      "file": "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
      "line": 59,
      "description": "c-1 owns all local scheduler, store, sync, CLI and Python source/test paths but its authoritative adrs list omits binding decisions for those crates. Missing: ADR-CRON-0001, ADR-CRON-0002, ADR-CRON-0003, ADR-CRON-0004, ADR-CRON-0005, ADR-CRON-0006, ADR-CRON-0007, ADR-CRON-0008, ADR-CRON-0009, ADR-STORE-0002, ADR-STORE-0004, ADR-STORE-0005, ADR-SYNC-0001, ADR-SYNC-0002, ADR-SYNC-0006, ADR-CLI-0002, ADR-CLI-0003, ADR-CLI-0004, ADR-PY-0001, ADR-PY-0002, ADR-PY-0003, ADR-PY-0004, ADR-PY-0005, ADR-BDC-0009, ADR-BDC-0012, ADR-BDC-0013, ADR-CORE-0001. These govern retained clock/deadline/fencing and hook behavior, file-sharing/index rules, dependency/test features, composition/threading, command mapping/configuration and Python reply/packaging compatibility during the migration. The list also omits ADR-BDC-0009/0012/0013 although its bounds and validation explicitly preserve those subjects.",
      "remediation": "Complete c-1\u2019s existing adrs list with the listed binding IDs; do not copy their decision prose or change scope. Retain superseded historical ADRs as history rather than treating them as current constraints."
    },
    {
      "id": "ARCH-002",
      "rule": "RULE-ADR-PLAN",
      "adr": null,
      "severity": "BLOCKING",
      "file": "docs/plans/phase-c/sprint-c-2-framework-application.md",
      "line": 50,
      "description": "c-2\u2019s new application composition is explicitly constrained by cache outage behavior, named filter scopes, one cache connection and mutex-owned !Sync Cache, but its adrs list omits ADR-BDC-0005, ADR-BDC-0013, ADR-STORE-0003 and ADR-CORE-0001. c-4\u2019s new tracked scripts/docs must preserve the no-consumer boundary, but its adrs list omits ADR-BDC-0010 and ADR-BDC-0012.",
      "remediation": "Add those governing binding IDs to the existing c-2 and c-4 adrs lists. Keep the existing behavior bounds; no new artifact or validation gate is needed."
    },
    {
      "id": "ARCH-003",
      "rule": "RULE-ADR-PLAN",
      "adr": "ADR-BDC-0016",
      "severity": "BLOCKING",
      "file": "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
      "line": 66,
      "description": "All four sprint adrs lists contain \"ADR-BDC-0016 (proposed)\", but docs/architecture.md:1102 expressly says it is not an approved exception. The adrs contract is a list of governing binding ADR IDs; this entry neither names a binding decision nor separates the proposed amendment from current authority. The phase says old rules remain binding unless approved, but does not make an actual approval decision an explicit dispatch prerequisite for the migration/template/application work that relies on the amendment.",
      "remediation": "Keep ADR-BDC-0016 Proposed until an actual authorized approval occurs. Separate the proposed decision from the binding adrs lists (for example an existing-doc proposed_adrs field), and explicitly block dispatch of dependent implementation until that approval is recorded. On approval, use the exact ID as governing authority. Do not mark it Approved merely because this QA round or the plan PR passes."
    }
  ],
  "adr_checks": [
    {
      "adr": "ADR-BDC-0005",
      "source": "docs/architecture.md:371",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0006",
      "source": "docs/architecture.md:412",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0010",
      "source": "docs/architecture.md:622",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0011",
      "source": "docs/architecture.md:731",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0014",
      "source": "docs/architecture.md:974",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0015",
      "source": "docs/architecture.md:1017",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0016",
      "source": "docs/architecture.md:1100",
      "listed_in_sprint_doc": true,
      "result": "not-applicable",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Proposed amendment is not binding; ARCH-003 covers metadata and approval prerequisite."
    },
    {
      "adr": "ADR-STORE-0001",
      "source": "docs/bd-cache-store/architecture.md:27",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-STORE-0003",
      "source": "docs/bd-cache-store/architecture.md:210",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-STORE-0006",
      "source": "docs/bd-cache-store/architecture.md:589",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-SYNC-0008",
      "source": "docs/bd-cache-sync/architecture.md:479",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-CLI-0001",
      "source": "docs/bd-cache-cli/architecture.md:29",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-CLI-0005",
      "source": "docs/bd-cache-cli/architecture.md:166",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-PY-0006",
      "source": "docs/bd-cache-py/architecture.md:269",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0009",
      "source": "docs/architecture.md:588",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0010",
      "source": "docs/architecture.md:622",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0011",
      "source": "docs/architecture.md:731",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0012",
      "source": "docs/architecture.md:769",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0014",
      "source": "docs/architecture.md:974",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0015",
      "source": "docs/architecture.md:1017",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0016",
      "source": "docs/architecture.md:1100",
      "listed_in_sprint_doc": true,
      "result": "not-applicable",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Proposed amendment is not binding; ARCH-003 covers metadata and approval prerequisite."
    },
    {
      "adr": "ADR-SYNC-0002",
      "source": "docs/bd-cache-sync/architecture.md:57",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-SYNC-0008",
      "source": "docs/bd-cache-sync/architecture.md:479",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-CLI-0003",
      "source": "docs/bd-cache-cli/architecture.md:102",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-CLI-0004",
      "source": "docs/bd-cache-cli/architecture.md:130",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-CLI-0005",
      "source": "docs/bd-cache-cli/architecture.md:166",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-PY-0006",
      "source": "docs/bd-cache-py/architecture.md:269",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0010",
      "source": "docs/architecture.md:622",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0012",
      "source": "docs/architecture.md:769",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0016",
      "source": "docs/architecture.md:1100",
      "listed_in_sprint_doc": true,
      "result": "not-applicable",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-3-template-upkeep.md"
      ],
      "notes": "Proposed amendment is not binding; ARCH-003 covers metadata and approval prerequisite."
    },
    {
      "adr": "ADR-BDC-0006",
      "source": "docs/architecture.md:412",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-BDC-0016",
      "source": "docs/architecture.md:1100",
      "listed_in_sprint_doc": true,
      "result": "not-applicable",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ],
      "notes": "Proposed amendment is not binding; ARCH-003 covers metadata and approval prerequisite."
    },
    {
      "adr": "ADR-PY-0004",
      "source": "docs/bd-cache-py/architecture.md:161",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-CLI-0001",
      "source": "docs/bd-cache-cli/architecture.md:29",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-4-release-qualification.md"
      ],
      "notes": "Current planning commitments preserve the decision; this is plan review, not an implementation compliance claim."
    },
    {
      "adr": "ADR-CRON-0001",
      "source": "docs/sc-cron/architecture.md:256",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0002",
      "source": "docs/sc-cron/architecture.md:289",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0003",
      "source": "docs/sc-cron/architecture.md:309",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0004",
      "source": "docs/sc-cron/architecture.md:328",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0005",
      "source": "docs/sc-cron/architecture.md:347",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0006",
      "source": "docs/sc-cron/architecture.md:373",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0007",
      "source": "docs/sc-cron/architecture.md:424",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0008",
      "source": "docs/sc-cron/architecture.md:462",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CRON-0009",
      "source": "docs/sc-cron/architecture.md:525",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-STORE-0002",
      "source": "docs/bd-cache-store/architecture.md:122",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-STORE-0004",
      "source": "docs/bd-cache-store/architecture.md:360",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-STORE-0005",
      "source": "docs/bd-cache-store/architecture.md:547",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-SYNC-0001",
      "source": "docs/bd-cache-sync/architecture.md:35",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-SYNC-0002",
      "source": "docs/bd-cache-sync/architecture.md:57",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-SYNC-0006",
      "source": "docs/bd-cache-sync/architecture.md:330",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CLI-0002",
      "source": "docs/bd-cache-cli/architecture.md:68",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CLI-0003",
      "source": "docs/bd-cache-cli/architecture.md:102",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CLI-0004",
      "source": "docs/bd-cache-cli/architecture.md:130",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-PY-0001",
      "source": "docs/bd-cache-py/architecture.md:33",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-PY-0002",
      "source": "docs/bd-cache-py/architecture.md:77",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-PY-0003",
      "source": "docs/bd-cache-py/architecture.md:123",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-PY-0004",
      "source": "docs/bd-cache-py/architecture.md:161",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-PY-0005",
      "source": "docs/bd-cache-py/architecture.md:206",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-BDC-0009",
      "source": "docs/architecture.md:588",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-BDC-0012",
      "source": "docs/architecture.md:769",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-BDC-0013",
      "source": "docs/architecture.md:853",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-CORE-0001",
      "source": "docs/bd-cache/architecture.md:33",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
      ],
      "notes": "Behavior retained by plan; missing authoritative coverage is ARCH-001."
    },
    {
      "adr": "ADR-BDC-0005",
      "source": "docs/architecture.md:371",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Behavior retained; metadata coverage gap ARCH-002."
    },
    {
      "adr": "ADR-BDC-0013",
      "source": "docs/architecture.md:853",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Behavior retained; metadata coverage gap ARCH-002."
    },
    {
      "adr": "ADR-STORE-0003",
      "source": "docs/bd-cache-store/architecture.md:210",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Behavior retained; metadata coverage gap ARCH-002."
    },
    {
      "adr": "ADR-CORE-0001",
      "source": "docs/bd-cache/architecture.md:33",
      "listed_in_sprint_doc": false,
      "result": "upheld",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-2-framework-application.md"
      ],
      "notes": "Behavior retained; metadata coverage gap ARCH-002."
    }
  ],
  "gate_artifact_checks": [
    {
      "artifact": "docs/plans/phase-c/phase-c-plan.md",
      "status": "not-applicable",
      "evidence_refs": [
        "docs/plans/phase-c/phase-c-plan.md:16",
        "docs/plans/phase-c/phase-c-plan.md:182"
      ],
      "notes": "Future sprint and release closure commitments are not claimed complete; no future runtime implementation or published artifact demanded at the planning gate."
    },
    {
      "artifact": "docs/plans/phase-c/anchors/anchors.json",
      "status": "closed",
      "evidence_refs": [
        "docs/plans/phase-c/reviews/anchor-coverage-post-hardening.md",
        "docs/plans/phase-c/reviews/workflow-state.json"
      ],
      "notes": "Four latest per-item gates pass. User-approved ANC-001 deterministic-lint and ADR-BDC-0009 non-applicability exceptions retained without changed semantic verdicts."
    }
  ],
  "merge_ready": false,
  "notes": "Read the assigned phase/sprint docs, root architecture and quality policy, and relevant per-crate architecture records (including scheduler/store/sync/CLI/Python decisions and shared model/core/backend/API constraints). Verified exact branch and HEAD before review. Architecture shape is coherent: one cache connection, shared composition via blocking work, no direct backend SQL, existing CLI/Python contracts, no sc-lint expansion, immutable upstream source and candidate-vs-hosted-release separation. Findings concern authoritative ADR coverage and proposed-amendment dispatch authority. No tests/CI adjudication, no product edits and no subagents. Executed as the Codex inherited-model role adaptation; no Grok/Sonnet execution claim."
}
```
