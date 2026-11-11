# C2 public Application startup candidate consumption

C2 remains in progress. This records developer evidence pending independent
owning review; it closes no finding, sprint, anchor, PR or release gate.
C1 QA8 remains FAIL4B2I. Its startup producer proof is credited separately from
FTQ-011 and unchanged required graph/acceptance blockers.

## Sources

Original C2 source: `45c9dd13258482ba8af3e5cfeae5ce146b120453`.
The four expected parent-owned metadata records were checkpointed in `a87fbe1`
before merges: C2-REQ-004/RSH-001 startup evidence, retracted C2-FTQ-006 and
open imported FTQ-011. No unexpected work was present.
Exact integration `07a1ebcb11085bb6294096139528e040720554c3` was merged as
`b09c3d08faa0994d8dcd9a4a7f488202361c4698`, then frozen C1 source
`adbcc88d3dc134ae94403bc2af7a8db76ba921a5` as
`dcaa8e4d15f231a46babe7d70da384a3e626b876`.
Both store/sync trees match that immutable producer source exactly.
No live C1 helper or integration worktree edits were copied.

Conflicts were limited to ARCH-001, RBQA-F001, RSH-001 and FTQ-010 TTL records.
Each preserves the owning C2 record verbatim and appends the complete conflicting
C1 record as attributed producer history. FTQ-010 retains prior independent
closure; FTQ-011 is the distinct current open producer-helper issue.

## Actual public counterexample and regression

The original QA5 probe source and manifest remain unchanged, with source SHA256
`604ce3bf268209bdf9f3a6f9d25023d7cacf85fa0624dc29c6c8c02480ad07c6`.
It retains Composed::schedule_store(), starts ordinary set_enabled behind an
external BEGIN IMMEDIATE writer, proves shared-mutex contention using a worker
view, then starts and shuts down the actual Application with T250ms.
Rebuilt against consumed C2 source, it returns Ok in251.517333ms before writer
release (original2T500ms), then management returns Ok(true) after rollback.
Actual management/probe threads join; Application joins cron/readiness; the
probe process exits0 and is waited/reaped under its unchanged12second watchdog.
Historical45c9dd1 failure at902.343541ms remains retained as the original negative.

Two new service regressions exercise the same public composition, additionally
observing that this Application's actual startup read is waiting before stop.
They use the existing owned10second child fixture and original ordinary500ms
bound; zero-timeout uses only the already established100ms scheduling tolerance.
They verify production expiry through sc_bd_cli::shutdown::requested(), closing
readiness, management still pending before release, management Ok(true) after
rollback, all owned joins and cache reopening. Failure observations release the
writer and keep the same shutdown future for its actual join; a rescue guard
also rolls back/joins ordinary management during unwind. The store test-view
probe is deadline-canceled and actually joined before Application construction.
No backend/helper child is created by the scenario. No private/new public
factory, cancellation token, listener, signal, timeout padding or SDK workaround
is introduced. sc-cron is added only as an existing-version service test dependency.

No Application production correction was indicated: shutdown already closes
readiness admission before stop and retains the cron task/readiness thread.
A canceled startup read need not claim a successful drain; actual idle/no-fault
cron completion remains the existing Application success condition. Genuine
JoinError and typed cache/startup causes remain unchanged.
This is supported public Application validation, not stock-daemon HTTP evidence.

## All findings remain tracked

| Finding | Disposition |
|---|---|
| QA-C2-001 | Open: runtime Windows CtrlBreak compatibility still required. |
| C2-REQ-003 | Open: exact CI/native/wheel consumer/QA/anchors/parent merge gates remain. Paste advisory remains unsuppressed. |
| C2-REQ-004 | Open, implemented pending independent owning review of consumed startup source and caller proof. |
| C2-REQ-007 | Open: forbidden direct sync-to-sc-command edge still requires approved published logging seam. |
| C2-REQ-009 | Open: portable actual endpoint/signal/instance-lock proof remains incomplete. |
| ARCH-001 | Open: unchanged direct logging introducer. |
| RSH-001 | Open, implemented pending owning review; prior post-start credit preserved. |
| RBQA-F001 | Open: distinct correlated boundary ID retained. |
| C4-FTQ-001 | Open: endpoint reservation race unchanged. |
| FTQ-011 | Open: C1 owns the imported intermittent ps inventory helper correction. No local helper fix or broad flaky suite rerun. |

C2-FTQ-006 remains independently retracted as a false positive; it is not counted
as an open finding. Every owning TTL retains its previous history.

## Validation

Commands use CARGO_INCREMENTAL=0 and the sole shared Cargo target.
Focused service and daemon checks, strict all-target/all-feature Clippy for
affected service/daemon/sync/store crates, formatting, source equivalence,
unchanged original public probe and post-push proof are recorded with exact
commands, hashes, outcomes and CI snapshots in
`/tmp/phase-c-execution/c-2/qa-5/startup-candidate-result.json` and `.md`.
Following memory b-mr7cp6x0-ipdnhs, local macOS checks are not represented as
Linux/Windows or final full-source qualification. Known upstream, graph, advisory,
consumer and platform limitations remain explicit. Parent owns independent QA.
