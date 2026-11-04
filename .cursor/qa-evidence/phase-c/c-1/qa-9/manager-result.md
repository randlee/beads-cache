# QA Findings Update

Generated: 2026-11-04T05:23:16.811002+00:00
QA Pass: qa-9
Sprint/Task: c-1 / qa-c-1-9
Branch: `sprint/c-1-runtime-migration`
Commit: `a753d2799183dd4e341c854c4515bf046447eb6c`
PR: #108
Verdict: **FAIL**

## Machine Status (JSON)
```json
{
  "sprint": "c-1",
  "task": "qa-c-1-9",
  "branch": "sprint/c-1-runtime-migration",
  "commit": "a753d2799183dd4e341c854c4515bf046447eb6c",
  "pr": 108,
  "verdict": "FAIL",
  "deliverables": {
    "complete": 29,
    "total": 34,
    "percent": 85.29
  },
  "findings": {
    "blocking": 3,
    "important": 2,
    "minor": 0
  },
  "blocking_ids": [
    "ARCH-001",
    "REQ-QA-004",
    "RBQA-F001"
  ],
  "merge_readiness": "not ready",
  "merge_reason": "5 open findings (3 Blocking, 2 Important, 0 Minor), incomplete deliverables and failing required boundary CI.",
  "next_action": "triage_and_fix",
  "owner": "parent-orchestrator",
  "reviewer_spawn_gate": "pass",
  "reviewer_manifest": [
    {
      "agent": "rust-qa-agent",
      "task_id": "/root/c1_rustqa9",
      "spawn_actor": "parent-orchestrator",
      "model": "gpt-6.1-sol",
      "reasoning_effort": "high",
      "model_provenance": "Available fallback; configured Grok/Composer unavailable",
      "fenced_json_received": true,
      "verdict": "findings",
      "findings": {
        "blocking": 0,
        "important": 2,
        "minor": 0
      }
    },
    {
      "agent": "arch-qa",
      "task_id": "/root/c1_archqa9",
      "spawn_actor": "parent-orchestrator",
      "model": "gpt-6.1-sol",
      "reasoning_effort": "high",
      "model_provenance": "Available fallback; configured Grok/Composer unavailable",
      "fenced_json_received": true,
      "verdict": "FAIL",
      "findings": {
        "blocking": 1,
        "important": 0,
        "minor": 0
      }
    },
    {
      "agent": "req-qa",
      "task_id": "/root/c1_reqqa9",
      "spawn_actor": "parent-orchestrator",
      "model": "gpt-6.1-sol",
      "reasoning_effort": "high",
      "model_provenance": "Available fallback; configured Grok/Composer unavailable",
      "fenced_json_received": true,
      "verdict": "FAIL",
      "findings": {
        "blocking": 1,
        "important": 0,
        "minor": 0
      }
    },
    {
      "agent": "flaky-test-qa",
      "task_id": "/root/c1_flakyqa9",
      "spawn_actor": "parent-orchestrator",
      "model": "gpt-6.1-sol",
      "reasoning_effort": "high",
      "model_provenance": "Available fallback; configured Composer unavailable",
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
    "qa_pass": "qa-9",
    "commit": "a753d2799183dd4e341c854c4515bf046447eb6c",
    "pr_number": 108,
    "pr_comment_url": "https://github.com/randlee/beads-cache/pull/108#issuecomment-6623311092",
    "coordinator_task_id": "/root/c1_quality_manager9",
    "parent_spawn_correlation": "/tmp/phase-c-execution/c-1/qa-9/parent-spawn-correlation.json",
    "triage": {
      "ttl_paths": [
        ".triage/c/findings/REQ-QA-004.ttl",
        ".triage/c/findings/ARCH-001.ttl",
        ".triage/c/findings/RBQA-F001.ttl",
        ".triage/c/findings/QA2-001.ttl",
        ".triage/c/findings/RSH-001.ttl",
        ".triage/c/findings/FTQ-011.ttl",
        ".triage/c/findings/FTQ-009.ttl",
        ".triage/c/findings/FTQ-010.ttl",
        ".triage/c/findings/RQA9-001.ttl"
      ],
      "new_record_worktree": "/Users/randlee/github/beads-cache-worktrees/integrate/phase-c",
      "frozen_review_worktree_unchanged": true
    },
    "carry_forward_ids": [
      "RBQA-F001"
    ],
    "carry_owner_evidence": "/tmp/phase-c-execution/c-1/qa-4/rust-best-practices-agent-result.json",
    "todo_scan_rows": 0,
    "ci_snapshot": "/tmp/phase-c-execution/c-1/qa-9/manager-pr108-ci.json",
    "ci_checks": "/tmp/phase-c-execution/c-1/qa-9/manager-pr108-checks.json",
    "ci_success": 14,
    "ci_failure": 1,
    "ci_pending": 0,
    "scope": "C1 QA9; paired C2 startup occurrence only; no whole C2, stock HTTP, anchor, sprint or merge acceptance"
  },
  "aggregation_source": "reviewer_fenced_json_union_todo_scan",
  "open_finding_ids": [
    "REQ-QA-004",
    "ARCH-001",
    "RBQA-F001",
    "QA2-001",
    "RQA9-001"
  ]
}
```

