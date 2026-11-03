# QA Findings Update

Generated: 2026-11-03T00:03:04.315549+00:00
QA Pass: qa-1
Sprint/Task: phase-c / phase-c-qa1
Branch: `plan/phase-c`
Commit: `0beef1a8e213b9b55af85d55ca014e85fde52782`
PR: #105
Verdict: **FAIL**

## Machine Status (JSON)
```json
{
  "sprint": "phase-c",
  "task": "phase-c-qa1",
  "branch": "plan/phase-c",
  "commit": "0beef1a8e213b9b55af85d55ca014e85fde52782",
  "pr": 105,
  "verdict": "FAIL",
  "deliverables": {
    "complete": 14,
    "total": 15,
    "percent": 93.33
  },
  "findings": {
    "blocking": 7,
    "important": 2,
    "minor": 1
  },
  "blocking_ids": ["REQ-QA-001", "REQ-QA-002", "REQ-QA-003", "REQ-QA-004", "ARCH-001", "ARCH-002", "ARCH-003", "RBP-F001", "RSH-001", "CQA-F001"],
  "merge_readiness": "not ready",
  "merge_reason": "Ten open findings (all severities); plan deliverables14/15. CI separately in progress. No implementation approval inferred.",
  "next_action": "triage_and_fix",
  "owner": "parent-orchestrator / plan author",
  "reviewer_spawn_gate": "pass",
  "reviewer_manifest": [{"agent": "req-qa", "task_id": "/root/qa_req", "spawn_actor": "parent-orchestrator", "fenced_json_received": true, "verdict": "FAIL", "findings": {"blocking": 4, "important": 0, "minor": 0}}, {"agent": "arch-qa", "task_id": "/root/qa_arch", "spawn_actor": "parent-orchestrator", "fenced_json_received": true, "verdict": "FAIL", "findings": {"blocking": 3, "important": 0, "minor": 0}}, {"agent": "ruthless-boundary-qa", "task_id": "/root/qa_boundary", "spawn_actor": "parent-orchestrator", "fenced_json_received": true, "verdict": "pass", "findings": {"blocking": 0, "important": 0, "minor": 0}}, {"agent": "rust-best-practices-agent", "task_id": "/root/qa_practices", "spawn_actor": "parent-orchestrator", "fenced_json_received": true, "verdict": "findings", "findings": {"blocking": 0, "important": 1, "minor": 0}}, {"agent": "rust-service-hardening-agent", "task_id": "/root/qa_service", "spawn_actor": "parent-orchestrator", "fenced_json_received": true, "verdict": "findings", "findings": {"blocking": 0, "important": 1, "minor": 0}}, {"agent": "ceremony-qa", "task_id": "/root/qa_ceremony", "spawn_actor": "parent-orchestrator", "fenced_json_received": true, "verdict": "findings", "findings": {"blocking": 0, "important": 0, "minor": 1}}],
  "missing_reviewers": [],
  "unparsed_reviewers": [],
  "evidence_chain": {"qa_pass": "qa-1", "commit": "0beef1a8e213b9b55af85d55ca014e85fde52782", "pr_comment_url": "https://github.com/randlee/beads-cache/pull/105#issuecomment-6591684348", "coordinator_task_id": "/root/quality_manager", "reviewer_tasks": ["/root/qa_req", "/root/qa_arch", "/root/qa_boundary", "/root/qa_practices", "/root/qa_service", "/root/qa_ceremony"], "triage": {"ttl_paths": []}, "ceremony_screen": "docs/plans/phase-c/reviews/qa1-screen.md", "ci": "14 SUCCESS; one Windows Rust leg IN_PROGRESS at observation; no CI PASS claimed", "todo_scan": "not applicable to doc_review; no implementation sprint-end/integration review", "aggregation_source": "reviewer_fenced_json_union_todo_scan"}
}
```

## Findings Summary
- Deliverables: 14/15 (93.33%)
- Blocking: 7
- Important: 2
- Minor: 1

