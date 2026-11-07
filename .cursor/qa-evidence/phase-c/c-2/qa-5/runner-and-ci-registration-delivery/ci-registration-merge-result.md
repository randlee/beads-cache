# C2 CI registration merge result

Pushed ordinary merge `2a6bfd0ec55802db0b9a9c0978ae5778a2050774` on existing
`sprint/c-2-framework-application`. Exact parents: C2
`c8d0c9e4906d576407459d0722f7a61989969ee2` and authoritative remote integration
`f1e0756e0e17a2002a69a7dcfba2de60aeb6887c` verified by ls-remote and fetched exactly.
No root local uncommitted audit or moving C1 source was consumed; integration was not pushed.

Actual Git conflicts were only RQA9/RQA10 TTL append histories: owning delivery
records versus incoming terminalOwnerAssessment. Entire owning records and every
unique incoming nonblank line are preserved. All13 findings/scoped closures/
retractions remain; canonical runnerTTL and C2-FTQ-006 blobs unchanged.

Full crates/manifests/lock/runtime/nativeCI/Pythonrunnerfix/consumer source hashes
are unchanged; only .cursor evidence/orchestration and .triage history change.
No Rust delta, Cargo, tests or fmt. No rebase/force/CI dispatch/rerun/auth/SDK/
publisher changes. Worktree clean.

Whole-merge gitdiffcheck exits2 on one incoming archived rawWindows log's CRLF.
Raw evidence remains byte-identical to frozenf1:266161bytes,3085CRLF, SHA256
`818a94d57a41b8298435e58498f70304ccfa3a8f746f387c6aaa1846e84d10a8`.
The failure output is preserved; no archive rewrite or waiver. Actual resolved
triage diffcheck passes. Exact graph/source/history checks pass.

PR109 bounded snapshot now reports **MERGEABLE**, mergeState **UNSTABLE**;
baseRefOid=f1 and headRefOid=2a6 match actual Git facts. Checks9 have registered.
This resolves the actual metadata conflict and restores observed CI registration;
it is not CI/QA/wholeacceptance or PR merge approval.

- Native CI run41798831453: [Windows126469322203](https://github.com/randlee/beads-cache/actions/runs/41798831453/job/126469322203) in progress; [macOS126469322220](https://github.com/randlee/beads-cache/actions/runs/41798831453/job/126469322220) queued. Linux126469322169 in progress; boundary126469321800 failure.
- Python run41798831422: macos15Rust126469321667 executed/failure; macos15wheel126469321781 queued. First5 matrix jobs registered; dependent pytest cases remain later. No raw cause inferred from these conclusions.

Original frozen7fb observation run41796715401/job126462431901 untouched.
Cargo unused, active0; C1 retains sole slot. All13 original dispositions remain,
including OPEN runner finding and unresolved historical RQA9/RQA10. **Not merge-ready**.
Stopped; parent owns remote diagnosis/monitoring and next independent QA.
