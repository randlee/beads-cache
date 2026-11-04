# QA Findings Update

Generated: 2026-11-04T03:17:20.659037+00:00
QA Pass: qa-7
Sprint/Task: c-1 / qa-c-1-7
Branch: `sprint/c-1-runtime-migration`
Commit: `5255610a16ef4500f0af508e29a533707a4a3e0a`
PR: #108
Verdict: **FAIL**

## Machine Status (JSON)
```json
{
  "sprint": "c-1",
  "task": "qa-c-1-7",
  "branch": "sprint/c-1-runtime-migration",
  "commit": "5255610a16ef4500f0af508e29a533707a4a3e0a",
  "pr": 108,
  "verdict": "FAIL",
  "deliverables": {
    "complete": 29,
    "total": 34,
    "percent": 85.29
  },
  "findings": {
    "blocking": 3,
    "important": 1,
    "minor": 0
  },
  "blocking_ids": [
    "REQ-QA-004",
    "ARCH-001",
    "RBQA-F001"
  ],
  "merge_readiness": "not ready",
  "merge_reason": "Four open findings (3 Blocking, 1 Important), 29/34 deliverables, and failing boundary CI; whole sprint/integration/anchor/merge/release acceptance remains blocked.",
  "next_action": "triage_and_fix",
  "owner": "parent-orchestrator",
  "reviewer_spawn_gate": "pass",
  "reviewer_manifest": [
    {
      "agent": "flaky-test-qa",
      "task_id": "/root/c1_flaky5",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "pass",
      "findings": {
        "blocking": 0,
        "important": 0,
        "minor": 0
      }
    },
    {
      "agent": "req-qa",
      "task_id": "/root/c1_req6",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "FAIL",
      "findings": {
        "blocking": 1,
        "important": 0,
        "minor": 0
      }
    },
    {
      "agent": "arch-qa",
      "task_id": "/root/c1_arch6",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "FAIL",
      "findings": {
        "blocking": 1,
        "important": 0,
        "minor": 0
      }
    },
    {
      "agent": "rust-qa-agent",
      "task_id": "/root/c1_rustqa6",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "findings",
      "findings": {
        "blocking": 0,
        "important": 1,
        "minor": 0
      }
    }
  ],
  "missing_reviewers": [],
  "unparsed_reviewers": [],
  "evidence_chain": {
    "qa_pass": "qa-7",
    "commit": "5255610a16ef4500f0af508e29a533707a4a3e0a",
    "pr_number": 108,
    "pr_comment_url": "https://github.com/randlee/beads-cache/pull/108#issuecomment-6621585641",
    "coordinator_task_id": "/root/c1_quality_manager7",
    "reviewer_tasks": [
      {
        "agent": "flaky-test-qa",
        "task_id": "/root/c1_flaky5",
        "spawn_actor": "parent-orchestrator",
        "invocation": "parent_followup_existing_independent_reviewer",
        "fenced_json_received": true,
        "verdict": "pass",
        "findings": {
          "blocking": 0,
          "important": 0,
          "minor": 0
        }
      },
      {
        "agent": "req-qa",
        "task_id": "/root/c1_req6",
        "spawn_actor": "parent-orchestrator",
        "invocation": "parent_followup_existing_independent_reviewer",
        "fenced_json_received": true,
        "verdict": "FAIL",
        "findings": {
          "blocking": 1,
          "important": 0,
          "minor": 0
        }
      },
      {
        "agent": "arch-qa",
        "task_id": "/root/c1_arch6",
        "spawn_actor": "parent-orchestrator",
        "invocation": "parent_followup_existing_independent_reviewer",
        "fenced_json_received": true,
        "verdict": "FAIL",
        "findings": {
          "blocking": 1,
          "important": 0,
          "minor": 0
        }
      },
      {
        "agent": "rust-qa-agent",
        "task_id": "/root/c1_rustqa6",
        "spawn_actor": "parent-orchestrator",
        "invocation": "parent_followup_existing_independent_reviewer",
        "fenced_json_received": true,
        "verdict": "findings",
        "findings": {
          "blocking": 0,
          "important": 1,
          "minor": 0
        }
      }
    ],
    "triage": {
      "phase_id": "c",
      "ttl_paths": [
        ".triage/c/findings/REQ-QA-004.ttl",
        ".triage/c/findings/ARCH-001.ttl",
        ".triage/c/findings/RBQA-F001.ttl",
        ".triage/c/findings/RSH-001.ttl",
        ".triage/c/findings/QA2-001.ttl",
        ".triage/c/findings/FTQ-009.ttl",
        ".triage/c/findings/FTQ-010.ttl"
      ]
    },
    "parent_correlation_required": true,
    "aggregation_source": "reviewer_fenced_json_union_todo_scan",
    "carry_forward": {
      "RBQA-F001": {
        "owner": "rust-best-practices-agent",
        "qa_pass": "qa-4",
        "rechecked": false,
        "artifact": "/tmp/phase-c-execution/c-1/qa-4/rust-best-practices-agent-result.json"
      }
    },
    "todo_scan": {
      "rows": 0,
      "artifact": "/tmp/phase-c-execution/c-1/qa-7/manager-todo-scan.txt"
    },
    "ci": {
      "success": 14,
      "failure": 1,
      "pending": 0,
      "artifact": "/tmp/phase-c-execution/c-1/qa-7/manager-ci.json"
    }
  },
  "aggregation_source": "reviewer_fenced_json_union_todo_scan",
  "finding_ids": [
    "REQ-QA-004",
    "ARCH-001",
    "RBQA-F001",
    "QA2-001"
  ]
}
```