## Blocking Findings
- REQ-QA-001 (blocking): c-1 owns every path in the store, sync, CLI, Python and local scheduler crates, but its authoritative requirements list omits governing approved crate requirements. Missing IDs by crate: {"bd-cache-cli": ["REQ-CLI-0002", "REQ-CLI-0004", "REQ-CLI-0005", "REQ-CLI-0006", "REQ-CLI-0008", "REQ-CLI-0009", "REQ-CLI-0010", "REQ-CLI-0011", "REQ-CLI-0012"], "bd-cache-py": ["REQ-PY-0001", "REQ-PY-0003", "REQ-PY-0004", "REQ-PY-0005", "REQ-PY-0007", "REQ-PY-0008", "REQ-PY-0010", "REQ-PY-0011", "REQ-PY-0012", "REQ-PY-0013", "REQ-PY-0014"], "bd-cache-store": ["REQ-STORE-0001", "REQ-STORE-0004", "REQ-STORE-0005", "REQ-STORE-0006", "REQ-STORE-0007", "REQ-STORE-0008"], "bd-cache-sync": ["REQ-SYNC-0001", "REQ-SYNC-0004", "REQ-SYNC-0005", "REQ-SYNC-0010", "REQ-SYNC-0012", "REQ-SYNC-0013"], "sc-cron": ["REQ-CRON-0001", "REQ-CRON-0002", "REQ-CRON-0003", "REQ-CRON-0004", "REQ-CRON-0005", "REQ-CRON-0006", "REQ-CRON-0007", "REQ-CRON-0008", "REQ-CRON-0009", "REQ-CRON-0010", "REQ-CRON-0011", "REQ-CRON-0012", "REQ-CRON-0013", "REQ-CRON-0016", "REQ-CRON-0017", "REQ-CRON-0018", "REQ-CRON-0014", "REQ-CRON-0015"]}. Directly touched repo constraints NFR-BDC-0001 (store/cache latency), NFR-BDC-0003 (incremental sync time), REQ-BDC-0703 (tick budget), REQ-BDC-0726 (consumer ingest) and REQ-BDC-0728 (failure-state marking) are also absent. This is a coverage defect even where preservation prose or existing tests already addresses the behavior.
- REQ-QA-002 (blocking): c-1 explicitly moves sc-command to published 0.1.1, but its listed and binding REQ-CLI-0001 explicitly mandates a git dependency at v0.1.0. Proposed ADR-BDC-0016 names ADR-CLI-0001 and REQ-API-0008, but does not name REQ-CLI-0001; c-1 owns CLI architecture, not docs/bd-cache-cli/requirements.md. The plan therefore lacks an owned requirement amendment for the dependency-source/version change.
- REQ-QA-003 (blocking): c-2 defines a shared cache/schedule composition specifically to preserve one connection per process and forbids a second open, but omits governing NFR-BDC-0007 from its requirements list.
- REQ-QA-004 (blocking): c-4 owns packaging/release qualification and explicitly promises the abi3-py39 platform/Python matrix and same commit/version across installed artifacts, but omits REQ-PY-0010 (wheel platforms and ABI) and REQ-PY-0014 (workspace/wheel version lockstep). Its new packaging unit tests and installed consumer procedure also touch NFR-BDC-0005 and NFR-BDC-0006, which are missing from the list. The narrow inherited Jev scope exclusions do not waive these repository-level requirements.
- ARCH-001 (blocking): c-1 owns all local scheduler, store, sync, CLI and Python source/test paths but its authoritative adrs list omits binding decisions for those crates. Missing: ADR-CRON-0001, ADR-CRON-0002, ADR-CRON-0003, ADR-CRON-0004, ADR-CRON-0005, ADR-CRON-0006, ADR-CRON-0007, ADR-CRON-0008, ADR-CRON-0009, ADR-STORE-0002, ADR-STORE-0004, ADR-STORE-0005, ADR-SYNC-0001, ADR-SYNC-0002, ADR-SYNC-0006, ADR-CLI-0002, ADR-CLI-0003, ADR-CLI-0004, ADR-PY-0001, ADR-PY-0002, ADR-PY-0003, ADR-PY-0004, ADR-PY-0005, ADR-BDC-0009, ADR-BDC-0012, ADR-BDC-0013, ADR-CORE-0001. These govern retained clock/deadline/fencing and hook behavior, file-sharing/index rules, dependency/test features, composition/threading, command mapping/configuration and Python reply/packaging compatibility during the migration. The list also omits ADR-BDC-0009/0012/0013 although its bounds and validation explicitly preserve those subjects.
- ARCH-002 (blocking): c-2’s new application composition is explicitly constrained by cache outage behavior, named filter scopes, one cache connection and mutex-owned !Sync Cache, but its adrs list omits ADR-BDC-0005, ADR-BDC-0013, ADR-STORE-0003 and ADR-CORE-0001. c-4’s new tracked scripts/docs must preserve the no-consumer boundary, but its adrs list omits ADR-BDC-0010 and ADR-BDC-0012.
- ARCH-003 (blocking): All four sprint adrs lists contain "ADR-BDC-0016 (proposed)", but docs/architecture.md:1102 expressly says it is not an approved exception. The adrs contract is a list of governing binding ADR IDs; this entry neither names a binding decision nor separates the proposed amendment from current authority. The phase says old rules remain binding unless approved, but does not make an actual approval decision an explicit dispatch prerequisite for the migration/template/application work that relies on the amendment.