## Findings Summary
- Deliverables: 29/34 (85.29%)
- Blocking: 3
- Important: 2
- Minor: 0

## Blocking Findings
- **ARCH-001** — OPEN at exact a753d279. bd-cache-sync directly depends on sc-command and imports its CanonicalLog contract in cron_log.rs:3. Approved ADR-BDC-0016 decision 5 permits only the published upstream sc-cron introducer below isolated Python. The helper-only correction leaves that edge, logger, lockfile and strict introducer-aware consumer guard unchanged; no Approved/Active amendment authorizes it.
- **REQ-QA-004** — At exact C1 a753d2799183dd4e341c854c4515bf046447eb6c, unauthorized direct sync->sc-command composition and the required isolated-consumer graph/hosted CI gate remain unresolved. All101 requirements and34 deliverables assessed;29/34 complete, five open: governed logging/graph composition, dependency/CLI/wheel acceptance, isolated consumer acceptance, full current hosted CI and independent QA/anchor/integration closeout. Current focused changed-helper/lifecycle validation earns completion credit; retained old14/15 and13/15 failures are not erased. Startup producer/public-caller verification is separately credited under current owner receipts and does not substitute for whole sprint acceptance. Current source-equivalent store fixture also has actual C2 Windows readiness failure33/34 at schedule.rs1084; exact cause unknown, routed as platform acceptance evidence, not mislabeled as signal failure. C1 core Linux/macOS/Windows current jobs are independently SUCCESS; this credit is retained and is not represented as overall CI green. The distinct Windows test observation is canonical RQA9-001, still open with its owner.
- **RBQA-F001** — The sync crate adds a direct sc-command contract/logging dependency beyond ADR-BDC-0016's approved upstream sc-cron exception.

## Detailed Findings
### REQ-QA-004 — Blocking

At exact C1 a753d2799183dd4e341c854c4515bf046447eb6c, unauthorized direct sync->sc-command composition and the required isolated-consumer graph/hosted CI gate remain unresolved. All101 requirements and34 deliverables assessed;29/34 complete, five open: governed logging/graph composition, dependency/CLI/wheel acceptance, isolated consumer acceptance, full current hosted CI and independent QA/anchor/integration closeout. Current focused changed-helper/lifecycle validation earns completion credit; retained old14/15 and13/15 failures are not erased. Startup producer/public-caller verification is separately credited under current owner receipts and does not substitute for whole sprint acceptance. Current source-equivalent store fixture also has actual C2 Windows readiness failure33/34 at schedule.rs1084; exact cause unknown, routed as platform acceptance evidence, not mislabeled as signal failure. C1 core Linux/macOS/Windows current jobs are independently SUCCESS; this credit is retained and is not represented as overall CI green. The distinct Windows test observation is canonical RQA9-001, still open with its owner.

Required correction: Resolve the genuine approved public receiver composition within the existing authorized contract, preserving canonical scheduler events, then remove the forbidden direct edge and pass the unchanged actual isolated graph and all required exact-head hosted ci/python gates. Complete remaining independent QA/development anchors and integration closeout. Keep distinct canonical IDs and prior failure history; do not widen deadlines, suppress guards, use SDK/publisher/auth workarounds or infer whole C2/stock-daemon HTTP acceptance from focused caller proof. Investigate and correct the actual Windows store-fixture readiness failure, preserving original causal/cleanup budgets, then obtain required platform qualification; do not guess its cause or retry/relax it to green.

Evidence/source: req-qa; crates/bd-cache-sync/Cargo.toml:31, crates/bd-cache-sync/src/cron_log.rs:3, .github/workflows/ci.yml:126, /tmp/phase-c-execution/c-1/qa-9/rust-qa-agent-result.json, /tmp/phase-c-execution/c-1/qa-9/rust-qa-pr108-ci.json, /tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log, crates/bd-cache-store/src/schedule.rs:1084

### ARCH-001 — Blocking

OPEN at exact a753d279. bd-cache-sync directly depends on sc-command and imports its CanonicalLog contract in cron_log.rs:3. Approved ADR-BDC-0016 decision 5 permits only the published upstream sc-cron introducer below isolated Python. The helper-only correction leaves that edge, logger, lockfile and strict introducer-aware consumer guard unchanged; no Approved/Active amendment authorizes it.

