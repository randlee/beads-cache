# Qualified C1 shutdown component consumed by C2

Owning C2 remains in progress. This receipt requests independent C2 QA5; it
closes no C2 finding, anchor, sprint or PR. Whole C1 remains FAIL (3 Blocking,
1 Important), and PR108 is not merged. No SDK/pin/baseline, publisher, graph
exception, signal handler or endpoint workaround is introduced.

## Immutable sources and merge resolution

- Before: `465018044fde792e2f1ea4d828f8fa08ca4cfc86`.
- Integration: `d4af63d2a81bebd6b5b967ef5032731152bb5f7c`, consumed by clean
  ordinary feature merge `74536fb88f0ea28884be65e8b501882fdc9fb404`.
- Qualified C1: `5255610a16ef4500f0af508e29a533707a4a3e0a`, consumed by
  `ddc0027ee06143eb5d7900efed4ad0ee84585bdd`.
- Only producer-merge conflicts: `.triage/c/findings/ARCH-001.ttl`,
  `RBQA-F001.ttl`, `RSH-001.ttl`. Each keeps its complete owning C2 record,
  open status and historical QA reviews, with appended scoped producer
  provenance. Integration's latest orchestration state remains intact.
- Producer-only FTQ-009 retains its independently closed disposition;
  FTQ-010 records actual C1 QA7 closure from the tracked report/qualification,
  rather than reopening it from stale committed pre-QA status.

[Tracked component qualification](../../../../.cursor/qa-evidence/phase-c/c-1/qa-7/component-qualification.json)
and [independent C1 QA7 report](https://github.com/randlee/beads-cache/pull/108#issuecomment-6621585641)
qualify only bounded producer shutdown plus complete owned fixtures and Linux
portability. They do not waive caller requirements or whole-C1 failure.

Both complete `crates/bd-cache-store` and `crates/bd-cache-sync` trees are
byte-identical to exact qualified C1 source after the merge. The local JSON
result records the matching tree hashes. C2 service/daemon, root manifest/lock,
CI, template helper, graph enforcement and owning receipts retain their legitimate
caller differences. The new lock entry adds existing store/rusqlite packages only
to service **dev-dependencies** for the actual contention fixture; package pins,
production dependency edges and the shared one-connection composition do not change.

## Actual caller binding

`Application::start_inner` already calls `SystemSchedules::system` over
`composed.schedule_store()` and the same cache. Consumed `system` now selects
`new_with_bounded_store`, making a worker view of the existing connection and
retaining `ScheduleShutdownControl`. The qualified cron path propagates its
absolute grace deadline, cancels current mutex/busy/progress waits, calls expiry
at T and retains the real upstream worker/store join within the original 2T.
Management calls retain their ordinary policy on the same scheduler core.

Application cleanup closes readiness admission, sets StopFlag, and joins its
real cron task and retained readiness thread. Daemon composition still invokes
that cleanup from public `run_with_cleanup`, while the upstream runtime retains
instance ownership. No production caller defect or interface change was needed.

Two new service unit regressions start the actual public Application, causally
observe **its** worker's SQLite busy callback while another test-only connection
holds `BEGIN IMMEDIATE`, then stop and join before that writer releases. Ordinary
250ms grace retains the original 500ms total bound; zero-timeout uses the
producer's existing 100ms scheduling observation tolerance without granting a
grace period. Both verify closing readiness, released cache claim and v3 reopen.
The test-only receiver observes the constructed worker and adds no public API.
Complete fixtures run under the existing ten-second owned child watchdog;
failure paths release the writer and retain the shutdown future's actual join.
No Python/backend child is launched by these contended store scenarios.

Existing real daemon readiness/timeout/cleanup/SIGTERM/LockHeld/restart coverage
is retained and rerun. Its known endpoint reservation race remains explicitly
open; a passing sample does not establish race-free listener ownership.

## All nine C2 findings

| Finding | Current disposition |
|---|---|
| QA-C2-001 | Open: published runtime0.1.1 CtrlC-only Windows handling does not support isolated-group CTRL_BREAK graceful stop. |
| C2-REQ-003 | Open: required exact C2 CI/native/wheel consumer/independent QA/anchors and C1 merge gates. Historical6232180 consumer proof is not retargeted. Paste advisory remains unsuppressed under unchanged approved baseline. |
| C2-REQ-004 | Implemented pending independent C2 QA5: exact qualified producer bound consumed; actual Application contention and zero-timeout caller regressions added. |
| C2-REQ-007 | Open: actual direct sync -> sc-command remains; requires approved published logging seam. Existing fail-closed graph and planted negatives remain. |
| C2-REQ-009 | Partial, open: real Unix public lifecycle proof retained and bounded Application binding requests QA5; endpoint observation and Windows stop still prevent complete portable lifecycle qualification. |
| ARCH-001 | Open, unchanged published logging seam/introducer blocker. |
| RSH-001 | Owning C2 remains open / implemented_pending_C2_QA; producer-only independent closure is preserved separately. |
| RBQA-F001 | Distinct correlated direct logging edge remains open. |
| C4-FTQ-001 | Open: production run_with_cleanup cannot publicly expose its owned ephemeral endpoint with real signal and selected instance root in0.1.1. Prior bounded public API assessment remains valid. |

Each owning TTL retains its history and records this scoped disposition.
Windows zero-exit/descendant reap/unlock/restart assertions, genuine canonical
logging delivery and all strict isolated graph guards remain required.

## Validation handoff

Pre-push affected strict all-target/all-feature Clippy and formatting pass.
Focused **post-push** service, daemon and qualified store/cron regressions,
source-equivalence hashes, actual current CI causes and final pushed SHA are
recorded in `/tmp/phase-c-execution/c-2/qa-4/qualified-shutdown-result.json` and
`.md`. Every Cargo command uses `CARGO_INCREMENTAL=0` and the serial shared
cached target `/tmp/phase-c-execution/c-1/restart-target`.
No unchanged full workspace/performance/live-consumer release qualification is
repeated. The parent owns independent QA5 and all finding/merge decisions.
