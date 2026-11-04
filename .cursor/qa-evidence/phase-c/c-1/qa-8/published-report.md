# QA Findings Update

Generated: 2026-11-04T04:41:22.772757+00:00
QA Pass: qa-8
Sprint/Task: c-1 / qa-c-1-8
Branch: `sprint/c-1-runtime-migration`
Commit: `adbcc88d3dc134ae94403bc2af7a8db76ba921a5`
PR: #108
Verdict: **FAIL**

## Machine Status (JSON)
```json
{
  "sprint": "c-1",
  "task": "qa-c-1-8",
  "branch": "sprint/c-1-runtime-migration",
  "commit": "adbcc88d3dc134ae94403bc2af7a8db76ba921a5",
  "pr": 108,
  "verdict": "FAIL",
  "deliverables": {
    "complete": 28,
    "total": 34,
    "percent": 82.35
  },
  "findings": {
    "blocking": 4,
    "important": 2,
    "minor": 0
  },
  "blocking_ids": [
    "REQ-QA-004",
    "ARCH-001",
    "RBQA-F001",
    "RSH-001"
  ],
  "merge_readiness": "not ready",
  "merge_reason": "Six open findings (4 Blocking, 2 Important), incomplete deliverables, reproduced lifecycle failure and failing required boundary CI.",
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
      "verdict": "findings",
      "findings": {
        "blocking": 0,
        "important": 1,
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
        "important": 2,
        "minor": 0
      }
    }
  ],
  "missing_reviewers": [],
  "unparsed_reviewers": [],
  "evidence_chain": {
    "qa_pass": "qa-8",
    "commit": "adbcc88d3dc134ae94403bc2af7a8db76ba921a5",
    "pr_comment_url": "https://github.com/randlee/beads-cache/pull/108#issuecomment-6622741614",
    "coordinator_task_id": "/root/c1_quality_manager8",
    "triage": {
      "ttl_paths": [
        ".triage/c/findings/REQ-QA-004.ttl",
        ".triage/c/findings/ARCH-001.ttl",
        ".triage/c/findings/RBQA-F001.ttl",
        ".triage/c/findings/QA2-001.ttl",
        ".triage/c/findings/RSH-001.ttl",
        ".triage/c/findings/FTQ-011.ttl",
        ".triage/c/findings/FTQ-009.ttl",
        ".triage/c/findings/FTQ-010.ttl"
      ]
    },
    "aggregation_source": "reviewer_fenced_json_union_todo_scan",
    "carry_forward_ids": [
      "RBQA-F001",
      "RSH-001"
    ],
    "todo_scan_rows": 0,
    "ci_snapshot": "/tmp/phase-c-execution/c-1/qa-8/manager-ci-checks.json",
    "ci_success": 14,
    "ci_failure": 1,
    "scope": "C1 QA8 only; no whole sprint/caller/anchor/merge/release acceptance"
  },
  "aggregation_source": "reviewer_fenced_json_union_todo_scan",
  "open_finding_ids": [
    "REQ-QA-004",
    "ARCH-001",
    "RBQA-F001",
    "RSH-001",
    "QA2-001",
    "FTQ-011"
  ]
}
```

## Findings Summary
- Deliverables: 28/34 (82.35%)
- Blocking: 4
- Important: 2
- Minor: 0

## Blocking Findings
- REQ-QA-004 — Blocking; see detailed finding.
- ARCH-001 — Blocking; see detailed finding.
- RBQA-F001 — Blocking; see detailed finding.
- RSH-001 — Blocking; see detailed finding.

## Detailed Findings
### REQ-QA-004 — Blocking

Location: `docs/plans/phase-c/sprint-c-1-runtime-migration.md:365`. REQ-QA-004 remains open at exact adbcc88d3dc134ae94403bc2af7a8db76ba921a5. Current supplied CI is6SUCCESS/1boundaryFAILURE/6IN_PROGRESS/2QUEUED; unchanged unauthorized direct sync -> sc-command composition persists. Required affected lifecycle validation additionally has actual14/15 broad failure from owned inventory timeout; focused recheck does not erase it. Startup correction is implemented with C1 source/developer proofs, but owning independent acceptance, C2 candidate consumption/probe and integration gates remain open. Full current checklist is28/34 with six incomplete/open rows.