## Findings Summary
- Deliverables: 29/34 (85.29%)
- Blocking: 3
- Important: 1
- Minor: 0

## Blocking Findings
- **REQ-QA-004 (Blocking)** — `docs/plans/phase-c/sprint-c-1-runtime-migration.md:365`: REQ-QA-004 remains open at exact 5255610a16ef4500f0af508e29a533707a4a3e0a. Actual Linux Test/both Clippy and full job now SUCCESS, resolving the observed Linux crash cause on that hosted run. Complete supplied snapshot has12SUCCESS/1boundaryFAILURE/2workspace-platformIN_PROGRESS. Actual dependency composition and strict isolated graph remain unchanged and unauthorized direct sync -> sc-command edge persists. Full plan assessment remains29/34 with five open rows; no all-platform/anchor/integration closeout. Correction: Keep REQ-QA-004 open until actual authorized consumer composition satisfies the unchanged strict dependency graph and all required exact-head ci/python checks are green, with independent QA and applicable development anchors complete before integration. Retain ARCH-001, RBQA-F001 and QA2-001 independently. Preserve RSH-001 actual owner/enforcer closure for unchanged production, published 0.1.1, absolute 2*T and expiry-at-T behavior; no deadline widening, missing-event substitution, baseline relaxation or waiver.
- **ARCH-001 (Blocking)** — `crates/bd-cache-sync/Cargo.toml:31`: OPEN/BLOCKED at exact 5255610a. Direct bd-cache-sync -> sc-command remains at Cargo.toml:31 and cron_log.rs:3. ADR-BDC-0016 decision 5 permits only the contract/logging dependency carried by published upstream sc-cron below isolated Python. Independent 185-file byte comparison to dc468b9c proves production source, manifests, lockfile, architecture, CI and strict graph guards unchanged. Target merge84edfb28 consumes a01386a and changes only orchestration metadata. Bounded inventory/session cleanup and Linux identity correction are test-only and neither remove nor authorize this edge. No binding ADR amendment widens the exception. Correction: Keep ARCH-001 open and do not merge this candidate. Obtain an authorized consumable upstream public receiver/type re-export or equivalent callback constructor delivering genuine canonical scheduler events, then remove the direct sync -> sc-command edge and verify the existing strict isolated consumer graph gate. Baseline or exception changes require separate authorization and an Approved/Active ADR amendment; no logging loss, relocated forbidden introducer, gate relaxation, or waiver is accepted.
- **RBQA-F001 (Blocking)** — `crates/bd-cache-sync/Cargo.toml:31`: The sync crate adds a direct sc-command contract/logging dependency beyond ADR-BDC-0016's approved upstream sc-cron exception. Correction: Remove the direct sync-to-sc-command edge and preserve scheduler logging through an approved upstream/public port. If the published API cannot support the bridge within current boundaries, resolve that upstream or obtain a separately authorized narrow ADR before changing dependency policy. Do not treat the new Crate Layout row as approval.

