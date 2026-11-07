# QA Findings Update

Generated: 2026-11-04T04:06:23.898952+00:00
QA Pass: qa-5
Sprint/Task: c-2 / qa-c-2-5
Branch: `sprint/c-2-framework-application`
Commit: `45c9dd13258482ba8af3e5cfeae5ce146b120453`
PR: #109
Verdict: **FAIL**

## Machine Status (JSON)
```json
{
  "sprint": "c-2",
  "task": "qa-c-2-5",
  "branch": "sprint/c-2-framework-application",
  "commit": "45c9dd13258482ba8af3e5cfeae5ce146b120453",
  "pr": 109,
  "verdict": "FAIL",
  "deliverables": {
    "complete": 14,
    "total": 21,
    "percent": 66.67
  },
  "findings": {
    "blocking": 7,
    "important": 2,
    "minor": 0
  },
  "blocking_ids": [
    "ARCH-001",
    "C2-REQ-003",
    "C2-REQ-004",
    "C2-REQ-007",
    "C2-REQ-009",
    "RBQA-F001",
    "RSH-001"
  ],
  "merge_readiness": "not ready",
  "merge_reason": "Nine open findings (7 Blocking, 2 Important); deliverables 14/21; current CI 10 success/5 failure; parent, exact-candidate qualification and acceptance gates remain.",
  "next_action": "triage_and_fix",
  "owner": "parent-orchestrator; C1 producer and C2 caller/acceptance owners",
  "reviewer_spawn_gate": "pass",
  "reviewer_manifest": [
    {
      "agent": "flaky-test-qa",
      "task_id": "/root/c4_flaky",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "FINDINGS",
      "findings": {
        "blocking": 0,
        "important": 2,
        "minor": 0
      }
    },
    {
      "agent": "req-qa",
      "task_id": "/root/c2_req4",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "FAIL",
      "findings": {
        "blocking": 4,
        "important": 0,
        "minor": 0
      }
    },
    {
      "agent": "arch-qa",
      "task_id": "/root/c2_arch4",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "FAIL",
      "findings": {
        "blocking": 5,
        "important": 0,
        "minor": 0
      }
    },
    {
      "agent": "rust-qa-agent",
      "task_id": "/root/c2_rustqa4",
      "spawn_actor": "parent-orchestrator",
      "invocation": "parent_followup_existing_independent_reviewer",
      "fenced_json_received": true,
      "verdict": "FINDINGS",
      "findings": {
        "blocking": 4,
        "important": 2,
        "minor": 0
      }
    }
  ],
  "missing_reviewers": [],
  "unparsed_reviewers": [],
  "evidence_chain": {
    "qa_pass": "qa-5",
    "commit": "45c9dd13258482ba8af3e5cfeae5ce146b120453",
    "pr_comment_url": "https://github.com/randlee/beads-cache/pull/109#issuecomment-6622258720",
    "coordinator_task_id": "/root/c2_quality_manager5",
    "reviewer_tasks": {
      "flaky-test-qa": "/root/c4_flaky",
      "req-qa": "/root/c2_req4",
      "arch-qa": "/root/c2_arch4",
      "rust-qa-agent": "/root/c2_rustqa4"
    },
    "triage": {
      "ttl_paths": [
        ".triage/c/findings/QA-C2-001.ttl",
        ".triage/c/findings/C2-REQ-003.ttl",
        ".triage/c/findings/C2-REQ-004.ttl",
        ".triage/c/findings/C2-REQ-007.ttl",
        ".triage/c/findings/C2-REQ-009.ttl",
        ".triage/c/findings/ARCH-001.ttl",
        ".triage/c/findings/RSH-001.ttl",
        ".triage/c/findings/RBQA-F001.ttl",
        ".triage/c/findings/C4-FTQ-001.ttl",
        ".triage/c/findings/C2-FTQ-006.ttl"
      ]
    },
    "ci": {
      "success": 10,
      "failure": 5,
      "pending": 0,
      "snapshot": "/tmp/phase-c-execution/c-2/qa-5/manager-ci.json"
    },
    "todo_scan": {
      "rows": 0,
      "path": "/tmp/phase-c-execution/c-2/qa-5/manager-todo-scan.txt"
    },
    "ceremony_adjudication": "/tmp/phase-c-execution/c-2/qa-5/screen-parent-adjudication.json",
    "assignment_sha256": "bd7062cfa9655669c07444c5b63b6c0768faa3fe23817c694b7db59f883ca29d",
    "handoff_proof": "/tmp/phase-c-execution/c-2/qa-5/manager-validated-handoff.json",
    "publish_mode": "comment_after_github_self_review_rejection"
  },
  "aggregation_source": "reviewer_fenced_json_union_todo_scan",
  "open_ids": [
    "ARCH-001",
    "C2-REQ-003",
    "C2-REQ-004",
    "C2-REQ-007",
    "C2-REQ-009",
    "C4-FTQ-001",
    "QA-C2-001",
    "RBQA-F001",
    "RSH-001"
  ]
}
```

