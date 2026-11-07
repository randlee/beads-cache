# C2 published SDK 0.2 consumption

C2 consumes the approved C1/C3 integration
`d6022fe55480fcb9b076355cd29e3b9167d241cc` by ordinary merge `e774322`.
C1 PR108 actually merged at `3e4d7a12ec3f4d60fe9397e394fbe5bf0e45f889`
after QA16 approved all 40 deliverables and its required CI. Earlier failed
C1/C2 reports, severities, source-specific credits and retractions remain
historical; none establishes current C2 acceptance. Five conflicting audit files
retain their owning C2 text and the complete qualified producer history.

Application dependencies now use exact published sc-runtime/sc-config/sc-transport
0.2.0 alongside C1's exact sc-cron/sc-command 0.2.0. They share one nominal
registry identity; default feature choices, introducer guards and the single
rusqlite 0.39 SQLite provider remain. utoipa 6/utoipa-axum 0.3 follow the actual
released runtime's router type. No Git/path framework exception, patch, fork or
producer-code edit is used. The published archive checksums and actual source
inspection are recorded under `/tmp/phase-c-execution/c-2/registry020-resume`.

The unchanged C3 helper regenerated both immutable v0.1.1 and v0.2.0 baselines
from Git objects. The current pin is release tag v0.2.0 at
`118ee72d831f79778f3584f57a48706111754673`, with the same renderer versions.
The existing four answers are retained; new schema properties `cron=false` and
`cron_commands=false` explicitly preserve the application-owned cache scheduler.
All 25 changes are reported, including adopted paths; root Cargo.toml now also
has an explicit adopted mapping. Runtime dependency/router changes are adopted.
Optional generated cron/command services, new CLI commands, demo stores/routes,
sc-lint boundaries/bootstrap/recipes and generated CI are comparison data.
They do not replace existing application domain code, CI or quality policy.
The changed upstream main assertion concerns an unused generated cron path.
The upstream logger test-supervisor delta is retained for review; this round
preserves the existing application supervisors and their budgets.

C2's CLI graph adapter now recognizes the canonical guard's current exact
message for the existing CLI-only exception. Its fixture uses the exact current
registry identity; it still plants and rejects a direct sync introducer.
The service edge table includes its existing test-only store feature dependency
for the public startup contention observer; production opens no second store.
Actual independently resolved API/Python/CLI normal/build graphs pass, and an
actual independently resolved server-feature root fails as required.

Public Application startup/post-start contention and zero-timeout regressions
pass. Startup shutdown joined in 251.377 ms with T=250 ms and 2T=500 ms; zero
joined in 1.285 ms. The unchanged public QA5 probe source, rebuilt with the sole
nominal dependency update, observed 251.961 ms and returned Ok, while ordinary
same-connection management remained pending until explicit rollback. All caller,
worker/readiness and management/probe joins are retained. These are public
in-process Application cases, not stock daemon HTTP reproductions.

The real Unix lifecycle test consumes released `run_with_cleanup_and_bound`:
port zero is bound by runtime itself, its callback supplies the actual endpoint,
and a deterministic competing bind must fail while it remains owned. Real
SIGTERM, run_with_cleanup semantics, retained instance ownership, readiness join
and same-root/endpoint restart remain. A portable released cleanup-fixture case
also retains real Application work and lock/endpoint controls.

Qualification currently has a concrete producer blocker: SDK0.2.0 removed the
public `testing::instrument_router` surface that SDK0.1.1 supplied. Existing
canceled-readiness tests use that genuine upstream middleware to obtain a 50 ms
HTTP timeout while actual work remains within its original one-second budget.
Production's private 30-second timeout cannot qualify that interleaving. Affected
all-target Clippy fails on the absent symbol. The exact failure, archive/source
identities, enabled features and minimal compatible restoration are preserved in
`producer-gap.json`; no replacement middleware, skip or budget change is made.

Assigned finding dispositions remain requests for independent review:

- QA-C2-001: released Windows CTRL_BREAK handling is consumed; actual hosted
  installed stop, reap, ownership release and restart must qualify this candidate.
- C2-REQ-003: fresh tested-source `ab7b5bc` native/wheel scratch proof passes
  the bounded supported workflow; the [actual receipt](c-2-consumer-qualification.md)
  retains bridge/create limits. Full exact-head OS/Python CI and independent
  acceptance remain; historical 6232180/65250a9 receipts are not retargeted.
- C2-REQ-004 / RSH-001: C1 correction is consumed unchanged and the actual public
  Application probes pass original bounds; owning C2 review remains required.
- C2-REQ-007 / ARCH-001 / RBQA-F001: real public cron logging facade and narrow
  isolated positive/negative guards are consumed; current wheels/platform CI and
  owning C2 review remain required. C1 scoped owner credits are preserved.
- C2-REQ-009 / C4-FTQ-001: public runtime-owned endpoint and cleanup-fixture
  controls are implemented; absent public timeout instrumentation currently
  blocks their required current qualification. Existing real-signal assertions
  remain; no reserve/free/rebind listener handoff remains in that fixture.
- FTQ-011 / RQA9-001 / RQA10-001 / RQA11-001: exact qualified C1 helper/store
  corrections are consumed without edits; historical failures and unknown
  low-level timing causes remain preserved. Current C2 matrix proof is separate.
- CI-C2-RUNNER-001: qualified supported macOS Python labels are consumed;
  current C2 wheel/install/Python-version matrix acceptance remains required.

The first full script run exposed five C3 generation-fixture failures because
its four-property mock schema copied the new six-property application answers.
Parent explicitly routed and authorized a fixture-only schema/expected-values
correction on C2; the helper implementation and planted failures are unchanged.
The original failing log is retained for QA6 and C3 contract review.

Developer runs as gpt-6.1-sol at high reasoning because configured Grok is
unavailable. This receipt claims no independent QA, anchor PASS, closure, merge
readiness or publication. Parent owns producer routing and final QA6/anchor/
enforcement gates. Exact candidate SHA, commands/logs, artifact provenance,
consumer results and actual CI handles are recorded in the external developer
result rather than extending the sprint's historical process narrative.