Required correction: Keep ARCH-001 open. Obtain the authorized consumable upstream public receiver/type re-export or equivalent genuine canonical-event callback constructor, remove the direct sync -> sc-command edge, and pass the unchanged isolated-consumer gate. Preserve logging and the narrow dependency policy. No publisher/auth/token work, version change, relocated forbidden introducer, fabricated event, gate relaxation or waiver is authorized by this review.

Evidence/source: arch-qa; crates/bd-cache-sync/Cargo.toml:31

### RBQA-F001 — Blocking

The sync crate adds a direct sc-command contract/logging dependency beyond ADR-BDC-0016's approved upstream sc-cron exception.

Required correction: Remove the direct sync-to-sc-command edge and preserve scheduler logging through an approved upstream/public port. If the published API cannot support the bridge within current boundaries, resolve that upstream or obtain a separately authorized narrow ADR before changing dependency policy. Do not treat the new Crate Layout row as approval.

Evidence/source: QA4 actual rust-best-practices owner; unchanged historical carry, no fresh boundary review; crates/bd-cache-sync/Cargo.toml:31

### QA2-001 — Important

The required exact-head isolated Python graph still rejects the direct bd-cache-sync -> sc-command introducer outside the approved published sc-cron exception.

Required correction: Preserve the failing guard; resolve the already-owned ARCH-001/RBQA-F001 public logging seam and remove the direct edge, then qualify the actual isolated consumer and required CI. No exception expansion or suppression.

Evidence/source: rust-qa-agent; crates/bd-cache-sync/Cargo.toml:31

### RQA9-001 — Important

Exact frozen C2 Windows required CI fails the shared store supervisor negative: the busy-persistence child is killed/reaped at its five-second watchdog before it emits the required real-operation/stalled-join observation, so this required test fails and that run does not establish the busy-write negative path.

Required correction: Have the owning flaky-test reviewer inspect this concrete occurrence and preserve the raw log. Investigate causal operation/stall acknowledgements and Windows execution before correcting the fixture; retain real negative-path coverage and existing product500ms/five-second ownership budgets. Do not infer a scheduling cause, widen deadlines or rerun to waive the failure.

Evidence/source: rust-qa-agent, flaky-test-qa; crates/bd-cache-store/src/schedule.rs:1084

### Current CI and enforcement scope

Exact a753d279: 14 SUCCESS, 1 FAILURE, no pending checks. Core Linux/macOS/Windows jobs succeeded. The required `fmt, doc, publish dry-run, boundaries` job126399209278 fails the strict isolated Python dependency graph; reviewer JSON records API graph/checker4 pass separately. Actual C2 Windows job126397662727 is a distinct shared-store 33-pass/1-fail occurrence retained as RQA9-001; underlying cause unknown. The absence of its receipt does not prove the operation never began or returned. Preserve original 30ms cancellation, 500ms observation and five-second ownership budgets.

All four mandatory role tasks were actually spawned by parent and completed; current XML fences equal saved JSON, exact commit pins match, and wrapper execution succeeded. Model gpt-6.1-sol/high is the recorded available fallback, without claiming configured Composer/Grok execution. TODO scan: zero rows. All five ceremony-screen not_applicable proposals were accepted by parent; no waivers. RBQA-F001 remains distinct unchanged historical carry from the actual QA4 owner; no fresh boundary review claimed. Enforcer performed no source review or Cargo/test execution. New RQA9-001 triage is in integration only; the reviewed C1 checkout remains frozen.

## Resolved Since Last Pass
- **RSH-001 — RESOLVED VERIFIED** for the reopened supported startup occurrence at exact C1 a753d279 and C2 b7de8e9, aggregated from the current actual Rust owner. C1 startup cases, independent rebuilt public Application probe250.876416ms within original500ms, C2 service7/7, truthful readiness/callback/management and actual joins support closure. No missing startup evidence remains; no whole-C2 or stock-HTTP acceptance.
- **FTQ-011 — CLOSED for exact C1** from current owning flaky-test-qa disposition and independent Rust evidence: removed separate50ms requirement, original absolute cleanup/lifetime budgets, complete census/root join/final emptiness and false failure receipt retained; lifecycle18/18 and four finite failed negative probes credited. Frozen C2 still needs corrected helper consumption and qualification.
- **FTQ-009 / FTQ-010 — prior scoped closures preserved.** New Windows RQA9-001 remains independent. Original developer14/15 and Rust13/15 failed logs remain retained and hashed; no rerun erases them.

## Merge Readiness
- Status: **not ready**
- Reason: 5 open findings (3 Blocking, 2 Important, 0 Minor), incomplete deliverables and failing required boundary CI.

## Next Action
- Action: triage_and_fix
- Owner: parent-orchestrator

## Reviewer Spawn Gate
- Gate: **pass**
- Reviewer manifest, missing/unparsed reviewers, and evidence chain are in the Machine Status JSON above.