## Findings Summary
- Deliverables: 14/21 (66.67%)
- Blocking: 7
- Important: 2
- Minor: 0

## Blocking Findings
- ARCH-001: OPEN
- C2-REQ-003: OPEN
- C2-REQ-004: OPEN
- C2-REQ-007: OPEN
- C2-REQ-009: OPEN
- RBQA-F001: OPEN
- RSH-001: OPEN

## Detailed Findings
### ARCH-001 — Blocking — OPEN

The production sync crate still introduces sc-command directly; cron_log.rs:3 imports CanonicalLog/NoopCanonicalLog/SharedCanonicalLog from it. ADR-BDC-0016 item5 permits only the upstream sc-cron-carried contract/logging exception beneath isolated Python/API consumers. Current root manifests remain pinned to published0.1.1; typed daemon error retention does not remove this introducing edge. The canonical graph gate correctly rejects it, so the former enforcement gap is not reopened.

Correction: Consume the owning C1/public published logging-facade correction, remove the direct sync edge, and qualify the isolated normal/build graph and wheels. Keep the narrow ADR/guard unchanged; no local fork or waiver.

Evidence: 

Reviewers: arch-qa, rust-qa-agent. Triage: `.triage/c/findings/ARCH-001.ttl` — still open.



### C2-REQ-003 — Blocking — OPEN

Exact reviewed45c9dd1 PR109 CI is not green: {'IN_PROGRESS': 1, 'SUCCESS': 10, 'FAILURE': 4}. Directly read Linux/macOS/Windows Python Rust logs show the actual unchanged forbidden sync-to-sc-command graph assertion; the boundary composite log shows unsuppressed paste RUSTSEC-2024-0436. Other job outcomes are retained as observed, not inferred. C1 PR108 remains OPEN at5255610 with no merge commit and its scoped producer qualification is not whole-C1 acceptance. No exact45c9dd1 native/wheel supported-consumer scratch qualification exists; historical623 C2 and652 C4 workflows retain their original source/bridge/platform limits. Current parent/CI/consumer/QA/anchor completion gates remain open.

Correction: Complete all current-source required CI and parent gates, preserving actual graph/advisory/Windows failures until their genuine corrections are adopted. Merge the completed C1 parent before C2 completion and qualify final same-source native/wheel consumer artifacts. Do not retarget historical receipts or waive/suppress a gate.

Evidence: /tmp/phase-c-execution/c-2/qa-5/req-qa-ci.json; /tmp/phase-c-execution/c-2/qa-5/req-qa-parent.json; docs/plans/phase-c/evidence/c-2-consumer-qualification.md:5; docs/plans/phase-c/evidence/c-2-qualified-shutdown-consumption.md:70

Reviewers: req-qa, rust-qa-agent. Triage: `.triage/c/findings/C2-REQ-003.ttl` — still open.



### C2-REQ-004 — Blocking — OPEN

The exact consumed qualified producer repairs post-start worker/store waits, but does not satisfy the original all-lifetime2T stop bound. A supported public Application caller may retain the Arc returned by Composed::schedule_store and issue ordinary same-connection management work before handing Composed to Application::start; no exclusive-startup precondition is documented. Independently executed public-API probe proves management owns the connection under an external BEGIN IMMEDIATE writer. cron_run_hurried awaits warn_at_startup()->rows on the ordinary management view atcron.rs171 before polling StopFlag or installing its deadline. Actual Application.shutdown remains pending at902.343541ms withT250ms/original2T500ms, and returns Ok only at978.142166ms after writer rollback. The local owned startup wait escapes the stop budget. This is not a new SDK deficiency and source equivalence to a previously qualified component does not exclude this counterexample.