Required action: Resolve actual authorized consumer composition and obtain all required exact-head ci/python gates green; independently investigate the real owned ps timeout and establish reliable affected-suite validation preserving existing budgets/cleanup without suppression or blanket retry. Complete independent RSH001 startup and QA/anchor gates. C2 must consume qualified candidate and rerun its own exact public probe before caller acceptance. Preserve prior post-start/FTQ009 credit, no baseline waiver, deadline widening, SDK/publisher workaround or false whole-sprint PASS.

### ARCH-001 — Blocking

Location: `crates/bd-cache-sync/Cargo.toml:31`. OPEN/BLOCKED at exact adbcc88d. Direct bd-cache-sync -> sc-command remains at Cargo.toml:31 and cron_log.rs:3. ADR-BDC-0016 decision5 permits only the contract/logging dependency introduced by published upstream sc-cron below isolated Python. All manifests, lockfile, canonical receiver source, governing architecture and strict graph guards remain byte-identical to5255610a. The cron startup read/lifetime correction changes neither this edge nor its authorization. No binding amendment widens the exception.

Required action: Keep ARCH-001 open and do not merge this candidate. Obtain an authorized consumable upstream public receiver/type re-export or equivalent callback constructor delivering genuine canonical scheduler events, then remove the direct sync -> sc-command edge and verify the existing strict isolated consumer graph gate. Baseline or exception changes require separate authorization and an Approved/Active ADR amendment; no logging loss, relocated forbidden introducer, gate relaxation, or waiver is accepted.

### RBQA-F001 — Blocking

Location: `crates/bd-cache-sync/Cargo.toml:31`. The sync crate adds a direct sc-command contract/logging dependency beyond ADR-BDC-0016's approved upstream sc-cron exception.

Required action: Remove the direct sync-to-sc-command edge and preserve scheduler logging through an approved upstream/public port. If the published API cannot support the bridge within current boundaries, resolve that upstream or obtain a separately authorized narrow ADR before changing dependency policy. Do not treat the new Crate Layout row as approval.

### RSH-001 — Blocking

Location: `crates/bd-cache-sync/src/cron.rs:159`. Canonical startup occurrence remains OPEN per current arch finding_dispositions, runtime dispositions and current triage. Core startup implementation is independently verified; actual C2 Application still consumes the older core, so candidate public caller qualification and owning adjudication remain pending. Prior post-start closure stays credited.

Required action: Consume this exact corrected candidate in C2 and replay the actual public Application startup contention counterexample, preserving original T/2T, retained joins and ordinary independent management policy; obtain owning adjudication. Old C2 probe is historical evidence, never candidate or stock-daemon HTTP evidence.

### QA2-001 — Important

Location: `crates/bd-cache-sync/Cargo.toml:31`. The actual required isolated Python dependency graph still fails after correction: bd-cache-sync directly introduces sc-command outside the approved published sc-cron introducer. This is the real failing gate associated with open ARCH-001/RBQA-F001, not an execution wrapper failure.

Required action: Keep the gate failing until the existing ARCH-001/RBQA-F001 remediation is satisfied through an authorized upstream public logging seam and the direct edge is removed; rerun the actual isolated consumer gate and required hosted qualification. Do not suppress or broaden the exception.

### FTQ-011 — Important

Location: `crates/bd-cache-sync/tests/common/session_fixture.rs:227`. The fixture makes successful teardown depend on a separately scheduled host-wide /bin/ps child exiting within 50 ms, independently of the enclosing 250 ms/one-second cleanup and five-second fixture budgets. Ordinary user-space scheduling or host inventory cost can therefore turn a correct lifecycle into an inventory TimedOut failure. The retained exact-head post-push suite actually failed 14/15 with that error in the stalled-join family, while the later focused family run passed. This is a concrete intermittent helper failure; unchanged helper bytes and rerunning to green do not cure it.