## Detailed Findings
### REQ-QA-001 — Blocking (req-qa)

docs/plans/phase-c/sprint-c-1-runtime-migration.md:19

c-1 owns every path in the store, sync, CLI, Python and local scheduler crates, but its authoritative requirements list omits governing approved crate requirements. Missing IDs by crate: {"bd-cache-cli": ["REQ-CLI-0002", "REQ-CLI-0004", "REQ-CLI-0005", "REQ-CLI-0006", "REQ-CLI-0008", "REQ-CLI-0009", "REQ-CLI-0010", "REQ-CLI-0011", "REQ-CLI-0012"], "bd-cache-py": ["REQ-PY-0001", "REQ-PY-0003", "REQ-PY-0004", "REQ-PY-0005", "REQ-PY-0007", "REQ-PY-0008", "REQ-PY-0010", "REQ-PY-0011", "REQ-PY-0012", "REQ-PY-0013", "REQ-PY-0014"], "bd-cache-store": ["REQ-STORE-0001", "REQ-STORE-0004", "REQ-STORE-0005", "REQ-STORE-0006", "REQ-STORE-0007", "REQ-STORE-0008"], "bd-cache-sync": ["REQ-SYNC-0001", "REQ-SYNC-0004", "REQ-SYNC-0005", "REQ-SYNC-0010", "REQ-SYNC-0012", "REQ-SYNC-0013"], "sc-cron": ["REQ-CRON-0001", "REQ-CRON-0002", "REQ-CRON-0003", "REQ-CRON-0004", "REQ-CRON-0005", "REQ-CRON-0006", "REQ-CRON-0007", "REQ-CRON-0008", "REQ-CRON-0009", "REQ-CRON-0010", "REQ-CRON-0011", "REQ-CRON-0012", "REQ-CRON-0013", "REQ-CRON-0016", "REQ-CRON-0017", "REQ-CRON-0018", "REQ-CRON-0014", "REQ-CRON-0015"]}. Directly touched repo constraints NFR-BDC-0001 (store/cache latency), NFR-BDC-0003 (incremental sync time), REQ-BDC-0703 (tick budget), REQ-BDC-0726 (consumer ingest) and REQ-BDC-0728 (failure-state marking) are also absent. This is a coverage defect even where preservation prose or existing tests already addresses the behavior.

Correction: Complete c-1 requirements.constrained_by for the existing owned-path fence using the enumerated approved IDs and repo constraints. For prototype-only sc-cron IDs, state their disposition under the proposed upstream ownership transition rather than silently dropping guarantees or copying an obsolete API. Keep behavior unchanged; do not add a new checklist or gate.

### REQ-QA-002 — Blocking (req-qa)

docs/plans/phase-c/sprint-c-1-runtime-migration.md:122; docs/plans/phase-c/sprint-c-1-runtime-migration.md:75

