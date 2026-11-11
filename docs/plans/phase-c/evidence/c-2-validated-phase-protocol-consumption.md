# C2 validated private phase protocol consumption

This candidate consumes the independently reviewed C1 private diagnostic protocol
into the existing C2 application branch. It supports later exact-context evidence
collection for historical RQA9; it does not establish that historical cause,
close an owning finding, or qualify the whole sprint.

- Clean starting C2: `e86d9da92a8c8c9b787c780e29bc54e417680d71`.
- Exact integration: `c0b251da8c491cb32acf49950b87557bd5f86e8e`.
- Ordinary integration merge: `c205d90944499d19a05d71f8646fb43bec227ab6`.
- Immutable reviewed C1 producer: `8c1aefafd7fd269c82c7974e610e848816d026d8`.
- Ordinary producer merge: `7d2383104cca83f4c799c3dcc86c35386130af17`.

Only RQA9 triage conflicted in each merge. The complete owning records remain,
and every unique incoming integration/producer line is appended. All previous
triage histories, the C2-FTQ-006 retraction, and scoped RSH/FTQ closure records
remain available. No live C1 inventory edits are consumed.

The whole store tree is `5479d866a253ba3d4becb96112ad3118b7959e1c` and sync tree
is `060ff9040235d995cef976caf3cbc5f619597de3`, exactly the immutable producer.
The schedule production prefix retains SHA256
`49438f00b7310fb4d385355dbc887ab3c353264eed25e05df9f824e5872da05a`.
`FixtureChild`, private protocol, session fixture and cron_hurry match producer
blobs exactly. C2 manifests, lock, runtime pin and CI workflow remain identical
to the starting C2 candidate. No store/sync production, test or helper edits are
made locally.

The private per-child main/worker checkpoints distinguish observed worker return
from delivery of the original stderr marker. The planted post-return gap retains
failed qualification, real contention, 30ms cancellation, the original 500ms
observation, and five-second owner lifetime. Actual root wait/cleanup, observer
error/unwind and false incomplete-receipt semantics are retained. Missing receipts
remain unresolved observations; diagnostic success is not historical cause proof.

C1 QA10 independently credited the changed diagnostic scope but remains **FAIL,
3 Blocking + 3 Important, 29/34**. Historical Windows RQA9 and actual C1 macOS
RQA10 remain OPEN with unknown cause. RQA10's error cannot identify the precise
inventory call; it establishes no C2 platform failure, orphaning, 50ms recurrence,
observer causality or production fault. Separate C2 daemon STOP 3221225786 is not
this store occurrence.

| Finding | Preserved disposition |
|---|---|
| QA-C2-001 | OPEN: published CtrlC-only runtime and separate actual C2 STOP 3221225786 remain; no Windows stop fix. |
| C2-REQ-003 | OPEN: current CI, consumer, parent merges, independent QA and anchor acceptance remain incomplete; advisory retained. |
| C2-REQ-004 | OPEN owning C2 disposition: existing qualified startup/post-start correction retained; this diagnostic promotion adds no broad acceptance. |
| C2-REQ-007 | OPEN: actual direct sync -> sc-command edge still requires approved published logging receiver seam. |
| C2-REQ-009 | OPEN: portable endpoint/signal/instance-lock qualification remains incomplete. |
| ARCH-001 | OPEN: published canonical logging seam/direct introducing edge unchanged; strict graph remains rejecting. |
| RSH-001 | OPEN owning C2 disposition; independently resolved supported C1 and paired public C2 startup scope preserved, not reopened. |
| RBQA-F001 | OPEN: distinct carried logging dependency finding retained. |
| C4-FTQ-001 | OPEN: released ephemeral reservation race retained; public endpoint seam required. |
| FTQ-011 | OPEN owning C2 disposition; exact C1 QA9 scoped helper closure and QA10 preservation retained; no removed 50ms recurrence claim. |
| RQA9-001 | OPEN: historical actual C2 Windows busy-negative cause UNKNOWN. Planted classifier control is not historical reproduction or correction. |
| RQA10-001 | OPEN imported C1-only macOS inventory timeout at producer 8c1aefa; precise phase/cause UNKNOWN. No C2 platform occurrence inferred. |

The first phase performs merges, source inspection and metadata only. Cargo and
push wait for an explicit sole local slot grant. After the grant, the assigned
scope is store controls, default health, seven service callers, affected strict
Clippy/fmt, concrete push, a focused post-push source proof and bounded exact-head
Windows/macOS job handles. Results are recorded under the execution receipt
`qa-5/validated-phase-protocol-consumption-result.json` and matching Markdown.
Remote monitoring and independent C2 QA remain parent-owned; no watch-to-green,
ordinary stress, repeated full1171/performance/lifecycle18 run or PR merge follows.

## Scoped local validation after exclusive slot grant

At the immutable producer composition, store controls pass 8/8, default-feature
health mutex/owner controls pass 2/2, and actual service caller tests pass 7/7
(with nested owned-child results counted only once). Workspace formatting and
private protocol formatting pass. Strict Clippy passes for store, sync, service
and daemon with all targets/all features and with default-feature libraries.
`CARGO_INCREMENTAL=0` and the parent-assigned restart target were used. No source
was changed and no ordinary full-suite/performance/lifecycle rerun was performed.

Raw store logs preserve actual root wait before any driver fallback for observer
error/unwind; incomplete cleanup retains a false receipt and refuses another
attempt. All commands exited zero without a driver fallback. These results
qualify this scoped diagnostic composition only. Historical RQA9 and actual
C1-only macOS RQA10 stay OPEN with unknown cause. Exact push and focused post-push
proof/native job handles belong to the external execution receipt.