## Detailed Findings
- **REQ-QA-004 (Blocking)** — `docs/plans/phase-c/sprint-c-1-runtime-migration.md:365`: REQ-QA-004 remains open at exact 5255610a16ef4500f0af508e29a533707a4a3e0a. Actual Linux Test/both Clippy and full job now SUCCESS, resolving the observed Linux crash cause on that hosted run. Complete supplied snapshot has12SUCCESS/1boundaryFAILURE/2workspace-platformIN_PROGRESS. Actual dependency composition and strict isolated graph remain unchanged and unauthorized direct sync -> sc-command edge persists. Full plan assessment remains29/34 with five open rows; no all-platform/anchor/integration closeout. Correction: Keep REQ-QA-004 open until actual authorized consumer composition satisfies the unchanged strict dependency graph and all required exact-head ci/python checks are green, with independent QA and applicable development anchors complete before integration. Retain ARCH-001, RBQA-F001 and QA2-001 independently. Preserve RSH-001 actual owner/enforcer closure for unchanged production, published 0.1.1, absolute 2*T and expiry-at-T behavior; no deadline widening, missing-event substitution, baseline relaxation or waiver.
- **ARCH-001 (Blocking)** — `crates/bd-cache-sync/Cargo.toml:31`: OPEN/BLOCKED at exact 5255610a. Direct bd-cache-sync -> sc-command remains at Cargo.toml:31 and cron_log.rs:3. ADR-BDC-0016 decision 5 permits only the contract/logging dependency carried by published upstream sc-cron below isolated Python. Independent 185-file byte comparison to dc468b9c proves production source, manifests, lockfile, architecture, CI and strict graph guards unchanged. Target merge84edfb28 consumes a01386a and changes only orchestration metadata. Bounded inventory/session cleanup and Linux identity correction are test-only and neither remove nor authorize this edge. No binding ADR amendment widens the exception. Correction: Keep ARCH-001 open and do not merge this candidate. Obtain an authorized consumable upstream public receiver/type re-export or equivalent callback constructor delivering genuine canonical scheduler events, then remove the direct sync -> sc-command edge and verify the existing strict isolated consumer graph gate. Baseline or exception changes require separate authorization and an Approved/Active ADR amendment; no logging loss, relocated forbidden introducer, gate relaxation, or waiver is accepted.
- **QA2-001 (Important)** — `crates/bd-cache-sync/Cargo.toml:31`: The actual required isolated Python dependency graph still fails after correction: bd-cache-sync directly introduces sc-command outside the approved published sc-cron introducer. This is the real failing gate associated with open ARCH-001/RBQA-F001, not an execution wrapper failure. Correction: Keep the gate failing until the existing ARCH-001/RBQA-F001 remediation is satisfied through an authorized upstream public logging seam and the direct edge is removed; rerun the actual isolated consumer gate and required hosted qualification. Do not suppress or broaden the exception.
- **RBQA-F001 (Blocking)** — `crates/bd-cache-sync/Cargo.toml:31`: The sync crate adds a direct sc-command contract/logging dependency beyond ADR-BDC-0016's approved upstream sc-cron exception. Correction: Remove the direct sync-to-sc-command edge and preserve scheduler logging through an approved upstream/public port. If the published API cannot support the bridge within current boundaries, resolve that upstream or obtain a separately authorized narrow ADR before changing dependency policy. Do not treat the new Crate Layout row as approval.

RBQA-F001 is the actual QA4 owner finding carried unrechecked; no new specialist review or closure is claimed.
Current hosted CI: 14 SUCCESS, 1 FAILURE, 0 pending at exact reviewed head. Native Linux/macOS/Windows jobs and all Python jobs pass. The fmt/doc/publish/boundaries job fails; required CI remains unfulfilled. This updates the older req-qa snapshot without changing its verdict or completion percentage.
TODO scan: zero rows, no TODO findings. Accepted ceremony decisions: REQ-QA-004, ARCH-001, QA2-001, RBQA-F001 all not_applicable; no waiver or publisher work.
Reviewer evidence only: Rust QA independently ran 11 lifecycle and 5 store tests plus affected Clippy/fmt, validating current Cargo fingerprints and binary hashes with up-to-date artifacts reused. Owning flaky reviewer replayed both original counterexamples and complete PID/PGID/SID/inventory audits; survivors were empty before fallback. Coordinator ran no Cargo, source review, or agents.
Available inherited model used; unavailable named model configurations are not claimed.

## Resolved Since Last Pass
- **FTQ-010 fixed**: owning flaky reviewer passes both actual QA5 preregistration and QA6 stopped-inventory counterexamples, timeout/unwind probes and complete survivor audits before fallback; Rust independently passes 11 lifecycle tests.
- **FTQ-009 closed preserved**: changed shared helper has explicit default cleanup_on_drop=true and SessionFixture ownership handoff; independent five current store tests prove normal paths, actual stalled joins and unwind. Whole-helper byte equivalence is not claimed.
- **RSH-001 closed preserved**: independent 185-path production equivalence preserves accepted service closure, including consumed target merge. No fresh whole-runtime certification claimed.

## Merge Readiness
- Status: **not ready**
- Reason: Four open findings (3 Blocking, 1 Important), 29/34 deliverables, and failing boundary CI; whole sprint/integration/anchor/merge/release acceptance remains blocked.

## Next Action
- Action: triage_and_fix
- Owner: parent-orchestrator

## Reviewer Spawn Gate
- Gate: **pass**
- Reviewer manifest, missing/unparsed reviewers, and evidence chain are in the Machine Status JSON above.
