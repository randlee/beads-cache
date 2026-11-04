```json
{
  "status": "FAIL",
  "reviewed_branch": "sprint/c-1-runtime-migration",
  "reviewed_commit": "a753d2799183dd4e341c854c4515bf046447eb6c",
  "errors": [],
  "scope": {
    "phase": "c",
    "sprint": "c-1"
  },
  "baselines_read": [
    ".claude/project/quality-policy.md",
    "docs/requirements.md",
    "docs/architecture.md",
    "docs/project-plan.md",
    "docs/bd-cache-store/requirements.md",
    "docs/bd-cache-sync/requirements.md",
    "docs/bd-cache-cli/requirements.md",
    "docs/bd-cache-py/requirements.md",
    "docs/bd-cache-api/requirements.md",
    "docs/sc-cron/requirements.md"
  ],
  "phase_or_sprint_docs_read": [
    "docs/plans/phase-c/sprint-c-1-runtime-migration.md"
  ],
  "requirement_checks": [
    {
      "id": "REQ-BDC-1002",
      "source": "docs/requirements.md:946",
      "listed_in_sprint_doc": true,
      "result": "violated",
      "evidence_refs": [
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-graph.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "docs/architecture.md:1148"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9. Direct sync->sc-command still exceeds authorized upstream-sc-cron-only exception; graph guard stays enabled and actual gate fails.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0804",
      "source": "docs/requirements.md:641",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0903",
      "source": "docs/requirements.md:757",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0001",
      "source": "docs/bd-cache-api/requirements.md:8",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0002",
      "source": "docs/bd-cache-api/requirements.md:32",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0003",
      "source": "docs/bd-cache-api/requirements.md:43",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0004",
      "source": "docs/bd-cache-api/requirements.md:60",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0005",
      "source": "docs/bd-cache-api/requirements.md:81",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0006",
      "source": "docs/bd-cache-api/requirements.md:100",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0007",
      "source": "docs/bd-cache-api/requirements.md:112",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0001",
      "source": "docs/requirements.md:63",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0006",
      "source": "docs/requirements.md:110",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0103",
      "source": "docs/requirements.md:156",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0107",
      "source": "docs/requirements.md:194",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0720",
      "source": "docs/requirements.md:493",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0722",
      "source": "docs/requirements.md:509",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0723",
      "source": "docs/requirements.md:517",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0724",
      "source": "docs/requirements.md:525",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0725",
      "source": "docs/requirements.md:533",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0727",
      "source": "docs/requirements.md:549",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0729",
      "source": "docs/requirements.md:565",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0730",
      "source": "docs/requirements.md:582",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "crates/bd-cache-sync/tests/cron_hurry.rs:277",
        "crates/bd-cache-store/src/schedule.rs:813",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0731",
      "source": "docs/requirements.md:590",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0732",
      "source": "docs/requirements.md:598",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0802",
      "source": "docs/requirements.md:616",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0803",
      "source": "docs/requirements.md:626",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0812",
      "source": "docs/requirements.md:719",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0911",
      "source": "docs/requirements.md:811",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "NFR-BDC-0002",
      "source": "docs/requirements.md:853",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "NFR-BDC-0004",
      "source": "docs/requirements.md:869",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "NFR-BDC-0005",
      "source": "docs/requirements.md:877",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "NFR-BDC-0006",
      "source": "docs/requirements.md:885",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "NFR-BDC-0007",
      "source": "docs/requirements.md:895",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "Cargo.lock:771",
        "crates/bd-cache-store/src/schedule.rs:95",
        "crates/bd-cache-store/tests/schedule_store.rs:448"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0002",
      "source": "docs/bd-cache-store/requirements.md:57",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0003",
      "source": "docs/bd-cache-store/requirements.md:79",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-API-0008",
      "source": "docs/bd-cache-api/requirements.md:127",
      "listed_in_sprint_doc": true,
      "result": "violated",
      "evidence_refs": [
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-graph.log",
        "docs/architecture.md:1148"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Direct sync->sc-command still exceeds authorized upstream-sc-cron-only exception; graph guard stays enabled and actual gate fails.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0001",
      "source": "docs/bd-cache-cli/requirements.md:14",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-run41776887223.json",
        "crates/bd-cache-cli/Cargo.toml:1",
        "docs/bd-cache-cli/requirements.md:14"
      ],
      "notes": "Exact frozen C1 core build/Clippy/test jobs now independently completed SUCCESS on Linux, macOS and Windows in run41776887223. Published exact0.1.1 default-features-off pin and version/usage contracts preserved on unchanged CLI source. This does not close the separate strict graph/composite CI gate.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0003",
      "source": "docs/bd-cache-cli/requirements.md:43",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0007",
      "source": "docs/bd-cache-cli/requirements.md:123",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0002",
      "source": "docs/bd-cache-py/requirements.md:30",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0006",
      "source": "docs/bd-cache-py/requirements.md:111",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0910",
      "source": "docs/requirements.md:803",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0721",
      "source": "docs/requirements.md:501",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0007",
      "source": "docs/requirements.md:118",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0008",
      "source": "docs/requirements.md:127",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0502",
      "source": "docs/requirements.md:365",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0002",
      "source": "docs/bd-cache-cli/requirements.md:31",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0004",
      "source": "docs/bd-cache-cli/requirements.md:57",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0005",
      "source": "docs/bd-cache-cli/requirements.md:78",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0006",
      "source": "docs/bd-cache-cli/requirements.md:104",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0008",
      "source": "docs/bd-cache-cli/requirements.md:139",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0009",
      "source": "docs/bd-cache-cli/requirements.md:160",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0010",
      "source": "docs/bd-cache-cli/requirements.md:174",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0011",
      "source": "docs/bd-cache-cli/requirements.md:186",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CLI-0012",
      "source": "docs/bd-cache-cli/requirements.md:202",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0001",
      "source": "docs/bd-cache-py/requirements.md:12",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0003",
      "source": "docs/bd-cache-py/requirements.md:52",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0004",
      "source": "docs/bd-cache-py/requirements.md:68",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0005",
      "source": "docs/bd-cache-py/requirements.md:89",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0007",
      "source": "docs/bd-cache-py/requirements.md:137",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0008",
      "source": "docs/bd-cache-py/requirements.md:149",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0010",
      "source": "docs/bd-cache-py/requirements.md:178",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0011",
      "source": "docs/bd-cache-py/requirements.md:194",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0012",
      "source": "docs/bd-cache-py/requirements.md:226",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0013",
      "source": "docs/bd-cache-py/requirements.md:244",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-PY-0014",
      "source": "docs/bd-cache-py/requirements.md:262",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0001",
      "source": "docs/bd-cache-store/requirements.md:9",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0004",
      "source": "docs/bd-cache-store/requirements.md:100",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0005",
      "source": "docs/bd-cache-store/requirements.md:136",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0006",
      "source": "docs/bd-cache-store/requirements.md:157",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0007",
      "source": "docs/bd-cache-store/requirements.md:185",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-STORE-0008",
      "source": "docs/bd-cache-store/requirements.md:211",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-SYNC-0001",
      "source": "docs/bd-cache-sync/requirements.md:10",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-SYNC-0004",
      "source": "docs/bd-cache-sync/requirements.md:91",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-SYNC-0005",
      "source": "docs/bd-cache-sync/requirements.md:109",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-SYNC-0010",
      "source": "docs/bd-cache-sync/requirements.md:222",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-SYNC-0012",
      "source": "docs/bd-cache-sync/requirements.md:433",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "crates/bd-cache-sync/tests/cron_hurry.rs:277",
        "crates/bd-cache-store/src/schedule.rs:813",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-SYNC-0013",
      "source": "docs/bd-cache-sync/requirements.md:467",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0001",
      "source": "docs/sc-cron/requirements.md:33",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0002",
      "source": "docs/sc-cron/requirements.md:53",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0003",
      "source": "docs/sc-cron/requirements.md:67",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0004",
      "source": "docs/sc-cron/requirements.md:78",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0005",
      "source": "docs/sc-cron/requirements.md:92",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0006",
      "source": "docs/sc-cron/requirements.md:123",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0007",
      "source": "docs/sc-cron/requirements.md:136",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0008",
      "source": "docs/sc-cron/requirements.md:152",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0009",
      "source": "docs/sc-cron/requirements.md:163",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0010",
      "source": "docs/sc-cron/requirements.md:185",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0011",
      "source": "docs/sc-cron/requirements.md:199",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0012",
      "source": "docs/sc-cron/requirements.md:219",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "crates/bd-cache-sync/tests/cron_hurry.rs:277",
        "crates/bd-cache-store/src/schedule.rs:813",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0013",
      "source": "docs/sc-cron/requirements.md:236",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0016",
      "source": "docs/sc-cron/requirements.md:275",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0017",
      "source": "docs/sc-cron/requirements.md:291",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0018",
      "source": "docs/sc-cron/requirements.md:317",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0014",
      "source": "docs/sc-cron/requirements.md:333",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance. Startup source supports actual owned stop/read/worker/cache joins and same-object reuse without changing public management policy; current independent focused producer/caller runtime acceptance is recorded by RustQA9.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-CRON-0015",
      "source": "docs/sc-cron/requirements.md:370",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "NFR-BDC-0001",
      "source": "docs/requirements.md:845",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "NFR-BDC-0003",
      "source": "docs/requirements.md:861",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/perf.rs:325",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-incremental.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0703",
      "source": "docs/requirements.md:450",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0726",
      "source": "docs/requirements.md:541",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    },
    {
      "id": "REQ-BDC-0728",
      "source": "docs/requirements.md:557",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "carried_forward_from": "5255610a16ef4500f0af508e29a533707a4a3e0a"
    }
  ],
  "deliverable_checks": [
    {
      "item": "Complete requirements list; all four deliverables trace to named requirements",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:18",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:225"
      ],
      "notes": "101 unique listed IDs resolve. All active owned-crate requirements are listed; omitted SYNC/PY historical IDs are explicitly superseded. Every repo NFR is listed.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D1: published 0.1.1 scheduler/command, rusqlite 0.39, one SQLite link provider",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "Cargo.toml:1",
        "Cargo.lock:1077",
        "Cargo.lock:1142",
        "Cargo.lock:1153"
      ],
      "notes": "Parsed lock contains exactly one registry package each: sc-command/sc-cron 0.1.1, rusqlite 0.39.0, libsqlite3-sys 0.37.0; defaults off in workspace declaration.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D1: local scheduler source/tests/workspace/publication retired",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "Cargo.toml:1",
        ".github/workflows/ci.yml:63",
        "docs/sc-cron/requirements.md:1"
      ],
      "notes": "crates/sc-cron does not exist; CI publish dry-run only sc-beads and sc-bd-cli. Historical CRON semantic contracts retained.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D1: authorized narrow CLI baseline amendment",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "docs/bd-cache-cli/requirements.md:14",
        "docs/architecture.md:1099"
      ],
      "notes": "Published baseline amendment records approval and preserves version/envelope/usage/exit bounds.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D2: exact upstream ScheduleStore on existing v3 connection, explicit limits",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:95",
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-store/src/schema.rs:6"
      ],
      "notes": "No local trait copy or standalone scheduler database; worker view shares Arc<Shared>. Additive schedule schema and active row limit remain.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D2: schedule state, generations, enabled flags, outcomes, lease/hook persistence",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/tests/schedule_store.rs:103",
        "crates/bd-cache-store/tests/schedule_store.rs:147",
        "crates/bd-cache-sync/tests/e2e.rs:311"
      ],
      "notes": "v3 fixture drains pending journal, preserves row id/generation/payload/flags, hook failures, identity, lease and user_version on reopen.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D2: target metadata round-trip/derived adapter and unsupported refusal",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:193",
        "crates/bd-cache-sync/src/launch.rs:16"
      ],
      "notes": "Metadata derived from actual persisted row; original payload kept. Unsupported native hook metadata is validated rather than silently discarded.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D3: published target APIs in one Scheduler, preserved execution signatures",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:680",
        "crates/bd-cache-sync/src/cron.rs:143",
        "crates/bd-cache-sync/src/launch.rs:35"
      ],
      "notes": "Same Scheduler owns worker/run-due. Public cron_run/cron_run_hurried, compose/execute/into_schedules surfaces remain.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D3: Python replacement, rollback, trust, mode lock and run identity",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-sync/tests/schedule_switch.rs:321",
        "crates/bd-cache-sync/tests/schedule_switch.rs:647",
        "crates/bd-cache-sync/src/launch.rs:16"
      ],
      "notes": "Existing suites and unchanged facade cover switching without dual enabled rows or payload identity spoofing.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D3: bounded graceful/hurried/zero shutdown, retained worker ownership",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "crates/bd-cache-sync/tests/cron_hurry.rs:130",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-store/src/schedule.rs:1074",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-store.log",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-7/startup-direct-results.json",
        "/tmp/phase-c-execution/c-1/qa-7/startup-old-negative.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json"
      ],
      "notes": "Production shutdown/startup core unchanged from adbcc88d and source re-reviewed: original T/2T, ordinary/zero/hurry stop, callback, actual retained read/worker/cache joins, no worker after startup stop, same-object reuse, rejected concurrent start and unrelated normal management/health policy preserved. Current exact-head independent producer/caller runtime proof and owner dispositions are recorded separately; no full C2 or stock-daemon HTTP claim.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D4: governed contract/logging exception and isolated graph checker",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "docs/architecture.md:1137",
        "docs/bd-cache-py/architecture.md:1",
        ".github/workflows/ci.yml:126",
        "crates/bd-cache-sync/tests/check_consumer_graph.py:1"
      ],
      "notes": "Governed exception and strict introducer-aware isolated graph checker exist, but actual Python consumer still has unauthorized bd-cache-sync->sc-command introducer at Cargo.toml31/cron_log.rs3. Published genuine receiver seam remains missing; no guard waiver or architecture expansion. REQ-QA-004, ARCH-001, RBQA-F001 and QA2-001 remain distinct open IDs.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "D4: actual installed wheel build and neutral 0.1 corpus matrix",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-7/parent-ci.json"
      ],
      "notes": "Historical installed-wheel/runner compatibility qualification retained only for unchanged surfaces/workflows. Current snapshot shows both wheel builds and Python Rust on3OS success, pytest matrix incomplete; full current platform gate remains separate and open.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Store-health seam: pre-move additive HealthReadControl and Composed.health_reads",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:37",
        "crates/bd-cache-sync/src/compose.rs:112"
      ],
      "notes": "Connection-bound handle exists without API/transport dependency or extra connection.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Store-health seam: lock, busy/progress/row deadline and typed errors",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/src/health.rs:89"
      ],
      "notes": "Deadline-aware mutex retry; SQLite busy/progress callbacks and row checks; existing STORE_FAILED cause, documented 50ms plus row tolerance.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Store-health seam: read-only scope and restoration on success/error/unwind",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:109",
        "crates/bd-cache-store/src/health.rs:320",
        "crates/bd-cache-store/src/schedule.rs:1011"
      ],
      "notes": "Settings restore while guard owned. Management exemption does not suppress health mutation refusal or deadline.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Store-health seam: actual contention, SQL work, decoding and normal scheduler writes",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:355",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "Existing owned-child mutex regression plus causal SQLite/writer and decoding tests passed in supplied qualification. No async timeout-only implementation.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Restart scope: exact view/control plus physical connection management isolation",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule_budget.rs:41",
        "crates/bd-cache-store/src/schedule_budget.rs:125",
        "crates/bd-cache-store/src/schedule.rs:1011",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-7/startup-direct-results.json",
        "/tmp/phase-c-execution/c-1/qa-7/startup-old-negative.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json"
      ],
      "notes": "Production shutdown/startup core unchanged from adbcc88d and source re-reviewed: original T/2T, ordinary/zero/hurry stop, callback, actual retained read/worker/cache joins, no worker after startup stop, same-object reuse, rejected concurrent start and unrelated normal management/health policy preserved. Current exact-head independent producer/caller runtime proof and owner dispositions are recorded separately; no full C2 or stock-daemon HTTP claim.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Restart scope: same-Scheduler immediate management notification",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:706",
        "crates/bd-cache-sync/tests/cron_hurry.rs:269",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-store/src/schedule.rs:1074",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-store.log",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-7/startup-direct-results.json",
        "/tmp/phase-c-execution/c-1/qa-7/startup-old-negative.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json"
      ],
      "notes": "Production shutdown/startup core unchanged from adbcc88d and source re-reviewed: original T/2T, ordinary/zero/hurry stop, callback, actual retained read/worker/cache joins, no worker after startup stop, same-object reuse, rejected concurrent start and unrelated normal management/health policy preserved. Current exact-head independent producer/caller runtime proof and owner dispositions are recorded separately; no full C2 or stock-daemon HTTP claim.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Restart scope: already-running mutex, busy SQL and executing SQL cancellation",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs:813",
        "crates/bd-cache-store/src/schedule.rs:857",
        "crates/bd-cache-store/src/schedule.rs:942",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-store/src/schedule.rs:1074",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-store.log",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log"
      ],
      "notes": "QA7 production and cron_hurry original lifecycle test bodies byte-identical to QA6; absolute2*T, expiry-at-T and original inner timing/assertions retained. New bounded inventory and safe raw Linux identity are confined to test session owner. FTQ010 closure belongs to owner.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Restart scope: reuse and rejected concurrent second start",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/src/cron.rs:262",
        "crates/bd-cache-sync/tests/cron_hurry.rs:159",
        "crates/bd-cache-sync/tests/cron_hurry.rs:277",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-store/src/schedule.rs:1074",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-store.log",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-7/startup-direct-results.json",
        "/tmp/phase-c-execution/c-1/qa-7/startup-old-negative.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json"
      ],
      "notes": "Production shutdown/startup core unchanged from adbcc88d and source re-reviewed: original T/2T, ordinary/zero/hurry stop, callback, actual retained read/worker/cache joins, no worker after startup stop, same-object reuse, rejected concurrent start and unrelated normal management/health policy preserved. Current exact-head independent producer/caller runtime proof and owner dispositions are recorded separately; no full C2 or stock-daemon HTTP claim.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-BDC-1002 dependency/CLI/wheel acceptance",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "Cargo.lock:1153",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json"
      ],
      "notes": "Published dependency/CLI/wheel implementation and historical actual installed-wheel qualification remain present for unchanged production, but requirement1002 narrow authorized consumer composition and exact-head full required gates are incomplete. No whole dependency/CLI/wheel acceptance claim.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-BDC-0103 pre-change v3 migration acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311"
      ],
      "notes": "Literal old v3 file with pending entry is migrated, synced, reopened and checked.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-BDC-0724 switch/rollback/health acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-sync/tests/schedule_switch.rs:321"
      ],
      "notes": "Real composition positive and failed-switch rollback proofs remain.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-BDC-0730 process/tree/lease/stop acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/cron_rows.rs:1",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "crates/bd-cache-cli/tests/sync_process.rs:1",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-7/startup-direct-results.json",
        "/tmp/phase-c-execution/c-1/qa-7/startup-old-negative.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json"
      ],
      "notes": "Production shutdown/startup core unchanged from adbcc88d and source re-reviewed: original T/2T, ordinary/zero/hurry stop, callback, actual retained read/worker/cache joins, no worker after startup stop, same-object reuse, rejected concurrent start and unrelated normal management/health policy preserved. Current exact-head independent producer/caller runtime proof and owner dispositions are recorded separately; no full C2 or stock-daemon HTTP claim.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-BDC-0001 outage/cache-hit latency acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:232",
        "crates/bd-cache-sync/tests/perf.rs:97"
      ],
      "notes": "Existing outage and row-touch/cache-hit suites; no changed path adds backend access to a cache hit.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-BDC-1002 / NFR-BDC-0004 store-health acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:355",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log"
      ],
      "notes": "Public bounded read, SQL, decoding and restoration tests present and qualification passed.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-API-0008 isolated consumer acceptance",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/check_consumer_graph.py:1",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-graph.log"
      ],
      "notes": "Governed exception and strict introducer-aware isolated graph checker exist, but actual Python consumer still has unauthorized bd-cache-sync->sc-command introducer at Cargo.toml31/cron_log.rs3. Published genuine receiver seam remains missing; no guard waiver or architecture expansion. REQ-QA-004, ARCH-001, RBQA-F001 and QA2-001 remain distinct open IDs.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "REQ-BDC-0721 / ANC-009 configured label and All scope acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/builtin_run.rs:1"
      ],
      "notes": "Scopes remain bd filters; v3 scenario includes configured label and All scopes, not phase definitions.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "CI prerequisite: both Linux wheel/test runners pinned compatibly",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        ".github/workflows/python.yml:71",
        ".github/workflows/python.yml:113",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-7/parent-ci.json"
      ],
      "notes": "Both Linux wheel-build/test runners remain pinnedubuntu24.04; current exact-C1 wheel jobs and Linux pytest3.9/3.11/3.13 pass. Historical installed-wheel corpus proof retained for unchanged wheel production; macOS pytest and full CI remain separate open qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Required formatting, locked Clippy modes, affected tests and docs",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/flaky-test-qa-result.json",
        "crates/bd-cache-sync/tests/common/session_fixture.rs:269",
        "/tmp/phase-c-execution/c-1/qa-8/helper-50ms-negative.json",
        "/tmp/phase-c-execution/c-1/qa-8/rust-qa-lifecycle.log",
        "/tmp/phase-c-execution/c-1/qa-8/flaky-retained-lifecycle-failure.log"
      ],
      "notes": "Current independent RustQA9 first-run exact-C1 affected lifecycle18/18(including five inventory tests), store5/5, strict scoped all-target/all-feature and default-library Clippy plus fmt pass. Old14/15 and13/15 retained; changed helper removes their identified50ms mechanism with deterministic delayed/noncooperative inventory regression and unchanged negative cleanup receipts. Historical full-workspace/docs proof carried only for unchanged respective production/doc sources; assignment explicitly calls for affected validation, not repetition of unchanged full1171/performance matrix. Hosted all-platform qualification remains open in its separate row. This is local changed-scope completion only; current C2 Windows shared-store readiness failure is expressly carried under platform row30 and REQ-QA-004, without an all-platform test acceptance claim.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Required full workspace three-OS ci and existing boundary/no-backend/advisory/publish gates",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        ".github/workflows/ci.yml:126",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-pr108-ci.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c1-boundary.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-run41776887223.json"
      ],
      "notes": "Required exact-C1 hosted check snapshot is independently recorded by RustQA9; boundary failure/pending or failed required checks prevent all-platform PASS. Actual current graph blocker is unchanged. Local Darwin and historical platform passes cannot close this row. Current C2 Windows shared-store unit group fails33/34 at schedule.rs1084 worker-operation readiness; source-equivalent store/helper, exact cause unproven, separate from earlier CtrlBreak history. C2 startup service7/7 passes there; Current C1 core3OS run41776887223 is independently SUCCESS; required strict graph composite remainsFAIL.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "NFR-BDC-0003 explicit migrated 1000-bead incremental release qualification",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/perf.rs:325",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-incremental.log"
      ],
      "notes": "Actual migrated schedule uses incremental cursor and changed row; 5.457458ms <1s.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Unit validation no live bd/Dolt; testbed-only live probes",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        ".github/workflows/ci.yml:149",
        "crates/bd-cache-sync/tests/common/repo.rs:1"
      ],
      "notes": "Deterministic fake composition in affected tests; no live backend invocation performed by reviewer.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    },
    {
      "item": "Independent QA, development anchors and green CI before integration",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:365",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:381"
      ],
      "notes": "Independent focused QA/runtime/caller progress is credited only within current reviewer receipts. Full sprint still has genuine composition/graph/CI findings, incomplete deliverables and no complete current development-anchor acceptance. No current QA/anchor/green-CI integration closeout or PR merge.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance."
    }
  ],
  "findings": [
    {
      "id": "REQ-QA-004",
      "severity": "Blocking",
      "category": "acceptance-gap",
      "source_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:365",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:381",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:504",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:678"
      ],
      "target_refs": [
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        ".github/workflows/ci.yml:126",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-pr108-ci.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log",
        "crates/bd-cache-store/src/schedule.rs:1084"
      ],
      "issue": "At exact C1 a753d2799183dd4e341c854c4515bf046447eb6c, unauthorized direct sync->sc-command composition and the required isolated-consumer graph/hosted CI gate remain unresolved. All101 requirements and34 deliverables assessed;29/34 complete, five open: governed logging/graph composition, dependency/CLI/wheel acceptance, isolated consumer acceptance, full current hosted CI and independent QA/anchor/integration closeout. Current focused changed-helper/lifecycle validation earns completion credit; retained old14/15 and13/15 failures are not erased. Startup producer/public-caller verification is separately credited under current owner receipts and does not substitute for whole sprint acceptance. Current source-equivalent store fixture also has actual C2 Windows readiness failure33/34 at schedule.rs1084; exact cause unknown, routed as platform acceptance evidence, not mislabeled as signal failure. C1 core Linux/macOS/Windows current jobs are independently SUCCESS; this credit is retained and is not represented as overall CI green. The distinct Windows test observation is canonical RQA9-001, still open with its owner.",
      "required_correction": "Resolve the genuine approved public receiver composition within the existing authorized contract, preserving canonical scheduler events, then remove the forbidden direct edge and pass the unchanged actual isolated graph and all required exact-head hosted ci/python gates. Complete remaining independent QA/development anchors and integration closeout. Keep distinct canonical IDs and prior failure history; do not widen deadlines, suppress guards, use SDK/publisher/auth workarounds or infer whole C2/stock-daemon HTTP acceptance from focused caller proof. Investigate and correct the actual Windows store-fixture readiness failure, preserving original causal/cleanup budgets, then obtain required platform qualification; do not guess its cause or retry/relax it to green.",
      "compliance_result": "partially-compliant"
    }
  ],
  "finding_dispositions": [
    {
      "id": "REQ-QA-004",
      "disposition": "open",
      "owner": "req-qa",
      "notes": "Five full-sprint acceptance gaps remain; focused runtime/helper success is credited separately.",
      "evidence_refs": [
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        ".github/workflows/ci.yml:126",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-pr108-ci.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log",
        "crates/bd-cache-store/src/schedule.rs:1084"
      ]
    },
    {
      "id": "ARCH-001",
      "disposition": "open",
      "owner": "arch-qa",
      "notes": "Current independent architecture receipt reconfirms unauthorized live introducer; no Approved/Active amendment.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "docs/architecture.md:1148",
        "crates/bd-cache-sync/Cargo.toml:31"
      ]
    },
    {
      "id": "RBQA-F001",
      "disposition": "open",
      "owner": "ruthless-boundary-qa / retained canonical owner",
      "notes": "Distinct correlated boundary ID retained; production receiver/dependency unchanged. No source evidence of correction.",
      "evidence_refs": [
        ".triage/c/findings/RBQA-F001.ttl:2",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json"
      ]
    },
    {
      "id": "QA2-001",
      "disposition": "open",
      "owner": "rust-qa-agent",
      "notes": "Actual current C1 hosted isolated consumer gate fails at unchanged forbidden introducer after checker4/4 and actual API graph pass.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c1-boundary.log"
      ]
    },
    {
      "id": "RSH-001",
      "disposition": "resolution_verified_for_C1_producer_and_actual_C2_public_caller; canonical_owning_adjudication_retained",
      "owner": "service/runtime owning reviewer and enforcer",
      "notes": "The previous missing-consumption/public-caller objection is resolved: frozen C2 b7de8e9 has same adbcc88d production startup core; independent current C1 lifecycle18/18, C2 service7/7 and actually rebuilt unchanged public probe250.876416ms<original500ms. Normal management remains pending until rollback; callback/readiness and real worker/readiness/probe/management joins preserved. Architecture owner confirms source remediation. This req reviewer has no remaining narrow startup acceptance objection and does not reopen solely from old status. Canonical owner/enforcer records govern formal closure; full C2, stock-daemon HTTP and all-platform acceptance are not inferred.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-c2-service.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-c2-rebuilt-probe.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-c2-rebuild-commands.json"
      ]
    },
    {
      "id": "FTQ-011",
      "disposition": "resolution_verified_in_source_and_current_affected_validation; canonical_flaky_owner_adjudication_retained",
      "owner": "flaky-test-qa",
      "notes": "50ms ambient per-invocation requirement removed; original250ms/1s cleanup/5s lifetime and false failure receipt maintained. Fresh stable stopped-session census/root join/final complete empty census required. Current independent first-run18/18(including five inventory cases) plus retained real50ms-mutant/negative process audits support correction. No requirement-level helper objection remains; owning flaky reviewer/enforcer formal status governs. Original14/15 and13/15 failures remain evidence, not waived or reinterpreted as passes.",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/common/session_fixture.rs:119",
        "crates/bd-cache-sync/tests/common/session_fixture.rs:269",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-8/helper-50ms-negative.log",
        "/tmp/phase-c-execution/c-1/qa-8/helper-postpush-probes.json"
      ]
    },
    {
      "id": "FTQ-009",
      "disposition": "prior_owned_closure_preserved_for_original_cleanup_mechanism",
      "notes": "Accepted store/helper cleanup source unchanged and independent current Darwin5/5 passed. Actual C2 Windows shared-store supervisor readiness failure is retained as a new observed platform acceptance gap under REQ-QA-004; it does not establish detachment or erase original scoped ownership proof. No claim of all-platform qualification.",
      "evidence_refs": [
        ".triage/c/findings/FTQ-009.ttl:6",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-store.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log"
      ]
    },
    {
      "id": "FTQ-010",
      "disposition": "prior_owned_closure_preserved_for_original_cleanup_mechanisms",
      "notes": "Session ownership before Python registration, in-flight launch closure, bounded owned inventory/output, actual timeout/unwind reap, safe Linux raw IDs and single cleanup handoff remain. All four developer negative audits retain exit101 and empty PID/group/session/inventory survivors before fallback. FTQ011 independent correction and platform qualification assessed separately.",
      "evidence_refs": [
        ".triage/c/findings/FTQ-010.ttl",
        "/tmp/phase-c-execution/c-1/qa-8/helper-postpush-probes.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-lifecycle.log"
      ]
    },
    {
      "id": "RQA9-001",
      "disposition": "open_new_distinct_canonical_finding",
      "owner": "rust-qa-agent; owning flaky-test adjudication pending",
      "notes": "Actual C2 Windows shared-store fixture misses required busy-persistence operation acknowledgement and fails33/34. Current C1 three-OS core and Darwin5/5 success do not erase it. No scheduling, orphan, signal or source-regression cause inferred. Separate from original FTQ009 repair and from FTQ011/RSH startup proof.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log",
        "crates/bd-cache-store/src/schedule.rs:1084"
      ]
    }
  ],
  "related_owner_context": {
    "ARCH-001": {
      "disposition": "open",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "docs/architecture.md:1148",
        "crates/bd-cache-sync/Cargo.toml:31"
      ]
    },
    "RBQA-F001": {
      "disposition": "open",
      "evidence_refs": [
        ".triage/c/findings/RBQA-F001.ttl:2",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json"
      ]
    },
    "QA2-001": {
      "disposition": "open",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c1-boundary.log"
      ]
    },
    "RSH-001": {
      "disposition": "resolution_verified_for_C1_producer_and_actual_C2_public_caller; canonical_owning_adjudication_retained",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-c2-service.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-c2-rebuilt-probe.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-c2-rebuild-commands.json"
      ]
    },
    "FTQ-011": {
      "disposition": "resolution_verified_in_source_and_current_affected_validation; canonical_flaky_owner_adjudication_retained",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/common/session_fixture.rs:119",
        "crates/bd-cache-sync/tests/common/session_fixture.rs:269",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-8/helper-50ms-negative.log",
        "/tmp/phase-c-execution/c-1/qa-8/helper-postpush-probes.json"
      ]
    },
    "FTQ-009": {
      "disposition": "prior_owned_closure_preserved_for_original_cleanup_mechanism",
      "evidence_refs": [
        ".triage/c/findings/FTQ-009.ttl:6",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-store.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log"
      ]
    },
    "FTQ-010": {
      "disposition": "prior_owned_closure_preserved_for_original_cleanup_mechanisms",
      "evidence_refs": [
        ".triage/c/findings/FTQ-010.ttl",
        "/tmp/phase-c-execution/c-1/qa-8/helper-postpush-probes.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-lifecycle.log"
      ]
    },
    "RQA9-001": {
      "id": "RQA9-001",
      "disposition": "open_new_distinct_canonical_finding",
      "owner": "rust-qa-agent; owning flaky-test adjudication pending",
      "notes": "Actual C2 Windows shared-store fixture misses required busy-persistence operation acknowledgement and fails33/34. Current C1 three-OS core and Darwin5/5 success do not erase it. No scheduling, orphan, signal or source-regression cause inferred. Separate from original FTQ009 repair and from FTQ011/RSH startup proof.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log",
        "crates/bd-cache-store/src/schedule.rs:1084"
      ]
    }
  },
  "summary": {
    "total_findings": 1,
    "blocking_findings": 1,
    "overall_compliance": "non-compliant",
    "deliverables_total": 34,
    "deliverables_complete": 29,
    "deliverables_incomplete": 5,
    "deliverable_completion_percent": 85.29,
    "requirement_total": 101,
    "requirement_results": {
      "violated": 2,
      "upheld": 99
    },
    "finding_count_scope": "Req-qa owned findings only; all carried canonical IDs, scoped verified resolutions, retained prior closures and new RQA9-001 are separately included in finding_dispositions."
  },
  "verification_notes": [
    "Following cm rule b-mr7cp6x0-ipdnhs: exact-head supported-platform CI is assessed separately from Darwin local evidence. cm context completed; degraded historical search did not block this review.",
    "Exact frozen clean C1 branch/HEAD verified before and after inspection. Read full current authoritative sprint, all101 checks,41 listed ADR resolutions, quality policy, requirements/architecture/project-plan baselines, per-crate requirements, prior complete QA8 report, all six current triage histories and retained FTQ009/010 owner/enforcer closures.",
    "Independently compared all159 crate src files plus9 crate manifests, root Cargo manifests/lockfile, workflows and governing requirement baselines against QA8 adbcc88d; identical. Test helper, cron_hurry callsite deadlines, sprint appendices and triage/evidence changed; no whole-source equivalence claimed. Existing production-preservation qualification is carried only for actual unchanged paths.",
    "Reviewed actual startup path: same bounded raw worker view/read future retained through stop/hurry and actual read join; original T/2T deadlines, callback, no worker after startup stop, reset only after join, private lifetime claim, unrelated public management and health policies unchanged. No new SDK port, scheduler copy or caller factory.",
    "Reviewed helper correction: inventory receives caller remaining absolute budget, output/parse checks refuse incomplete proof, owned child and output reaped on completion/error/unwind; no separate50ms cap. Existing100ms preliminary drain,250ms quiescence,1s total cleanup and5s fixture lifetime remain. Errors require fresh complete stable stopped-session censuses, actual root wait and final complete empty-session census; failed required proof leaves receipt false and cleanup cannot restart its budget.",
    "Retained actual developer14/15 and independentRust13/15 failures, actual old50ms-mutant exit101/TimedOut, original QA5 PATH-cat/QA6 SIGSTOP counterexamples and all four current developer audit logs. Counterexamples remain failed tests, not passing fixtures; empty PID/group/session/inventory survivor receipts occur before driver fallback. Changed-source regression success does not erase original failures.",
    "Read C2 candidate receipt and verified every listed raw-log hash. Frozen C2 b7de8e9 consumes adbcc88d production core; candidate Application/daemon developer21/21 and unchanged public-probe251.883875ms are development evidence. Current independent Rust QA9 caller/probe evidence is attributed separately. Generic retained-management Application caller proof does not establish a stock-daemon HTTP reproduction or full C2 sprint acceptance.",
    "No full1171/performance/release-consumer rerun; no Cargo/source/triage changes, agents, dependency changes, graph waiver, publisher/auth/token work, merge or external comments by this reviewer. Original historical installed-wheel/performance/docs proof is retained only for unchanged corresponding production. Current CI, independent QA/anchors and merge remain separate gates.",
    "Current C2 Windows job126397662727 actual shared-store supervisor failure at schedule.rs1084 is preserved without guessing scheduling, SDK signal failure or root cause. C2 service startup7/7 passes in the same hosted job. Later exact C1 core3OS completion run41776887223 independently SUCCESS supersedes pending core state; strict graph composite stillFAILS and full all-required acceptance remains open.",
    "Current independent Rust receipt also replays all four unchanged source-free negative drivers successfully: actual fixture exits101 and empty survivor evidence before fallback. These are negative cleanup proofs, not healthy passing fixture claims. New Important RQA9-001 remains independently open for actual C2 Windows shared-store readiness failure. Owning flaky/servicing/enforcer canonical adjudication is pending for scoped FTQ011/RSH resolutions; req-qa has no remaining narrow implementation objection."
  ],
  "gate_reason": "FAIL: full authoritative sprint is29/34(85.29%), with five incomplete/open deliverables, REQ-BDC-1002 and REQ-API-0008 composition violations, and current required composite CI/QA/anchor gates unclosed (C1 core three-OS jobs are independently SUCCESS). Focused changed-helper and actual startup caller success do not close these independent gaps.",
  "source_equivalence": {
    "artifact": "/tmp/phase-c-execution/c-1/qa-9/req-qa-source-equivalence.json",
    "comparison_commit": "adbcc88d3dc134ae94403bc2af7a8db76ba921a5",
    "production_src_files": 159,
    "production_manifests": 9,
    "production_src_manifests_byte_identical": true,
    "whole_source_byte_identical": false,
    "changed_tests": [
      "crates/bd-cache-sync/tests/common/session_fixture.rs",
      "crates/bd-cache-sync/tests/cron_hurry.rs"
    ],
    "limitations": "Actual test helper/callsites changed and independently inspected. C2 includes unchanged production startup core, but retains old helper tests and is not source-identical as a whole."
  },
  "model": {
    "configured": "Composer (unavailable)",
    "actual": "gpt-6.1-sol",
    "reasoning_effort": "high",
    "fallback_disclosed": true
  },
  "review_role": "Independent req-qa; source/evidence review, no Cargo execution",
  "architecture_alignment": {
    "receipt": "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
    "reviewed_commit": "a753d2799183dd4e341c854c4515bf046447eb6c",
    "listed_adrs_resolved": 41,
    "listed_adr_results": {
      "upheld": 40,
      "violated": 1
    },
    "violation": "ADR-BDC-0016 live direct sync->sc-command introducer",
    "draft_not_applicable": "ADR-BDC-0008",
    "assessment_attribution": "ADR compliance is current independent arch-qa work. Req-qa resolved all41 references and checked source/plan alignment in its relevant changed startup/helper/boundary paths; no substitution for architecture owner."
  },
  "current_ci": {
    "source": "/tmp/phase-c-execution/c-1/qa-9/rust-qa-run41776887223.json",
    "exact_head": "a753d2799183dd4e341c854c4515bf046447eb6c",
    "snapshot_only": true,
    "c1_concrete_failure": {
      "cause": "actual isolated Python graph rejects direct sync->sc-command introducer; API graph passes and checker4/4",
      "raw_log": "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c1-boundary.log",
      "job_url": "https://github.com/randlee/beads-cache/actions/runs/41776887223/job/126399209278",
      "job_id": 126399209278
    },
    "qualification": "Exact C1 core Linux/macOS/Windows build/Clippy/tests are independently SUCCESS. Required composite job stillFAILS actual isolated Python graph. Earlier PR-rollup counts remain historical, with incomplete macOS pytest; no full current all-required-CI acceptance.",
    "earlier_PR_snapshot": "/tmp/phase-c-execution/c-1/qa-9/rust-qa-pr108-ci.json",
    "completed_core_platform_jobs": [
      {
        "name": "fmt, doc, publish dry-run, boundaries",
        "status": "completed",
        "conclusion": "failure",
        "databaseId": 126399209278,
        "url": "https://github.com/randlee/beads-cache/actions/runs/41776887223/job/126399209278"
      },
      {
        "name": "clippy and test (macos-latest)",
        "status": "completed",
        "conclusion": "success",
        "databaseId": 126399209361,
        "url": "https://github.com/randlee/beads-cache/actions/runs/41776887223/job/126399209361"
      },
      {
        "name": "clippy and test (windows-latest)",
        "status": "completed",
        "conclusion": "success",
        "databaseId": 126399209441,
        "url": "https://github.com/randlee/beads-cache/actions/runs/41776887223/job/126399209441"
      },
      {
        "name": "clippy and test (ubuntu-latest)",
        "status": "completed",
        "conclusion": "success",
        "databaseId": 126399209473,
        "url": "https://github.com/randlee/beads-cache/actions/runs/41776887223/job/126399209473"
      }
    ],
    "earlier_PR_snapshot_counts": {
      "FAILURE": 1,
      "SUCCESS": 8,
      "IN_PROGRESS": 4,
      "QUEUED": 2
    },
    "current_core_run_counts": {
      "SUCCESS": 3,
      "FAILURE": 1
    },
    "current_core_run_counts_scope": "Four core workflow jobs only, not the full Python/check rollup; exact C1 core3OS success plus strict graph/composite failure."
  },
  "supplemental_platform_gap": {
    "id": "REQ-QA-004",
    "source_head": "b7de8e9879503353879c500edacf12a6832d709d",
    "job_id": 126397662727,
    "test": "schedule::tests::shutdown_fixture_supervisor_kills_and_reaps_each_stalled_cleanup",
    "failure": "worker operation did not run",
    "location": "crates/bd-cache-store/src/schedule.rs:1084",
    "observed_result": "33passed/1failed in store unit group; actual C2 service startup/regression7/7 passed earlier in same job",
    "raw_log": "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log",
    "attribution": "C2 hosted job on old adbcc88d store/helper blobs identical to current C1 for this store fixture. Actual portability/readiness gap; root cause not established. This is not evidence of current CtrlBreak failure or product shutdown regression. Existing historical signal finding remains separately owned.",
    "canonical_id": "RQA9-001",
    "owning_receipt": "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
    "qualification": "New Important distinct canonical shared-store fixture finding; C1 current Windows core passes, C2 current Windows fails. No waiver or inferred cause."
  },
  "canonical_owner_receipts": {
    "rust": {
      "path": "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
      "RSH-001": "independently verified pending owner; canonical open until owning adjudication",
      "FTQ-011": "correction independently verified pending flaky owner; canonical open until owning adjudication"
    },
    "architecture": {
      "path": "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
      "ADR-BDC-0016": "violated; unchanged live forbidden introducer"
    },
    "scope": "Req-qa evidence assessment does not mutate triage or issue canonical owner/enforcer closure."
  }
}
```
