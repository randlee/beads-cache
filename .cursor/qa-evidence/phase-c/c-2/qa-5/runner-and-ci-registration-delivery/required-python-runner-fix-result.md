# Required Python runner correction

Pushed `c8d0c9e4906d576407459d0722f7a61989969ee2` on existing
`sprint/c-2-framework-application` / PR109, direct child of frozen7fb2246.
Only files: `.github/workflows/python.yml` and unchanged parent canonical
`.triage/c/findings/CI-C2-RUNNER-001.ttl`. No extra merge.

Exactly three executable label replacements at40/75/117: macos-14→macos-15.
PASS: installed PyYAML full AST comparison after only3 substitutions, exact raw
text/diff scope, expanded3+2+6=11 matrix, Python3.9/3.11/3.13, unchanged actions/
commands/guards/timeouts/triggers/needs and matching artifact upload/download.
Existing consumer lint and diffcheck PASS. No actionlint installed; no install.
No Cargo, broad suite, generated test file or product/test change.

Allcrates tree `13e7df86d9d103b6a85f1fd99ab0d97dfe89d4ab`, manifests/lock,
runtime/pins/adoption, docs, nativeCI and consumer script are unchanged.
Every prior history and parent canonical record is byte-identical. All13 remain
tracked; scoped closures and C2-FTQ-006 retraction preserved. CI-C2-RUNNER-001
stays OPEN pending actual changed-source receipts and independent owner validation.

PR109 exact head confirmed. Two bounded exact-head run-list snapshots return[];
check-runs API total0. **New native/Python handles are not yet registered.** Parent
was notified. No CI outcome, queue cause, manual dispatch or green wait inferred.
Actual macOS15 arm64/Pythonversions/wheel/installed-pytest qualification remains
required. Retirement explains unsupported configuration; some prior retired-label
jobs executed, so their specific queue cause remains unproven.

Frozen7fb observation run41796715401/job126462431901 is untouched. No SDK/publisher,
guard, source, generation/pin/fingerprint or acceptance change; no nested agents,
self-QA closure or PRmerge. Worktree clean; Cargo unused, active0. **Not merge-ready**.

- QA-C2-001: OPEN: current37b Windows first-stop CTRL_BREAK abnormal exit3221225786 confirmed; published runtime stop support remains required.
- C2-REQ-003: OPEN: current CI, consumer, parent merges, independent QA and anchor acceptance remain incomplete; advisory retained.
- C2-REQ-004: OPEN owning C2 disposition: existing qualified startup/post-start correction retained; this diagnostic promotion adds no broad acceptance.
- C2-REQ-007: OPEN: actual direct sync -> sc-command edge still requires approved published logging receiver seam.
- C2-REQ-009: OPEN: portable endpoint/signal/instance-lock qualification remains incomplete.
- ARCH-001: OPEN: published canonical logging seam/direct introducing edge unchanged; strict graph remains rejecting.
- RSH-001: OPEN owning C2 disposition; independently resolved supported C1 and paired public C2 startup scope preserved, not reopened.
- RBQA-F001: OPEN: distinct carried logging dependency finding retained.
- C4-FTQ-001: OPEN: released ephemeral reservation race retained; public endpoint seam required.
- FTQ-011: OPEN owning C2 disposition; original independently corrected50ms-cap mechanism/scoped closure preserved. RQA10 remains a distinct actual recovery timeout.
- RQA9-001: OPEN historical failed qualification/cause unknown. Independent owner credits validated observability/control scope as resolved; no further unchanged store rerun.
- RQA10-001: OPEN: same recovery inventory timeout observed on C1 macOS8c and actual C2 macOS37b. Stage/call/root/census cause unresolved. Exact ec68 diagnostics now consumed; no cause correction claimed.
- CI-C2-RUNNER-001: OPEN: exact three supported arm64 labels corrected and static contracts PASS; actual changed-head macOS15/toolchain/Python3.9/3.11/3.13/wheel/pytest receipts and independent owner validation pending. No self-closure.

Stopped. Parent owns CI registration, remote monitoring and independent QA.