Correction: Route the remaining startup-lifetime budget/ownership defect to local C1 cron source and consume its correction in C2. Ensure owned startup diagnostic store work and early stop obey the original stop deadline while retaining real joins and independent ordinary management policies. Add a causal actual public Application/start-stop regression with a retained public same-connection management operation contended behind an external writer; verify join within original2T before releasing the writer. Preserve current successful post-start ordinary/zero-timeout tests. Do not invent exclusive-startup preconditions, require unrelated SDK publication, pad or reset budgets, detach/drop owned work, cancel unrelated management operations, or remove diagnostic validation.

Evidence: crates/bd-cache-sync/src/cron.rs:171; crates/bd-cache-sync/src/schedule.rs:760; crates/bd-cache-sync/src/compose.rs:82; crates/bd-cache-service/src/lib.rs:70; crates/bd-cache-service/src/lib.rs:121; /tmp/phase-c-execution/c-2/qa-5/startup-probe-result.json; /tmp/phase-c-execution/c-2/qa-5/rustqa-startup-probe.log

Reviewers: req-qa, arch-qa. Triage: `.triage/c/findings/C2-REQ-004.ttl` — still open.

Scope: supported public Application/SystemSchedules retained-management startup interleaving, locally C1-owned. Shutdown pending 902.343541 ms exceeds the original 500 ms bound (T=250 ms); after explicit rollback it completes at 978.142166 ms. Stock daemon composition does not itself start that independent management writer; no stock-daemon HTTP reproduction or new SDK requirement is asserted.



### C2-REQ-007 — Blocking — OPEN

The consumed exact5255610 sync producer still declares the unauthorized direct sc-command edge at Cargo.toml31. Canonical independently resolved graph enforcement remains correctly wired and narrow; actual current Linux/macOS/Windows Python Rust checks reject the direct sync introducer. The shutdown component qualification does not remove this edge or amend ADR-BDC-0016/REQ-API-0008. Genuine canonical logging must be retained through the approved public seam; no introducer relocation or whitelist is accepted.

Correction: Consume the approved published upstream logging seam through C1, remove the actual direct sync introducer, and retain canonical independent-root/normal-build and planted direct-edge negative checks. No broader boundary exception, removed logging or local framework workaround.

Evidence: crates/bd-cache-sync/Cargo.toml:31; scripts/consumer_checks.py:47; .cursor/qa-evidence/phase-c/c-1/qa-7/component-qualification.json; /tmp/phase-c-execution/c-2/qa-5/req-qa-ci.json

Reviewers: req-qa. Triage: `.triage/c/findings/C2-REQ-007.ttl` — still open.



### C2-REQ-009 — Blocking — OPEN

The actual Unix public run_with_cleanup/SIGTERM/LockHeld/read-join/restart supplement is present and byte-identical to reviewed3282502. New in-process Application writer-contention/zero-timeout tests and qualified producer binding repair the worker/store component; they do not exercise runtime listener ownership, controlled stop or the instance lock. The actual runtime fixture still drops a reserved ephemeral listener before runtime bind, and no public owned-port0 notification/cleanup/signal mechanism or deterministic competing-bind negative has been consumed. Portable lifecycle qualification and actual Windows CtrlBreak graceful stop remain open. A passing Unix sample or bounded Application join cannot substitute for those required daemon/process observables.

Correction: Retain the actual public Unix lifecycle proof and the now-correct bounded Application worker binding. Complete race-free public actual-bound-endpoint/cleanup/signal and runtime-instance-lock process proof on required platforms, including Windows graceful stop, using supported SDK behavior. No private discovery, custom signal/factory, free-port handoff, retries, sleeps, widened deadlines or replacement with in-process-only tests.

Evidence: crates/bd-cache-daemon/src/readiness_tests/runtime_lifecycle.rs:39; crates/bd-cache-daemon/src/readiness_tests/runtime_lifecycle.rs:191; docs/bd-cache-daemon/requirements.md:52; crates/bd-cache-service/src/shutdown_tests.rs:33; docs/plans/phase-c/evidence/c-2-qualified-shutdown-consumption.md:54