c-1 explicitly moves sc-command to published 0.1.1, but its listed and binding REQ-CLI-0001 explicitly mandates a git dependency at v0.1.0. Proposed ADR-BDC-0016 names ADR-CLI-0001 and REQ-API-0008, but does not name REQ-CLI-0001; c-1 owns CLI architecture, not docs/bd-cache-cli/requirements.md. The plan therefore lacks an owned requirement amendment for the dependency-source/version change.

Correction: Add the CLI requirements file to c-1 owned_paths and explicitly plan a narrowly approved amendment of REQ-CLI-0001 from the old git tag to the published baseline, preserving its version/envelope/usage and three-OS guarantees. Name that amendment in the existing proposed migration decision/requirement; do not silently change the Approved text or claim approval already exists.

### REQ-QA-003 — Blocking (req-qa)

docs/plans/phase-c/sprint-c-2-framework-application.md:20; docs/plans/phase-c/sprint-c-2-framework-application.md:173

c-2 defines a shared cache/schedule composition specifically to preserve one connection per process and forbids a second open, but omits governing NFR-BDC-0007 from its requirements list.

Correction: Add NFR-BDC-0007 to c-2 requirements.constrained_by; retain the existing shared-composition and duplicate-open tests. No new behavior or artifact is needed.

### REQ-QA-004 — Blocking (req-qa)

docs/plans/phase-c/sprint-c-4-release-qualification.md:16; docs/plans/phase-c/sprint-c-4-release-qualification.md:91

c-4 owns packaging/release qualification and explicitly promises the abi3-py39 platform/Python matrix and same commit/version across installed artifacts, but omits REQ-PY-0010 (wheel platforms and ABI) and REQ-PY-0014 (workspace/wheel version lockstep). Its new packaging unit tests and installed consumer procedure also touch NFR-BDC-0005 and NFR-BDC-0006, which are missing from the list. The narrow inherited Jev scope exclusions do not waive these repository-level requirements.

Correction: List REQ-PY-0010, REQ-PY-0014, NFR-BDC-0005 and NFR-BDC-0006 as c-4 constraints. Keep live installed-consumer proof on the designated scratch testbed and helper unit tests deterministic/no-bd, as already intended; no additional matrix or process gate is needed.

### ARCH-001 — Blocking (arch-qa)

docs/plans/phase-c/sprint-c-1-runtime-migration.md:59

c-1 owns all local scheduler, store, sync, CLI and Python source/test paths but its authoritative adrs list omits binding decisions for those crates. Missing: ADR-CRON-0001, ADR-CRON-0002, ADR-CRON-0003, ADR-CRON-0004, ADR-CRON-0005, ADR-CRON-0006, ADR-CRON-0007, ADR-CRON-0008, ADR-CRON-0009, ADR-STORE-0002, ADR-STORE-0004, ADR-STORE-0005, ADR-SYNC-0001, ADR-SYNC-0002, ADR-SYNC-0006, ADR-CLI-0002, ADR-CLI-0003, ADR-CLI-0004, ADR-PY-0001, ADR-PY-0002, ADR-PY-0003, ADR-PY-0004, ADR-PY-0005, ADR-BDC-0009, ADR-BDC-0012, ADR-BDC-0013, ADR-CORE-0001. These govern retained clock/deadline/fencing and hook behavior, file-sharing/index rules, dependency/test features, composition/threading, command mapping/configuration and Python reply/packaging compatibility during the migration. The list also omits ADR-BDC-0009/0012/0013 although its bounds and validation explicitly preserve those subjects.

Correction: Complete c-1’s existing adrs list with the listed binding IDs; do not copy their decision prose or change scope. Retain superseded historical ADRs as history rather than treating them as current constraints.

### ARCH-002 — Blocking (arch-qa)

docs/plans/phase-c/sprint-c-2-framework-application.md:50

c-2’s new application composition is explicitly constrained by cache outage behavior, named filter scopes, one cache connection and mutex-owned !Sync Cache, but its adrs list omits ADR-BDC-0005, ADR-BDC-0013, ADR-STORE-0003 and ADR-CORE-0001. c-4’s new tracked scripts/docs must preserve the no-consumer boundary, but its adrs list omits ADR-BDC-0010 and ADR-BDC-0012.

