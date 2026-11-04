# C2 validated phase protocol consumption

Pushed `37b37376329ff9ceb84319ea17d769f84d67fbf9` on `sprint/c-2-framework-application` (PR109).
Ordinary integration merge `c205d90944499d19a05d71f8646fb43bec227ab6`, then immutable
producer merge `7d2383104cca83f4c799c3dcc86c35386130af17` consume exact integration
`c0b251da8c491cb32acf49950b87557bd5f86e8e` and producer
`8c1aefafd7fd269c82c7974e610e848816d026d8`. Later C1 inventory source is not consumed.

Store/sync/protocol/helpers match producer exactly. Production prefix and C2
service/daemon/manifests/lock/pin/CI are unchanged. Every original and imported
finding history and the unchanged C2-FTQ-006 retraction are preserved.

Local validation: store8/8, default health2/2, service7/7, fmt/private fmt and
strict four-crate all-target/all-feature plus default-library Clippy PASS.
No unexpected test/lint failures or driver fallback occurred. Focused postpush
direct planted negative exits101 in5.146266708s, classification
`worker_returned_required_marker_missing`, original qualification assertion
retained and `root_joined_and_cleaned=true`. Expected failure is classifier
control evidence, not historical reproduction or closure.

Cargo is **RELEASED**, active0, immediately reported to parent. The initial
pre-Cargo result is preserved separately. Working tree is clean.

Exact head CI run `41792355909` bounded snapshot:

- Windows: [126448419419](https://github.com/randlee/beads-cache/actions/runs/41792355909/job/126448419419), in progress.
- macOS: [126448419348](https://github.com/randlee/beads-cache/actions/runs/41792355909/job/126448419348), in progress.
- Boundary job `126448419184`: observed FAILURE; raw cause not inferred.
- Python run `41792355950`: launched at exact head; no full acceptance claimed.

Parent owns remaining remote monitoring and independent C2 QA. No CIwatch,
full1171/performance/lifecycle18 rerun, SDK/pin/guard/signal/endpoint/publisher/
auth/token work, source/helper edits, agents, PRmerge or self-QA closure.

C1 QA10 remains **FAIL3B3I29/34**. Historical RQA9 and actual C1-only macOS
RQA10 stay OPEN/unknown cause; no C2 platform failure inferred. Merge readiness:
**NOT READY**. Existing composition/endpoint/signal/advisory/consumer and parent
acceptance gaps remain preserved.

- QA-C2-001: OPEN: published CtrlC-only runtime and separate actual C2 STOP 3221225786 remain; no Windows stop fix.
- C2-REQ-003: OPEN: current CI, consumer, parent merges, independent QA and anchor acceptance remain incomplete; advisory retained.
- C2-REQ-004: OPEN owning C2 disposition: existing qualified startup/post-start correction retained; this diagnostic promotion adds no broad acceptance.
- C2-REQ-007: OPEN: actual direct sync -> sc-command edge still requires approved published logging receiver seam.
- C2-REQ-009: OPEN: portable endpoint/signal/instance-lock qualification remains incomplete.
- ARCH-001: OPEN: published canonical logging seam/direct introducing edge unchanged; strict graph remains rejecting.
- RSH-001: OPEN owning C2 disposition; independently resolved supported C1 and paired public C2 startup scope preserved, not reopened.
- RBQA-F001: OPEN: distinct carried logging dependency finding retained.
- C4-FTQ-001: OPEN: released ephemeral reservation race retained; public endpoint seam required.
- FTQ-011: OPEN owning C2 disposition; exact C1 QA9 scoped helper closure and QA10 preservation retained; no removed 50ms recurrence claim.
- RQA9-001: OPEN: historical actual C2 Windows busy-negative cause UNKNOWN. Planted classifier control is not historical reproduction or correction.
- RQA10-001: OPEN imported C1-only macOS inventory timeout at producer 8c1aefa; precise phase/cause UNKNOWN. No C2 platform occurrence inferred.
