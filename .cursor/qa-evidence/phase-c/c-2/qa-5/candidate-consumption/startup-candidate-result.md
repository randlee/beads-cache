# C2 startup candidate developer result

Pushed **`b7de8e9879503353879c500edacf12a6832d709d`** to `sprint/c-2-framework-application`.
Focused local validation PASS; independent owning QA is pending. All nine C2
findings plus imported FTQ-011 remain OPEN. C1 QA8 remains FAIL4B2I. No PR merged.

Before `45c9dd13258482ba8af3e5cfeae5ce146b120453`; metadata checkpoint `a87fbe1`;
exact integration `07a1ebcb11085bb6294096139528e040720554c3` merged as `b09c3d0`;
frozen C1 candidate `adbcc88d3dc134ae94403bc2af7a8db76ba921a5` merged as `dcaa8e4`.
Store/sync trees match the immutable candidate exactly (tree identities in JSON).
No live helper edits copied. Four TTL conflicts preserve complete owning C2 and
attributed producer history. The four initial dirty records were the exact
expected QA metadata; no unexpected work existed.

## Actual original probe

Original source/manifest SHA256s remain unchanged. The original45c9dd1 failure
pending902.343541ms remains preserved. Rebuild/replay against consumed C2 source
passes251.517333ms initially and **251.883875ms post-push**, under unchangedT250ms /
original2T500ms. Final post-push total260.927125ms includes rollback and actual
management join; management Ok(true), Application Ok(()), probe process exit0,
waited/reaped,12second watchdog not triggered. The build JSON records actual C2
manifest/package/artifact/freshness paths. Exact post-push binary SHA256:
`ded074af201d34af43982eb36573339e97d678383cf6335aa1eb5ef96a037a31`.
Compiler rustc1.98.1/LLVM22.1.8, host aarch64-apple-darwin; full versions in JSON.

## Caller regression and final verification

Two new startup tests use the existing owned-child fixture. They prove real
normal retained-management contention, observe this Application's startup read,
then actually join before writer release. Post-push ordinary251.034791ms and
zero1.337917ms preserve original500ms and existing100ms zero scheduling limit.
Production callback Some(15), readiness Err(Closing), management remains pending
before rollback then returns Ok(true); probe/management/cron/readiness really join
and cache reopens. Rescue rollback/join precedes assertions; failed timing retains
the original shutdown future. No production caller correction was indicated.
Typed genuine cache/startup/JoinError causes remain unchanged. This is public
Application validation; no stock-daemon HTTP counterexample is claimed.

Post-push **21/21 tests PASS**: service7, daemon unit10, daemon integration4,
including actual Unix signal/lock/readiness cleanup/restart sample and typed-error
retention. Existing endpoint reservation race remains OPEN despite green sample.
Strict affected service/daemon/sync/store all-target/all-feature Clippy and fmt
PASS. Initial new-test too-many-lines lint was fixed by splitting setup helper;
its failed log is retained, no waiver. No broad producer15 or full1171 rerun.

All Cargo commands used CARGO_INCREMENTAL=0 and shared target
`/tmp/phase-c-execution/c-1/restart-target`. **Cargo released to parent immediately
after final build/probe; C1 acknowledged and building. No further Cargo.**

## Bounded exact-head CI snapshot

Current b7de8e9 snapshot: **1 success,4 failure,4 in progress**. Actual failed
composite126397663052 reports paste RUSTSEC-2024-0436 / advisories FAILED.
Python Rust checks on Linux126397663102, macOS126397663069 and Windows126397663048
all report the unchanged forbidden direct sync-to-sc-command graph AssertionError.
Other checks were still running; no final all-platform result is inferred.
Sandbox index-lock/network diagnostics and initial CLI escape-display refusals
are retained; authorized normal-permission retries succeeded. No gate suppressed.

## All findings

- **QA-C2-001**: open: published0.1.1 Windows CtrlC-only handling; prior actual CtrlBreak failure retained, current-head Windows still running at snapshot
- **C2-REQ-003**: open: current CI failures/pending, parent merge, native/wheel actual consumer, independent QA and anchor gates
- **C2-REQ-004**: open / implemented_pending_independent_C2_QA: exact startup candidate consumed, unchanged actual public probe and causal Application regressions pass locally
- **C2-REQ-007**: open: forbidden direct sync-to-sc-command edge remains; approved published logging seam required
- **C2-REQ-009**: open: real Unix lifecycle sample passes; portable actual endpoint/signal/instance-lock qualification incomplete
- **ARCH-001**: open: direct sync-to-sc-command introducer unchanged
- **RSH-001**: open / implemented_pending_independent_C2_QA: producer startup source and actual caller developer proof credited; owning disposition pending
- **RBQA-F001**: open: distinct correlated boundary finding preserved
- **C4-FTQ-001**: open: released ephemeral reservation race unchanged; green Unix sample does not close it
- **FTQ-011**: open: imported producer inventory helper issue; C1 sole writer, helper bytes unchanged, broad known-flaky producer suite not rerun

C2-FTQ-006 remains independently retracted false positive, outside open counts.
Full command outcomes, logs/hashes, compiler/artifact/source identities and limits
are in `startup-candidate-result.json`. The tracked scoped receipt is
`docs/plans/phase-c/evidence/c-2-startup-candidate-consumption.md`.
Developer stopped for parent independent exact-caller QA; no self-closure,
whole-sprint/anchor acceptance or PR merge.
