```json
{
  "status": "FAIL",
  "reviewed_branch": "sprint/c-1-runtime-migration",
  "reviewed_commit": "8c1aefafd7fd269c82c7974e610e848816d026d8",
  "errors": [],
  "scope": {
    "phase": "c",
    "sprint": "c-1"
  },
  "model": {
    "configured": "Composer",
    "configured_available": false,
    "actual": "gpt-6.1-sol",
    "reasoning_effort": "high",
    "qualification": "Honest available fallback; no claim configured model ran."
  },
  "review_role": "independent req-qa; source/evidence only; no Cargo/tests/source or triage mutation/subagents/PR posting/merge",
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
        "docs/architecture.md:1148",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log:560"
      ],
      "notes": "The approved upstream-only contract/logging exception remains violated by direct bd-cache-sync -> sc-command; actual exact-head isolated Python graph fails. Tracked in Blocking REQ-QA-004 and distinct owner IDs ARCH-001/RBQA-F001/QA2-001.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0001",
      "source": "docs/bd-cache-api/requirements.md:8",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0002",
      "source": "docs/bd-cache-api/requirements.md:32",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0003",
      "source": "docs/bd-cache-api/requirements.md:43",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0004",
      "source": "docs/bd-cache-api/requirements.md:60",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0005",
      "source": "docs/bd-cache-api/requirements.md:81",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0006",
      "source": "docs/bd-cache-api/requirements.md:100",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0007",
      "source": "docs/bd-cache-api/requirements.md:112",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/bd-cache-api/requirements.md:127",
        "crates/bd-cache-sync/src/schedule.rs:690",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "crates/bd-cache-store/src/schedule.rs",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Supported C1 producer and paired rebuilt public C2 startup correction remains credited through exact production equivalence and owner receipts. Historical RQA9 and current macOS RQA10 remain failed test qualification, not proven production violations; they block platform/CI acceptance separately. No stock HTTP or whole C2 proof inferred.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "NFR-BDC-0007",
      "source": "docs/requirements.md:895",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "Cargo.lock:771",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/tests/schedule_store.rs:448",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0002",
      "source": "docs/bd-cache-store/requirements.md:57",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0003",
      "source": "docs/bd-cache-store/requirements.md:79",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-API-0008",
      "source": "docs/bd-cache-api/requirements.md:127",
      "listed_in_sprint_doc": true,
      "result": "violated",
      "evidence_refs": [
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "docs/architecture.md:1148",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log:560"
      ],
      "notes": "The approved upstream-only contract/logging exception remains violated by direct bd-cache-sync -> sc-command; actual exact-head isolated Python graph fails. Tracked in Blocking REQ-QA-004 and distinct owner IDs ARCH-001/RBQA-F001/QA2-001.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0001",
      "source": "docs/bd-cache-cli/requirements.md:14",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-run41776887223.json",
        "crates/bd-cache-cli/Cargo.toml:1",
        "docs/bd-cache-cli/requirements.md:14",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0003",
      "source": "docs/bd-cache-cli/requirements.md:43",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0007",
      "source": "docs/bd-cache-cli/requirements.md:123",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0006",
      "source": "docs/bd-cache-py/requirements.md:111",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0002",
      "source": "docs/bd-cache-cli/requirements.md:31",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0004",
      "source": "docs/bd-cache-cli/requirements.md:57",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0005",
      "source": "docs/bd-cache-cli/requirements.md:78",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0006",
      "source": "docs/bd-cache-cli/requirements.md:104",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0008",
      "source": "docs/bd-cache-cli/requirements.md:139",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0009",
      "source": "docs/bd-cache-cli/requirements.md:160",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0010",
      "source": "docs/bd-cache-cli/requirements.md:174",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0011",
      "source": "docs/bd-cache-cli/requirements.md:186",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CLI-0012",
      "source": "docs/bd-cache-cli/requirements.md:202",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-cli/Cargo.toml:1",
        ".github/workflows/ci.yml:157",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0001",
      "source": "docs/bd-cache-py/requirements.md:12",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0003",
      "source": "docs/bd-cache-py/requirements.md:52",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0004",
      "source": "docs/bd-cache-py/requirements.md:68",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0005",
      "source": "docs/bd-cache-py/requirements.md:89",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0007",
      "source": "docs/bd-cache-py/requirements.md:137",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0008",
      "source": "docs/bd-cache-py/requirements.md:149",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0010",
      "source": "docs/bd-cache-py/requirements.md:178",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0011",
      "source": "docs/bd-cache-py/requirements.md:194",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0012",
      "source": "docs/bd-cache-py/requirements.md:226",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0013",
      "source": "docs/bd-cache-py/requirements.md:244",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-PY-0014",
      "source": "docs/bd-cache-py/requirements.md:262",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        ".github/workflows/python.yml:40",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Current exact-head Python Rust3OS, wheel2OS and six installed-wheel pytest jobs are all SUCCESS; core macos-latest remains independently failed.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0001",
      "source": "docs/bd-cache-store/requirements.md:9",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0004",
      "source": "docs/bd-cache-store/requirements.md:100",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0005",
      "source": "docs/bd-cache-store/requirements.md:136",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0006",
      "source": "docs/bd-cache-store/requirements.md:157",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0007",
      "source": "docs/bd-cache-store/requirements.md:185",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-STORE-0008",
      "source": "docs/bd-cache-store/requirements.md:211",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/tests/schedule_store.rs:401",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-SYNC-0004",
      "source": "docs/bd-cache-sync/requirements.md:91",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-SYNC-0005",
      "source": "docs/bd-cache-sync/requirements.md:109",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-SYNC-0010",
      "source": "docs/bd-cache-sync/requirements.md:222",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "crates/bd-cache-store/src/schedule.rs",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Supported C1 producer and paired rebuilt public C2 startup correction remains credited through exact production equivalence and owner receipts. Historical RQA9 and current macOS RQA10 remain failed test qualification, not proven production violations; they block platform/CI acceptance separately. No stock HTTP or whole C2 proof inferred.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-SYNC-0013",
      "source": "docs/bd-cache-sync/requirements.md:467",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:632",
        "crates/bd-cache-sync/src/cron.rs:201",
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0001",
      "source": "docs/sc-cron/requirements.md:33",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0002",
      "source": "docs/sc-cron/requirements.md:53",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0003",
      "source": "docs/sc-cron/requirements.md:67",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0004",
      "source": "docs/sc-cron/requirements.md:78",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0005",
      "source": "docs/sc-cron/requirements.md:92",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0006",
      "source": "docs/sc-cron/requirements.md:123",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0007",
      "source": "docs/sc-cron/requirements.md:136",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0008",
      "source": "docs/sc-cron/requirements.md:152",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0009",
      "source": "docs/sc-cron/requirements.md:163",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0010",
      "source": "docs/sc-cron/requirements.md:185",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0011",
      "source": "docs/sc-cron/requirements.md:199",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "crates/bd-cache-store/src/schedule.rs",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Supported C1 producer and paired rebuilt public C2 startup correction remains credited through exact production equivalence and owner receipts. Historical RQA9 and current macOS RQA10 remain failed test qualification, not proven production violations; they block platform/CI acceptance separately. No stock HTTP or whole C2 proof inferred.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0013",
      "source": "docs/sc-cron/requirements.md:236",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0016",
      "source": "docs/sc-cron/requirements.md:275",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0017",
      "source": "docs/sc-cron/requirements.md:291",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0018",
      "source": "docs/sc-cron/requirements.md:317",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0014",
      "source": "docs/sc-cron/requirements.md:333",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance. Supported C1 producer and paired rebuilt public C2 startup correction remains credited through exact production equivalence and owner receipts. Historical RQA9 and current macOS RQA10 remain failed test qualification, not proven production violations; they block platform/CI acceptance separately. No stock HTTP or whole C2 proof inferred.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "REQ-CRON-0015",
      "source": "docs/sc-cron/requirements.md:370",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "docs/sc-cron/requirements.md:1",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:35",
        "crates/bd-cache-sync/tests/cron_hurry.rs:115",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    },
    {
      "id": "NFR-BDC-0003",
      "source": "docs/requirements.md:861",
      "listed_in_sprint_doc": true,
      "result": "upheld",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/perf.rs:325",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-incremental.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
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
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "QA10 independently verifies all mandatory baseline bytes against reviewed QA9 and 158 whole production src.rs files; the changed schedule.rs cfg(test) module is excluded from blanket equality, while its production prefix is equal. Requirement behavior and listed source paths are preserved; this is source/preservation evidence, not full current CI or sprint acceptance.",
      "carried_forward_from": "a753d2799183dd4e341c854c4515bf046447eb6c"
    }
  ],
  "deliverable_checks": [
    {
      "item": "Complete requirements list; all four deliverables trace to named requirements",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:18",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:225",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "101 unique listed IDs resolve. All active owned-crate requirements are listed; omitted SYNC/PY historical IDs are explicitly superseded. Every repo NFR is listed. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D01",
      "requirement_ids": [
        "REQ-BDC-1002"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D1: published 0.1.1 scheduler/command, rusqlite 0.39, one SQLite link provider",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "Cargo.toml:1",
        "Cargo.lock:1077",
        "Cargo.lock:1142",
        "Cargo.lock:1153",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Parsed lock contains exactly one registry package each: sc-command/sc-cron 0.1.1, rusqlite 0.39.0, libsqlite3-sys 0.37.0; defaults off in workspace declaration. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D02",
      "requirement_ids": [
        "REQ-BDC-1002"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D1: local scheduler source/tests/workspace/publication retired",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "Cargo.toml:1",
        ".github/workflows/ci.yml:63",
        "docs/sc-cron/requirements.md:1",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "crates/sc-cron does not exist; CI publish dry-run only sc-beads and sc-bd-cli. Historical CRON semantic contracts retained. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D03",
      "requirement_ids": [
        "REQ-BDC-1002"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D1: authorized narrow CLI baseline amendment",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "docs/bd-cache-cli/requirements.md:14",
        "docs/architecture.md:1099",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Published baseline amendment records approval and preserves version/envelope/usage/exit bounds. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D04",
      "requirement_ids": [
        "REQ-CLI-0001"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D2: exact upstream ScheduleStore on existing v3 connection, explicit limits",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/schema.rs:6",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "No local trait copy or standalone scheduler database; worker view shares Arc<Shared>. Additive schedule schema and active row limit remain. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D05",
      "requirement_ids": [
        "REQ-BDC-1002",
        "REQ-STORE-0003"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D2: schedule state, generations, enabled flags, outcomes, lease/hook persistence",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/tests/schedule_store.rs:103",
        "crates/bd-cache-store/tests/schedule_store.rs:147",
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "v3 fixture drains pending journal, preserves row id/generation/payload/flags, hook failures, identity, lease and user_version on reopen. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D06",
      "requirement_ids": [
        "REQ-STORE-0003",
        "REQ-BDC-0103"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D2: target metadata round-trip/derived adapter and unsupported refusal",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/launch.rs:16",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Metadata derived from actual persisted row; original payload kept. Unsupported native hook metadata is validated rather than silently discarded. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D07",
      "requirement_ids": [
        "REQ-BDC-1002"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D3: published target APIs in one Scheduler, preserved execution signatures",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:680",
        "crates/bd-cache-sync/src/cron.rs:143",
        "crates/bd-cache-sync/src/launch.rs:35",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Same Scheduler owns worker/run-due. Public cron_run/cron_run_hurried, compose/execute/into_schedules surfaces remain. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D08",
      "requirement_ids": [
        "REQ-BDC-0720",
        "REQ-BDC-1002"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D3: Python replacement, rollback, trust, mode lock and run identity",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-sync/tests/schedule_switch.rs:321",
        "crates/bd-cache-sync/tests/schedule_switch.rs:647",
        "crates/bd-cache-sync/src/launch.rs:16",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Existing suites and unchanged facade cover switching without dual enabled rows or payload identity spoofing. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D09",
      "requirement_ids": [
        "REQ-BDC-0724",
        "REQ-BDC-0725"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
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
        "crates/bd-cache-store/src/schedule.rs",
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
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "Production and original causal assertions are present, with supported startup/post-start scope preserved by exact production equivalence and actual owner receipts. Current changed shared-helper/protocol controls pass8/8 plus default-health2/2; this credits implementation and bounded ownership, including error/unwind/no-retry. Original Windows RQA9 qualification remains unresolved, and exact current macOS lifecycle inventory17/18 failure is separately retained in D31; neither proves a production shutdown regression. Full platform acceptance is not credited here.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D10",
      "requirement_ids": [
        "REQ-BDC-0730",
        "REQ-CRON-0012"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D4: governed contract/logging exception and isolated graph checker",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "docs/architecture.md:1137",
        "docs/bd-cache-py/architecture.md:1",
        ".github/workflows/ci.yml:126",
        "crates/bd-cache-sync/tests/check_consumer_graph.py:1",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log:560",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json"
      ],
      "notes": "Governed narrow exception and strict checker exist; actual exact8c isolated Python consumer rejects unauthorized direct sync -> sc-command after checker4/4 and API isolated graph PASS. Genuine published canonical receiver seam remains unresolved. No guard waiver or new SDK/interface solution exists.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D11",
      "requirement_ids": [
        "REQ-API-0008",
        "REQ-BDC-1002"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "D4: actual installed wheel build and neutral 0.1 corpus matrix",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json",
        ".github/workflows/python.yml:40"
      ],
      "notes": "Current exact-head Python matrix is complete: Rust/clippy3OS, actual wheelsLinux/macOS and installed-wheel pytest3.9/3.11/3.13 on both OSes all SUCCESS. This closes the wheel/corpus validation item while isolated dependency acceptance and whole ci workflow remain separately open.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D12",
      "requirement_ids": [
        "REQ-PY-0010",
        "REQ-BDC-0812"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Store-health seam: pre-move additive HealthReadControl and Composed.health_reads",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:37",
        "crates/bd-cache-sync/src/compose.rs:112",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Connection-bound handle exists without API/transport dependency or extra connection. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D13",
      "requirement_ids": [
        "REQ-BDC-1002",
        "NFR-BDC-0004"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Store-health seam: lock, busy/progress/row deadline and typed errors",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:204",
        "crates/bd-cache-store/src/health.rs:89",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Deadline-aware mutex retry; SQLite busy/progress callbacks and row checks; existing STORE_FAILED cause, documented 50ms plus row tolerance. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D14",
      "requirement_ids": [
        "NFR-BDC-0004"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Store-health seam: read-only scope and restoration on success/error/unwind",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:109",
        "crates/bd-cache-store/src/health.rs:320",
        "crates/bd-cache-store/src/schedule.rs",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Settings restore while guard owned. Management exemption does not suppress health mutation refusal or deadline. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D15",
      "requirement_ids": [
        "NFR-BDC-0004"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Store-health seam: actual contention, SQL work, decoding and normal scheduler writes",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:355",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Existing owned-child mutex regression plus causal SQLite/writer and decoding tests passed in supplied qualification. No async timeout-only implementation. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D16",
      "requirement_ids": [
        "NFR-BDC-0004"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Restart scope: exact view/control plus physical connection management isolation",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule_budget.rs:41",
        "crates/bd-cache-store/src/schedule_budget.rs:125",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/src/cron.rs:159",
        "crates/bd-cache-sync/src/cron.rs:282",
        "crates/bd-cache-sync/src/schedule.rs:569",
        "crates/bd-cache-sync/tests/cron_hurry.rs:753",
        "/tmp/phase-c-execution/c-1/qa-7/startup-direct-results.json",
        "/tmp/phase-c-execution/c-1/qa-7/startup-old-negative.log",
        "/tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-9/arch-qa-result.json",
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "Production and original causal assertions are present, with supported startup/post-start scope preserved by exact production equivalence and actual owner receipts. Current changed shared-helper/protocol controls pass8/8 plus default-health2/2; this credits implementation and bounded ownership, including error/unwind/no-retry. Original Windows RQA9 qualification remains unresolved, and exact current macOS lifecycle inventory17/18 failure is separately retained in D31; neither proves a production shutdown regression. Full platform acceptance is not credited here.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D17",
      "requirement_ids": [
        "REQ-BDC-0730",
        "NFR-BDC-0004"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Restart scope: same-Scheduler immediate management notification",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/src/schedule.rs:706",
        "crates/bd-cache-sync/tests/cron_hurry.rs:269",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-store/src/schedule.rs",
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
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "Production and original causal assertions are present, with supported startup/post-start scope preserved by exact production equivalence and actual owner receipts. Current changed shared-helper/protocol controls pass8/8 plus default-health2/2; this credits implementation and bounded ownership, including error/unwind/no-retry. Original Windows RQA9 qualification remains unresolved, and exact current macOS lifecycle inventory17/18 failure is separately retained in D31; neither proves a production shutdown regression. Full platform acceptance is not credited here.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D18",
      "requirement_ids": [
        "REQ-CRON-0014"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Restart scope: already-running mutex, busy SQL and executing SQL cancellation",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-store/tests/support/fixture_child.rs:18",
        "crates/bd-cache-store/src/schedule.rs",
        "crates/bd-cache-sync/tests/cron_hurry.rs:481",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-store.log",
        "/tmp/phase-c-execution/c-1/qa-4/fix-postpush-lifecycle.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "Production and original causal assertions are present, with supported startup/post-start scope preserved by exact production equivalence and actual owner receipts. Current changed shared-helper/protocol controls pass8/8 plus default-health2/2; this credits implementation and bounded ownership, including error/unwind/no-retry. Original Windows RQA9 qualification remains unresolved, and exact current macOS lifecycle inventory17/18 failure is separately retained in D31; neither proves a production shutdown regression. Full platform acceptance is not credited here.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D19",
      "requirement_ids": [
        "REQ-BDC-0730",
        "REQ-CRON-0012"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
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
        "crates/bd-cache-store/src/schedule.rs",
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
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "Production and original causal assertions are present, with supported startup/post-start scope preserved by exact production equivalence and actual owner receipts. Current changed shared-helper/protocol controls pass8/8 plus default-health2/2; this credits implementation and bounded ownership, including error/unwind/no-retry. Original Windows RQA9 qualification remains unresolved, and exact current macOS lifecycle inventory17/18 failure is separately retained in D31; neither proves a production shutdown regression. Full platform acceptance is not credited here.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D20",
      "requirement_ids": [
        "REQ-CRON-0012"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "REQ-BDC-1002 dependency/CLI/wheel acceptance",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "Cargo.lock:1153",
        "/tmp/phase-c-execution/c-1/qa-4/parent-ci.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log:560"
      ],
      "notes": "Published dependency/CLI/wheel implementation present; exact current wheel/Python matrix fully SUCCESS and core Linux/WindowsSUCCESS. Approved isolated composition still violates the narrow introducer rule and current core macOS fails, so full1002 dependency/CLI/wheel acceptance remains incomplete.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D21",
      "requirement_ids": [
        "REQ-BDC-1002",
        "REQ-CLI-0001",
        "REQ-PY-0010"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "REQ-BDC-0103 pre-change v3 migration acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Literal old v3 file with pending entry is migrated, synced, reopened and checked. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D22",
      "requirement_ids": [
        "REQ-BDC-0103"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "REQ-BDC-0724 switch/rollback/health acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/schedule_switch.rs:242",
        "crates/bd-cache-sync/tests/schedule_switch.rs:321",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Real composition positive and failed-switch rollback proofs remain. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D23",
      "requirement_ids": [
        "REQ-BDC-0724"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
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
        "/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "Production and original causal assertions are present, with supported startup/post-start scope preserved by exact production equivalence and actual owner receipts. Current changed shared-helper/protocol controls pass8/8 plus default-health2/2; this credits implementation and bounded ownership, including error/unwind/no-retry. Original Windows RQA9 qualification remains unresolved, and exact current macOS lifecycle inventory17/18 failure is separately retained in D31; neither proves a production shutdown regression. Full platform acceptance is not credited here.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D24",
      "requirement_ids": [
        "REQ-BDC-0730"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "REQ-BDC-0001 outage/cache-hit latency acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:232",
        "crates/bd-cache-sync/tests/perf.rs:97",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Existing outage and row-touch/cache-hit suites; no changed path adds backend access to a cache hit. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D25",
      "requirement_ids": [
        "REQ-BDC-0001",
        "NFR-BDC-0001"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "REQ-BDC-1002 / NFR-BDC-0004 store-health acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-store/src/health.rs:355",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-focused-tests.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Public bounded read, SQL, decoding and restoration tests present and qualification passed. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D26",
      "requirement_ids": [
        "REQ-BDC-1002",
        "NFR-BDC-0004"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "REQ-API-0008 isolated consumer acceptance",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/check_consumer_graph.py:1",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-graph.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json",
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log:560",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json"
      ],
      "notes": "Governed narrow exception and strict checker exist; actual exact8c isolated Python consumer rejects unauthorized direct sync -> sc-command after checker4/4 and API isolated graph PASS. Genuine published canonical receiver seam remains unresolved. No guard waiver or new SDK/interface solution exists.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D27",
      "requirement_ids": [
        "REQ-API-0008"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "REQ-BDC-0721 / ANC-009 configured label and All scope acceptance",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/e2e.rs:311",
        "crates/bd-cache-sync/tests/builtin_run.rs:1",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Scopes remain bd filters; v3 scenario includes configured label and All scopes, not phase definitions. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D28",
      "requirement_ids": [
        "REQ-BDC-0721"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "CI prerequisite: both Linux wheel/test runners pinned compatibly",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        ".github/workflows/python.yml:71",
        ".github/workflows/python.yml:113",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json"
      ],
      "notes": "Both Linux wheel-build and installed-wheel-test runners remain ubuntu24.04; current exact-head wheel plus all six Python3.9/3.11/3.13 installed-wheel tests are SUCCESS. Python3.9 retained. Separate macos-latest core failure does not revoke this runner prerequisite completion.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D29",
      "requirement_ids": [
        "REQ-BDC-0812",
        "REQ-BDC-0910"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Required local affected formatting, locked Clippy modes, changed tests and warning-free docs evidence",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-commands.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-direct-negative.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-current-run.json"
      ],
      "notes": "Current independent RustQA10: fmt/private fmt, strict affected storeall-targets/all-features, defaultlib and actual cron_hurry helper-compat Clippy PASS; changed shutdown8/8 plus default-health2/2 PASS. Direct planted missing-marker negative truly exits101 in5.06444975s with DatabaseBusy return and actual root cleanup before fallback(false). Hosted exact-head Format/Docs stepsSUCCESS. Earlier full-workspace validation is carried only for unchanged source; assignment expressly disallows replay of unchanged full1171/perf/lifecycle/App suite. This is local affected validation completion, not required all-platform suite acceptance; D31 remains open for RQA9/RQA10 and graph failure.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D30",
      "requirement_ids": [
        "NFR-BDC-0005"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Required full workspace three-OS ci and existing boundary/no-backend/advisory/publish gates",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-current-run.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-windows-job-final.json",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-macos.log:1572",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log:560",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "notes": "Required full current CI incomplete: exact8c1aefa coreLinuxSUCCESS andWindowsSUCCESS, coremacOSFAIL17/18cron_hurry with owned-session inventory timeout (RQA10-001), and isolated-consumer boundaryFAIL. Current Python matrix11/11SUCCESS retained. Format/docs/advisories/publish/simple boundary stepsPASS, but later script/strictconsumer/no-backend steps skipped after graph failure. Original Windows store33/34RQA9 remains unresolved despite later currentWindowsPASS. macOS raw cannot distinguish cleanup/quiescence/finaldrain inventory from separate final250ms audit; no orphan/host-load/50ms recurrence inferred. No all-platform success.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D31",
      "requirement_ids": [
        "REQ-BDC-1002",
        "REQ-CLI-0001",
        "NFR-BDC-0005"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "NFR-BDC-0003 explicit migrated 1000-bead incremental release qualification",
      "status": "present",
      "closure_state": "closed",
      "evidence_refs": [
        "crates/bd-cache-sync/tests/perf.rs:325",
        "/tmp/phase-c-execution/c-1/qa-4/restart-postpush-incremental.log",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Actual migrated schedule uses incremental cursor and changed row; 5.457458ms <1s. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D32",
      "requirement_ids": [
        "NFR-BDC-0003"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Unit validation no live bd/Dolt; testbed-only live probes",
      "status": "present",
      "closure_state": "not-applicable",
      "evidence_refs": [
        ".github/workflows/ci.yml:149",
        "crates/bd-cache-sync/tests/common/repo.rs:1",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ],
      "notes": "Deterministic fake composition in affected tests; no live backend invocation performed by reviewer. QA10: current production/source and unchanged acceptance paths independently compared to QA9; scoped prior runtime evidence is retained without relabeling historical passes as current full qualification.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D33",
      "requirement_ids": [
        "NFR-BDC-0005",
        "NFR-BDC-0006"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    },
    {
      "item": "Independent QA, development anchors and green CI before integration",
      "status": "partially-present",
      "closure_state": "open",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:217",
        "docs/project-plan.md:65"
      ],
      "notes": "Current independent Rust/flaky/arch reports are completed FAIL. No full current development-anchor receipt exists because sprint acceptance remains incomplete. Local controls, prior startup proof and Python11/11SUCCESS do not close current requirements/graph/CI or independent QA/anchor/integration gates; sprint remains in_progress and no merge is claimed.",
      "verification_basis": "QA9 independently verified unchanged governed production/baseline blobs against full QA8 assessment; current helper and deadline callsites separately reviewed. Historical proof is scoped preservation evidence, not current-head CI acceptance.",
      "check_id": "C1-D34",
      "requirement_ids": [
        "REQ-BDC-1002"
      ],
      "assessed_at": "8c1aefafd7fd269c82c7974e610e848816d026d8"
    }
  ],
  "findings": [
    {
      "id": "REQ-QA-004",
      "severity": "Blocking",
      "category": "acceptance-gap",
      "source_refs": [
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:225",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:365",
        "docs/plans/phase-c/sprint-c-1-runtime-migration.md:381",
        "docs/architecture.md:1148",
        "docs/bd-cache-api/requirements.md:127"
      ],
      "target_refs": [
        "crates/bd-cache-sync/Cargo.toml:31",
        "crates/bd-cache-sync/src/cron_log.rs:3",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log:560",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-macos.log:1572",
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ],
      "issue": "At exact frozen clean C1 8c1aefafd7fd269c82c7974e610e848816d026d8, approved contract/logging composition still has the forbidden direct sync -> sc-command edge and actual isolated Python consumer failure. Current core Linux/Windows and all11Python jobsSUCCESS; core macOS required lifecycle suiteFAIL17/18 with unresolved inventory timeoutRQA10-001. Historical Windows store qualificationRQA9-001 remains unresolved: new phase protocol accurately classifies a planted actual returned-worker/missing-marker failure, but does not identify or repair the original cause. Full101 requirements and34deliverables re-assessed: 29/34complete with five open composition/acceptance/CI/QA-anchor items. No full current development-anchor or integration closeout. Source and local8+2/control progress receive scoped credit; prior failures remain failed evidence.",
      "required_correction": "Resolve canonical logging composition within the existing approved public receiver contract, remove the unauthorized edge and pass the unchanged actual isolated consumer gate. Preserve and causally adjudicate originalRQA9 and currentRQA10 under original budgets/owned cleanup; obtain all required exact-head hosted gates and independent QA/development-anchor/integration closeout. No SDK/publisher/auth workaround, deadline relaxation, rerun-to-green waiver, fabricated events or wholeC2/stockHTTP inference.",
      "compliance_result": "partially-compliant",
      "incomplete_deliverable_ids": [
        "C1-D11",
        "C1-D21",
        "C1-D27",
        "C1-D31",
        "C1-D34"
      ]
    }
  ],
  "finding_dispositions": [
    {
      "id": "REQ-QA-004",
      "severity": "Blocking",
      "owner": "req-qa",
      "disposition": "open",
      "notes": "Open full-sprint acceptance finding, independently re-assessed here.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ]
    },
    {
      "id": "ARCH-001",
      "severity": "Blocking",
      "owner": "arch-qa",
      "disposition": "open",
      "notes": "Current actual architecture fenceFAIL1B: approved upstream-only receiver composition violation; no new structure defect.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ]
    },
    {
      "id": "RBQA-F001",
      "severity": "Blocking",
      "owner": "retained ruthless-boundary-qa",
      "disposition": "open",
      "notes": "Distinct carried original canonical boundary finding; actual production edge remains and current architecture/source proof supports open disposition.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ]
    },
    {
      "id": "QA2-001",
      "severity": "Important",
      "owner": "rust-qa-agent",
      "disposition": "open",
      "notes": "Current actual isolated Python graph failure; owner severityImportant retained.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ]
    },
    {
      "id": "RQA9-001",
      "severity": "Important",
      "owner": "rust-qa-agent and flaky-test-qa",
      "disposition": "open",
      "notes": "Original C2Windows qualification remains open/unlocalized. New diagnostic/control is not historical reproduction or repair; silence never proves non-entry.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ]
    },
    {
      "id": "RQA10-001",
      "severity": "Important",
      "owner": "rust-qa-agent and flaky-test-qa",
      "disposition": "open",
      "notes": "Distinct actual current macOS lifecycle inventory timeout17/18. Cleanup/quiescence/finaldrain or separatepost-cleanup250ms audit unresolved. No orphan/host-load/50ms-cap recurrence or shared-observer regression proven.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json"
      ]
    },
    {
      "id": "FTQ-009",
      "disposition": "scoped_owned_closure_preserved",
      "owner": "original owning reviewer/enforcer; current fences corroborate",
      "notes": "Original unbounded fixture lifetime mechanism closure preserved. Current8/8 and2default-health controls retain actual root kill/wait, ordinary no-op observer, observer error/panic, failedreceipt and refused repeat cleanup; RQA9 remains separately open.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ]
    },
    {
      "id": "FTQ-010",
      "disposition": "scoped_owned_closure_preserved",
      "owner": "original owning reviewer/enforcer; current fences corroborate",
      "notes": "Original session-orphan/inventory ownership mechanism closure preserved. Current session_fixture/cron_hurry wholebytes match QA9, existing one-attempt session handoff preserved; changed shared include strict compatibility and default health checksPASS.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ]
    },
    {
      "id": "FTQ-011",
      "disposition": "scoped_owned_closure_preserved",
      "owner": "original owning reviewer/enforcer; current fences corroborate",
      "notes": "Original independent50ms cap defect closure preserved per actual current flaky owner fence: helper byte-identicalQA9 with caller absolute budget, complete stable/final censuses, actual root wait and one-attempt cleanup. Original14/15 and13/15 failures retained. Current macOSRQA10 failure separately open and does not establish originalcap recurrence.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ]
    },
    {
      "id": "RSH-001",
      "disposition": "scoped_owned_closure_preserved",
      "owner": "original owning reviewer/enforcer; current fences corroborate",
      "notes": "Supported C1 producer and paired actual rebuilt public C2 caller startup/poststart correction closure preserved per actual owner/enforcer and current Rust/flaky/arch evidence. Exact production source equal; prior public250.876416ms<500ms, causal management/readiness/worker joins retained. No wholeC2/stockHTTP/allplatform inference.",
      "evidence_refs": [
        "/tmp/phase-c-execution/c-1/qa-10/rust-qa-agent-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/arch-qa-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/req-qa-source-equivalence.json"
      ]
    }
  ],
  "summary": {
    "total_findings": 1,
    "blocking_findings": 1,
    "important_findings": 0,
    "minor_findings": 0,
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
    "finding_count_scope": "Req-qa owned findings only. Related owner open union separately:3Blocking3Important0Minor, six distinct canonical IDs; not duplicate req findings.",
    "related_owner_open_counts": {
      "Blocking": 3,
      "Important": 3,
      "Minor": 0
    }
  },
  "source_equivalence": {
    "head": "8c1aefafd7fd269c82c7974e610e848816d026d8",
    "clean": true,
    "baseline": "a753d2799183dd4e341c854c4515bf046447eb6c",
    "whole_src_rs_equal_count": 158,
    "whole_src_rs_changed": [
      "crates/bd-cache-store/src/schedule.rs"
    ],
    "schedule_production_prefix_equal": true,
    "schedule_production_prefix_sha256": "49438f00b7310fb4d385355dbc887ab3c353264eed25e05df9f824e5872da05a",
    "equal_files": [
      "crates/bd-cache-api/src/data/beads.rs",
      "crates/bd-cache-api/src/data/entry.rs",
      "crates/bd-cache-api/src/data/manage.rs",
      "crates/bd-cache-api/src/data/mod.rs",
      "crates/bd-cache-api/src/data/sync.rs",
      "crates/bd-cache-api/src/data/x_cache.rs",
      "crates/bd-cache-api/src/decode.rs",
      "crates/bd-cache-api/src/error.rs",
      "crates/bd-cache-api/src/filter.rs",
      "crates/bd-cache-api/src/lib.rs",
      "crates/bd-cache-api/src/ops/beads.rs",
      "crates/bd-cache-api/src/ops/manage.rs",
      "crates/bd-cache-api/src/ops/mod.rs",
      "crates/bd-cache-api/src/ops/scopes.rs",
      "crates/bd-cache-api/src/page.rs",
      "crates/bd-cache-api/src/request/beads.rs",
      "crates/bd-cache-api/src/request/manage.rs",
      "crates/bd-cache-api/src/request/mod.rs",
      "crates/bd-cache-api/src/request/sync.rs",
      "crates/bd-cache-api/src/v0.rs",
      "crates/bd-cache-cli/src/args.rs",
      "crates/bd-cache-cli/src/bin/bd-cache.rs",
      "crates/bd-cache-cli/src/bin/bdc.rs",
      "crates/bd-cache-cli/src/bin/fake-bd.rs",
      "crates/bd-cache-cli/src/bin/fake-python.rs",
      "crates/bd-cache-cli/src/commands/beads.rs",
      "crates/bd-cache-cli/src/commands/cron.rs",
      "crates/bd-cache-cli/src/commands/manage.rs",
      "crates/bd-cache-cli/src/commands/mod.rs",
      "crates/bd-cache-cli/src/commands/runtime.rs",
      "crates/bd-cache-cli/src/commands/schedule.rs",
      "crates/bd-cache-cli/src/commands/scopes.rs",
      "crates/bd-cache-cli/src/commands/signal.rs",
      "crates/bd-cache-cli/src/commands/sync.rs",
      "crates/bd-cache-cli/src/commands/tick.rs",
      "crates/bd-cache-cli/src/config.rs",
      "crates/bd-cache-cli/src/help.rs",
      "crates/bd-cache-cli/src/lib.rs",
      "crates/bd-cache-cli/src/log.rs",
      "crates/bd-cache-cli/src/render/json.rs",
      "crates/bd-cache-cli/src/render/mod.rs",
      "crates/bd-cache-cli/src/render/text.rs",
      "crates/bd-cache-cli/src/request.rs",
      "crates/bd-cache-cli/src/scenario.rs",
      "crates/bd-cache-model/src/budget.rs",
      "crates/bd-cache-model/src/call.rs",
      "crates/bd-cache-model/src/compat.rs",
      "crates/bd-cache-model/src/config.rs",
      "crates/bd-cache-model/src/error.rs",
      "crates/bd-cache-model/src/ingest.rs",
      "crates/bd-cache-model/src/journal.rs",
      "crates/bd-cache-model/src/lib.rs",
      "crates/bd-cache-model/src/limits.rs",
      "crates/bd-cache-model/src/op.rs",
      "crates/bd-cache-model/src/pyjson.rs",
      "crates/bd-cache-model/src/reply.rs",
      "crates/bd-cache-model/src/scope.rs",
      "crates/bd-cache-model/src/state.rs",
      "crates/bd-cache-model/src/store.rs",
      "crates/bd-cache-model/src/testing/contract.rs",
      "crates/bd-cache-model/src/testing/mod.rs",
      "crates/bd-cache-model/src/testing/store.rs",
      "crates/bd-cache-py/src/api.rs",
      "crates/bd-cache-py/src/compose.rs",
      "crates/bd-cache-py/src/convert.rs",
      "crates/bd-cache-py/src/error.rs",
      "crates/bd-cache-py/src/handle.rs",
      "crates/bd-cache-py/src/lib.rs",
      "crates/bd-cache-py/src/phase_b.rs",
      "crates/bd-cache-py/src/slot.rs",
      "crates/bd-cache-store/src/codec.rs",
      "crates/bd-cache-store/src/error.rs",
      "crates/bd-cache-store/src/health.rs",
      "crates/bd-cache-store/src/lib.rs",
      "crates/bd-cache-store/src/location.rs",
      "crates/bd-cache-store/src/lock.rs",
      "crates/bd-cache-store/src/registry.rs",
      "crates/bd-cache-store/src/schedule_budget.rs",
      "crates/bd-cache-store/src/schema.rs",
      "crates/bd-cache-store/src/store.rs",
      "crates/bd-cache-sync/src/compose.rs",
      "crates/bd-cache-sync/src/control.rs",
      "crates/bd-cache-sync/src/cron.rs",
      "crates/bd-cache-sync/src/cron_log.rs",
      "crates/bd-cache-sync/src/front_end.rs",
      "crates/bd-cache-sync/src/health.rs",
      "crates/bd-cache-sync/src/history.rs",
      "crates/bd-cache-sync/src/hook.rs",
      "crates/bd-cache-sync/src/job.rs",
      "crates/bd-cache-sync/src/launch.rs",
      "crates/bd-cache-sync/src/lease.rs",
      "crates/bd-cache-sync/src/lib.rs",
      "crates/bd-cache-sync/src/mode_lock.rs",
      "crates/bd-cache-sync/src/module_hook.rs",
      "crates/bd-cache-sync/src/payload.rs",
      "crates/bd-cache-sync/src/python_job.rs",
      "crates/bd-cache-sync/src/run.rs",
      "crates/bd-cache-sync/src/schedule.rs",
      "crates/bd-cache-sync/src/text.rs",
      "crates/bd-cache-sync/src/trust.rs",
      "crates/bd-cache/src/backend_check.rs",
      "crates/bd-cache/src/drain.rs",
      "crates/bd-cache/src/get.rs",
      "crates/bd-cache/src/ingest.rs",
      "crates/bd-cache/src/lib.rs",
      "crates/bd-cache/src/list.rs",
      "crates/bd-cache/src/miss.rs",
      "crates/bd-cache/src/outage.rs",
      "crates/bd-cache/src/prune.rs",
      "crates/bd-cache/src/ready.rs",
      "crates/bd-cache/src/refresh.rs",
      "crates/bd-cache/src/resolve.rs",
      "crates/bd-cache/src/scopes.rs",
      "crates/bd-cache/src/status.rs",
      "crates/bd-cache/src/sync.rs",
      "crates/bd-cache/src/view.rs",
      "crates/bd-cache/src/write.rs",
      "crates/sc-bd-cli/src/argv.rs",
      "crates/sc-bd-cli/src/backend.rs",
      "crates/sc-bd-cli/src/bounded.rs",
      "crates/sc-bd-cli/src/cache.rs",
      "crates/sc-bd-cli/src/command.rs",
      "crates/sc-bd-cli/src/dolt.rs",
      "crates/sc-bd-cli/src/lib.rs",
      "crates/sc-bd-cli/src/locks.rs",
      "crates/sc-bd-cli/src/ops.rs",
      "crates/sc-bd-cli/src/path.rs",
      "crates/sc-bd-cli/src/probe.rs",
      "crates/sc-bd-cli/src/process_group.rs",
      "crates/sc-bd-cli/src/run.rs",
      "crates/sc-bd-cli/src/runner.rs",
      "crates/sc-bd-cli/src/shutdown.rs",
      "crates/sc-bd-cli/src/testing/fake_bd.rs",
      "crates/sc-bd-cli/src/testing/mod.rs",
      "crates/sc-bd-cli/src/testing/recording.rs",
      "crates/sc-beads/src/backend.rs",
      "crates/sc-beads/src/cache.rs",
      "crates/sc-beads/src/clock.rs",
      "crates/sc-beads/src/compat.rs",
      "crates/sc-beads/src/detect.rs",
      "crates/sc-beads/src/error.rs",
      "crates/sc-beads/src/gates.rs",
      "crates/sc-beads/src/issues.rs",
      "crates/sc-beads/src/lib.rs",
      "crates/sc-beads/src/logging.rs",
      "crates/sc-beads/src/parse.rs",
      "crates/sc-beads/src/row.rs",
      "crates/sc-beads/src/row/generated.rs",
      "crates/sc-beads/src/test_support.rs",
      "crates/sc-beads/src/testing/clock.rs",
      "crates/sc-beads/src/testing/contract.rs",
      "crates/sc-beads/src/testing/fake.rs",
      "crates/sc-beads/src/testing/mod.rs",
      "crates/sc-beads/src/types/backend.rs",
      "crates/sc-beads/src/types/cli.rs",
      "crates/sc-beads/src/types/fs.rs",
      "crates/sc-beads/src/types/issue.rs",
      "crates/sc-beads/src/types/payload.rs"
    ]
  },
  "baseline_equivalence": [
    {
      "path": ".claude/project/quality-policy.md",
      "equal_to_QA9": true,
      "sha256": "328c92e3b39d12912e4a561dffa0166f137147b347fc17372731dbbf336960ac"
    },
    {
      "path": "docs/requirements.md",
      "equal_to_QA9": true,
      "sha256": "8f781db453f279c75b200f68891ac6e2af2ed01db288f4eb75ac7b24d1358678"
    },
    {
      "path": "docs/architecture.md",
      "equal_to_QA9": true,
      "sha256": "18f9c855d732770243f756d4819a2a7e88c36f616048409f786f9ad2cd9efc1d"
    },
    {
      "path": "docs/project-plan.md",
      "equal_to_QA9": true,
      "sha256": "806d5ce6a6982e7a6b91ff551ae53ace17c80eff9b61abc13b397d901a95ea4c"
    },
    {
      "path": "docs/bd-cache-store/requirements.md",
      "equal_to_QA9": true,
      "sha256": "be7fcb1e0ea62ebce31db8227a802a89309a8abef86547c8a5221750b69bce65"
    },
    {
      "path": "docs/bd-cache-sync/requirements.md",
      "equal_to_QA9": true,
      "sha256": "65fc39922dff0f06e67ccbafd09aaa3285499ad97dfebc1f22686db4a79b72a0"
    },
    {
      "path": "docs/bd-cache-cli/requirements.md",
      "equal_to_QA9": true,
      "sha256": "6f0d1e1ace38339630eb1f496fbd26109fa2f31071303c842264970d68f6bc67"
    },
    {
      "path": "docs/bd-cache-py/requirements.md",
      "equal_to_QA9": true,
      "sha256": "1cd4219771746a889e098898b524b75ccd3e266a928291b0650390c7902f0447"
    },
    {
      "path": "docs/bd-cache-api/requirements.md",
      "equal_to_QA9": true,
      "sha256": "e9ac4708165c27ac85078ad8d7d2dfe4a6542be0b892deccabcce5250e37918b"
    },
    {
      "path": "docs/sc-cron/requirements.md",
      "equal_to_QA9": true,
      "sha256": "fc6a9ef08c08209fee915f5f8fbb71ba224dda35b56d6227bbc4a1e881b582aa"
    }
  ],
  "verification_notes": [
    "ACK and cm context completed; applicable memory b-mr7cp6x0-ipdnhs keeps platform evidence separate.",
    "All101 unique listed IDs resolve, every repoNFR listed, and all currently binding owned-crate IDs listed; omittedSYNC0002/0003/0006/0007/0008/0009/0011 andPY0009 explicitly superseded.",
    "Table recomputed item-by-item: openD11,D21,D27,D31,D34;29/34 is the resulting count, not a forced carry-forward. Product/source behavior preservation rows do not assert full all-platform qualification; all required test/platform acceptance remains in D31.",
    "Actual current Rust/flaky/arch JSON matched their fenced Markdown before use; independent raw hosted logs, current final checks and local owner command/control evidence read.",
    "No fresh test was run by this reviewer. Historical source-equivalent acceptance is explicitly preservation evidence only; full current CI/QA/anchor gate remainsFAIL.",
    "Frozen checkout verified clean exact assigned branch/HEAD before and after analysis."
  ],
  "current_ci": {
    "evidence": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-pr108-checks-final.json",
    "checks_total": 15,
    "success": 13,
    "failure": 2,
    "core": {
      "Linux": "SUCCESS",
      "Windows": "SUCCESS",
      "macOS": "FAILURE"
    },
    "python_all11": "SUCCESS",
    "isolated_consumer_graph": "FAILURE",
    "full_current_ci_green": false
  },
  "gate_reason": "FAIL: 29/34deliverables complete; REQ-BDC-1002/REQ-API-0008 composition violations and open exact-head isolated graph/macOS required CI, historicalRQA9 qualification and QA/anchor/integration gates. Current local8+2/direct-negative and fullPython successes are credited in their actual scopes."
}
```
