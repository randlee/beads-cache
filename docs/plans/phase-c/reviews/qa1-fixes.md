# QA1 developer corrections

Input: [qa1-manager.md](qa1-manager.md), actual QA1 FAIL at
`0beef1a8e213b9b55af85d55ca014e85fde52782` (seven Blocking, two Important,
one Minor), and the six exact reviewer outputs linked by that report.
[qa1-screen.md](qa1-screen.md) retained all ten findings; none was rejected.
This correction is not QA approval or an implementation dispatch decision.

| Finding | Correction in existing documents |
|---|---|
| REQ-QA-001 | c-1's authoritative constrained_by list now includes every enumerated CLI/Python/store/sync/CRON ID and the five repo constraints. Its prototype-disposition section preserves semantics, separates obsolete API identity from behavior and requires explicit transition approval before recording historical ownership. |
| REQ-QA-002 | c-1 owns the CLI requirements document and expressly plans the narrow REQ-CLI-0001 git-source/tag amendment. Proposed REQ1002 and ADR0016 name it; original Approved CLI text is untouched. |
| REQ-QA-003 | c-2 lists NFR-BDC-0007; the single shared composition remains mandatory. |
| REQ-QA-004 | c-4 lists REQ-PY-0010/0014 and NFR-BDC-0005/0006. Existing wheel/version/testbed constraints remain intact. |
| ARCH-001 | c-1's binding adrs list includes all enumerated active governing IDs. Superseded sync/Python decisions are not revived. |
| ARCH-002 | c-2 and c-4 binding lists include their missing outage/filter/store/cache and product-independence decisions. |
| ARCH-003 | Every sprint moves proposed ADR0016 into proposed_adrs. Phase/sprint/root-proposal text requires an explicit authorized project-owner decision before implementation dispatch or applying requirement amendments; review/CI/PR merge alone cannot approve them. |
| RBP-F001 | c-2 fixes daemon stderr/exit and typed code/cause/recovery outcomes for config, endpoint, listener policy, instance conflict and bind failures, reusing pinned template error formatting and logger-finish precedence; process acceptance checks those outcomes without changing bdc. |
| RSH-001 | c-1 supplies an additive store-owned synchronous health-read deadline scope with bounded connection acquisition, SQLite execution/decoding and restoration. c-2 admits one probe with no waiter queue to one owned dedicated blocking worker, retains the permit until actual completion after request cancellation, and joins it during cleanup before instance release. Acceptance includes contention, canceled/saturated probes and real cleanup completion; dropping an async future is not cancellation proof. |
| CQA-F001 | Removed rejected candidate-packet history from the phase plan while retaining inherited bounds and proposed daemon status. |

The readiness seam is inside c-1's existing store/sync/Cargo fence and consumed
by c-2's existing service/daemon fence; no new sprint or dependency edge was added.
Critical path three, width two and four sprints remain unchanged. Existing cache
and schedule storage, local CLI/Python contracts and backend/deadline bounds remain
in force. No anchor record, scope, question, fixture or override was edited.

Validation: `git diff --check`; consumer-boundary lint on modified tracked docs;
automated checks of all ten finding-specific corrections, exact metadata additions,
wave-one path independence, unchanged approved root-document prefixes, unchanged
active CLI/API/CRON requirement files and anchor files. The new notes file remains
untracked until the coordinator checkpoints it and is outside the lint's current
tracked-file scan. This pass implements planning corrections only; it did not
compile or test the proposed future runtime/store implementation. No commit/push
was performed. Root owns the checkpoint, anchor requalification and re-QA.