Reviewers: req-qa, arch-qa. Triage: `.triage/c/findings/C2-REQ-009.ttl` — still open.



### C4-FTQ-001 — Important — OPEN

Existing endpoint ownership gap remains unchanged: the test drops its ephemeral reservation before the real runtime binds that released address, allowing a concurrent listener to acquire it. Store-start handshakes precede bind and cannot close the gap. The documented upstream fixture need and honest OPEN receipt do not constitute a fix.

Correction: Use a compatible public upstream endpoint notification/owned-listener mechanism preserving actual run_with_cleanup, signals and instance-root ownership; observe the runtime-owned bound endpoint without releasing/rebinding a reserved port. Add a deterministic competing-bind negative. Do not add bind retries, random-port loops, sleeps, timeout widening, a local framework fork, or substitute a fixture that omits the cleanup path.

Evidence: Current45c9dd1 recheck: Previously reviewed3282502 runtime_lifecycle.rs191-193 binds127.0.0.1:0, copies local_addr and drops the listener;195 fixes the now-free address;197 starts the runtime. start() sends state from stores before bind. Pinned builder.rs207-208 explicitly orders stores/routes before bind, with actual bind at356. File is byte-identical to e7bafc2 via git diff. docs/plans/phase-c/evidence/c-2-cross-sprint-qa4.md32-59 preserves OPEN and explains why existing DaemonFixture::start/with_control cannot supply the required public cleanup plus actual endpoint seam. C4-FTQ-001 TTL remains open. Current source retains these exact signal/endpoint operations. No fixed disposition or new execution is claimed.

Reviewers: flaky-test-qa, rust-qa-agent. Triage: `.triage/c/findings/C4-FTQ-001.ttl` — still open.



### QA-C2-001 — Important — OPEN

Pinned runtime Windows CtrlC-only handling still fails the actual isolated-process CTRL_BREAK graceful-stop acceptance.

Correction: Adopt the approved public upstream Windows CtrlBreak path and pass unchanged zero-exit, worker/readiness joins, descendant reap, unlock and restart assertions. Do not accept abnormal termination or add a competing local signal handler.

Evidence: Actual current Windowscore job126370045569 at45c9dd1 reaches first_stop=stop(child) and fails line394 with (3221225786, empty stdout, empty stderr); pinned sc-runtime0.1.1 shutdown listener remains CtrlC-only. Same-source hosted native tests passed before the installed process acceptance; complete log retained.

Reviewers: flaky-test-qa, rust-qa-agent. Triage: `.triage/c/findings/QA-C2-001.ttl` — still open.



### RBQA-F001 — Blocking — OPEN

Assigned original boundary-owner ID remains OPEN at the same direct sync-to-sc-command introducing edge. The public logging adapter and isolated-consumer policy retain the same binding exception scope. This is correlated with ARCH-001 rather than a newly invented defect; the separate canonical ID is preserved as requested by the owning sprint record.

Correction: Resolve through the C1/published upstream logging surface, remove this direct edge and obtain the owning boundary disposition after isolated qualification. Preserve canonical receiver delivery and fail-closed graph enforcement.

Evidence: 

Reviewers: arch-qa, rust-qa-agent. Triage: `.triage/c/findings/RBQA-F001.ttl` — still open.



### RSH-001 — Blocking — OPEN

The qualified post-start worker/store correction is consumed correctly, but the public startup path still reads warnings through the unbounded management view before observing stop. A supported retained schedule view can hold that shared connection during an ordinary management write, extending an actual Application shutdown beyond2T before the bounded worker starts.

Correction: Route this newly discovered startup occurrence to the C1 shared cron producer owner: make startup work stop-aware and cooperatively bounded within the existing absolute2T lifetime, preserving real owned joins and ordinary independent management policy. Add a deterministic early-stop/shared-connection regression. No new SDK surface, detached task, timeout widening or management-policy suppression is required by this finding. After the producer correction, consume the exact source and requalify the caller.