Required action: Redesign inventory acquisition so test success does not depend on an individually scheduled host-wide command meeting an ambient 50 ms speed condition. Prefer a deterministic kernel/session inventory interface scoped to owned processes, retaining one hard absolute cleanup lifetime and explicit ownership/reap of any subprocess. Keep the stopped-inventory failure proof separate from healthy lifecycle assertions. Do not mask the observed failure with retries, sleep padding, suppressed timeout errors or widened lifecycle budgets.

### Evidence scope and validation credit

Four actual independent reviewer turns are recorded in the manifest at frozen adbcc88d3dc134ae94403bc2af7a8db76ba921a5. Required fix-round set is req-qa, arch-qa, rust-qa-agent and conditional flaky-test-qa. Rust best-practices is not redispatched; RBQA-F001 is explicitly carried from actual QA4 owner JSON, with unchanged composition reported by current reviewers. RSH-001 unresolved carry is counted even though absent from current findings arrays. Duplicate FTQ-011 reports are counted once.

Independent Rust evidence credits all four startup cases, store 5/5, management isolation 1/1, canonical logging 1/1, formatting and Clippy. Startup keeps the real owned read/join and original absolute T/2T; ordinary retained management policy and same-instance reuse are preserved. These positives do not close the current public Application occurrence until actual C2 consumption and public reprobe. The first nonexistent cron_log test invocation failed before testing; corrected cron_logging passes, both logs retained.

FTQ-011 is the actual independent broad lifecycle 13/15 failure, corroborating retained developer 14/15 failure. Focused green retry does not erase either outcome. The arbitrary 50 ms whole-host ps speed dependency is distinct from repaired FTQ-009 store/helper lifetimes and FTQ-010 pre-registration ownership and bounded inventory child kill/reap. Precise slow invocation phase/scheduler cause is uninstrumented and is not inferred.

Following cass rule b-mr7cp6x0-ipdnhs, current hosted core Linux/macOS/Windows jobs were checked individually and all succeed. Fresh coordinator snapshot: 14 SUCCESS, one FAILURE, no pending; failing composite fmt/doc/publish/boundaries check retains the isolated Python boundary failure. PR head matches the frozen candidate and is OPEN/UNSTABLE. Earlier pending reviewer snapshots are historical observations, superseded only for CI status by this fresh snapshot; deliverable completion remains the owning req-qa result 28/34 (82.35%).

TODO triage: all Rust files scanned using the prescribed finder in the assigned worktree; zero rows, no tags or unresolved TODO findings. No coordinator Cargo, reviewer source analysis, edits, or agent spawns. Inherited available Codex model used honestly; configured alternate models were not claimed.

All six ceremony decisions are accepted as not_applicable per screen-parent-adjudication.json. No waiver, dependency baseline change, publisher work, merge, whole-sprint acceptance, anchor acceptance or release acceptance is granted. Parent routes deterministic inventory correction, actual C2 candidate consumption/public reprobe and existing strict graph remediation; retain budgets and actual owned completion, no timeout padding, failed-census-as-empty or detached process.

Input caveat: template XML contains unescaped reviewer text that prevents strict whole-document XML parsing. Template fields and handoff manifest/four JSON fences extract unambiguously; each JSON parses and matches the frozen head. This presentation defect creates no missing reviewer or wrapper execution failure.


## Resolved Since Last Pass
- FTQ-009 remains closed: actual current independent store 5/5 and prior owning closure.
- FTQ-010 remains closed for original pre-registration/orphan and unbounded inventory mechanisms; FTQ-011 is distinct and open.
- RSH-001 prior post-start repair and current core startup implementation credited; full canonical startup closure remains open pending actual caller qualification.

## Merge Readiness
- Status: **not ready**
- Reason: Six open findings (4 Blocking, 2 Important), incomplete deliverables, reproduced lifecycle failure and failing required boundary CI.

## Next Action
- Action: triage_and_fix
- Owner: parent-orchestrator

## Reviewer Spawn Gate
- Gate: **pass**
- Reviewer manifest, missing/unparsed reviewers, and evidence chain are in the Machine Status JSON above.