Correction: Add those governing binding IDs to the existing c-2 and c-4 adrs lists. Keep the existing behavior bounds; no new artifact or validation gate is needed.

### ARCH-003 — Blocking (arch-qa)

docs/plans/phase-c/sprint-c-1-runtime-migration.md:66

All four sprint adrs lists contain "ADR-BDC-0016 (proposed)", but docs/architecture.md:1102 expressly says it is not an approved exception. The adrs contract is a list of governing binding ADR IDs; this entry neither names a binding decision nor separates the proposed amendment from current authority. The phase says old rules remain binding unless approved, but does not make an actual approval decision an explicit dispatch prerequisite for the migration/template/application work that relies on the amendment.

Correction: Keep ADR-BDC-0016 Proposed until an actual authorized approval occurs. Separate the proposed decision from the binding adrs lists (for example an existing-doc proposed_adrs field), and explicitly block dispatch of dependent implementation until that approval is recorded. On approval, use the exact ID as governing authority. Do not mark it Approved merely because this QA round or the plan PR passes.

### RBP-F001 — Important (rust-best-practices-agent)

docs/plans/phase-c/sprint-c-2-framework-application.md:119

The new foreground daemon has startup/refusal failure modes but the plan never fixes their user-facing error and recovery contract. It names upstream endpoint validation and instance locking, and tests duplicate-instance refusal, yet only readiness is tied to existing cache error codes. An implementer must still choose how malformed daemon configuration, a disallowed listener, instance conflict and listener bind failure reach the operator, including stable code/typed source, output/exit behavior and actionable recovery.

Correction: Clarify these failure mappings in the existing c-2 daemon contract, using the pinned upstream error types/codes and current cache-health codes where available; state causes, actionable recovery and observable output/exit or HTTP behavior. A compact table or prose in the sprint doc is sufficient. Extend the existing process/HTTP tests to assert those defined failure outcomes. Do not introduce a new error framework, separate manifest or CI gate, and do not alter the preserved bdc exit contract.

### RSH-001 — Important (rust-service-hardening-agent)

docs/plans/phase-c/sprint-c-2-framework-application.md:181

The planned /ready adapter submits shared-cache work to the blocking pool but defines no finite admission bound or ownership of that work after request cancellation. Concurrent probes can accumulate blocking jobs waiting for the one cache mutex; the upstream HTTP timeout only drops the handler future, so submitted blocking work can survive the response or HTTP drain and escape the stated shutdown bound.

Correction: Bound readiness work before submitting it to the blocking pool, with an explicit saturation response and no unbounded waiter queue. Keep capacity owned until the actual blocking job finishes, and include admitted readiness work in application cleanup while instance ownership is held. State the cancellation/drain rule in the existing c-2 contract and add a bounded concurrent-probe/process acceptance scenario proving saturation, timed-out/disconnected probes and shutdown do not leave queued or orphan work. Reuse upstream request timeout behavior; do not add duplicate generic middleware or a new process artifact.

### CQA-F001 — Minor (ceremony-qa)

docs/plans/phase-c/phase-c-plan.md:198

The work-bounds section repeats the history of the removed ANC-012..015 packet and its rejected rubric, although the current inherited bounds are already linked immediately above and the same provenance separation is explained in intent.md. This history does not define any remaining implementation work.

Correction: Remove the rejected-packet history at lines198–202. Retain the current inherited-bounds link and the explicit statement that the daemon remains proposed scope.

CI: 14 SUCCESS; one Windows Rust leg IN_PROGRESS at observation; no CI PASS claimed

All ten findings retained by ceremony screen. Named roles executed using the inherited Codex model adaptation; unavailable Cursor/Grok/Sonnet model execution is not claimed.


## Resolved Since Last Pass
- none

## Merge Readiness
- Status: **not ready**
- Reason: Ten open findings (all severities); plan deliverables14/15. CI separately in progress. No implementation approval inferred.

## Next Action
- Action: triage_and_fix
- Owner: parent-orchestrator / plan author

## Reviewer Spawn Gate
- Gate: **pass**
- Reviewer manifest, missing/unparsed reviewers, and evidence chain are in the Machine Status JSON above.