Evidence: Independent source-free public-API probe at45c9dd1 retains Composed::schedule_store(), starts its ordinary set_enabled write under a real external BEGIN IMMEDIATE writer and proves actual same-connection mutex contention via a bounded worker view notification. Then Application::start(grace250ms) and actual shutdown remain pending902.343541ms (>original2T500ms); after explicit writer ROLLBACK, management returnsOk(true), both probe/management threads join and Application returnsOk at978.142166ms. Direct probe process exited0 and was waited/reaped; no product edits/private access. Source cron171 warn_at_startup()->schedule.rows760 reads original management store before stop loop and deadlines200+.

Reviewers: arch-qa, rust-qa-agent. Triage: `.triage/c/findings/RSH-001.ttl` — still open.

Scope: supported public Application/SystemSchedules retained-management startup interleaving, locally C1-owned. Shutdown pending 902.343541 ms exceeds the original 500 ms bound (T=250 ms); after explicit rollback it completes at 978.142166 ms. Stock daemon composition does not itself start that independent management writer; no stock-daemon HTTP reproduction or new SDK requirement is asserted.



### Current exact-head CI
- SUCCESS: [pytest (macos-14, python 3.13)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126371105590)
- SUCCESS: [pytest (macos-14, python 3.9)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126371105550)
- SUCCESS: [pytest (ubuntu-24.04, python 3.13)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126371105622)
- SUCCESS: [pytest (ubuntu-24.04, python 3.11)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126371105526)
- SUCCESS: [pytest (ubuntu-24.04, python 3.9)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126371105504)
- SUCCESS: [pytest (macos-14, python 3.11)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126371105466)
- FAILURE: [clippy and test (windows-latest)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126370046279)
- SUCCESS: [wheel (macos-14)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126370046334)
- FAILURE: [clippy and test (macos-14)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126370046347)
- SUCCESS: [wheel (ubuntu-24.04)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126370046016)
- FAILURE: [clippy and test (windows-latest)](https://github.com/randlee/beads-cache/actions/runs/41767757266/job/126370045569)
- SUCCESS: [clippy and test (macos-latest)](https://github.com/randlee/beads-cache/actions/runs/41767757266/job/126370045893)
- SUCCESS: [clippy and test (ubuntu-latest)](https://github.com/randlee/beads-cache/actions/runs/41767757266/job/126370045806)
- FAILURE: [fmt, doc, publish dry-run, boundaries](https://github.com/randlee/beads-cache/actions/runs/41767757266/job/126370045969)
- FAILURE: [clippy and test (ubuntu-24.04)](https://github.com/randlee/beads-cache/actions/runs/41767757295/job/126370046194)

## Resolved Since Last Pass
- C2-FTQ-006: independently retracted false positive, not a waiver. The actual same-UID stopped fixture child was force-killed and reaped in 0.0013147504 seconds under supported healthy-kernel process control. No reachable credential-change, inventory-subprocess or repeated-budget defect was found. Its TTL and superseded draft artifacts remain preserved; it is absent from every current findings array.
- Qualified immutable C1 producer source 5255610a16ef4500f0af508e29a533707a4a3e0a remains credited: store/sync trees match exactly. All 35 independently executed scoped tests pass, including actual post-start Application ordinary/zero-timeout contention joins and the real Unix lifecycle sample; strict scoped Clippy and fmt pass. These positives do not establish whole C1/C2 acceptance or erase the startup counterexample.
- All nine parent ceremony decisions are accepted as not_applicable; no finding was waived or excluded. Distinct correlated ARCH-001/RBQA-F001 IDs remain in the union.
- Coordinator scope: evidence enforcement, TODO scan, current CI and report publication only. No coordinator source review, Cargo, implementation or reviewer agents. Available inherited model used; no claim to the unavailable YAML role model.


## Merge Readiness
- Status: **not ready**
- Reason: Nine open findings (7 Blocking, 2 Important); deliverables 14/21; current CI 10 success/5 failure; parent, exact-candidate qualification and acceptance gates remain.

## Next Action
- Action: triage_and_fix
- Owner: parent-orchestrator; C1 producer and C2 caller/acceptance owners

## Reviewer Spawn Gate
- Gate: **pass**
- Reviewer manifest, missing/unparsed reviewers, and evidence chain are in the Machine Status JSON above.
