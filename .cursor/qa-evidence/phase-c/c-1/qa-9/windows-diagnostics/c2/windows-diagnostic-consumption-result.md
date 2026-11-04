C2 Windows diagnostic consumption pushed at `e86d9da92a8c8c9b787c780e29bc54e417680d71`. This is diagnosis only; RQA9-001 cause remains UNKNOWN.

Baseline `b7de8e9879503353879c500edacf12a6832d709d` -> ordinary integration merge `c90a190c5cfe5af40be09bd6e1038abc03f63dfe` from `63ebc1fa032f7ccaa2b5b0b97a8f9621599060d0` -> ordinary diagnostic merge `e86d9da92a8c8c9b787c780e29bc54e417680d71` from immutable `bcfcae7021522454e85be964661b9525bb15e58c`. Store/sync trees exactly equal immutable producer; all production source prefixes equal original baseline. Worktree clean; original failure log preserved byte-for-byte.

Local postpush scoped checks passed: store shutdown5/5, service7/7, fmt, four-crate strict Clippy all-target/all-feature and default-lib. `CARGO_INCREMENTAL=0`, target `/tmp/phase-c-execution/c-1/restart-target`. Cargo RELEASED immediately after checks; zero active owned build/test commands. Compiler/features and exact selected test binaries are in `windowsdiag-source-identities.json` / compiler receipt.

Exact C2 Windows run `41780092066`, job `126409418610`: https://github.com/randlee/beads-cache/actions/runs/41780092066/job/126409418610. Head matches pushed commit. Bounded snapshot: `in_progress`, Clippy running and Test pending; no Windows causal output available yet. Parent monitors from this handle. No remote wait-to-green.

Original Windows source `b7de8e9879503353879c500edacf12a6832d709d`, run41776396362/job126397662727 failed store33/1 exit101. Missing receipt does not prove operation never began or returned. Darwin/C1/later green is not a demonstrated fix. Original30ms cancellation,500ms observation and complete5s owner lifetime preserved.

All11 finding states retained:

- QA-C2-001: open — Pinned runtime CtrlC-only behavior; original CTRL_BREAK graceful-stop failure retained.
- C2-REQ-003: open — Overall CI, parent merges, consumer/platform, independent QA and anchor acceptance pending.
- C2-REQ-004: open — Qualified startup correction retained; owning C2 disposition pending; no broad acceptance.
- C2-REQ-007: open — Direct sync-to-sc-command edge remains; published logging seam required.
- C2-REQ-009: open — Portable endpoint/signal/instance-lock qualification incomplete.
- ARCH-001: open — Published logging seam and direct introducing edge unchanged.
- RSH-001: open — Exact C1 and paired public C2 startup closure evidence preserved; overall owning C2 acceptance pending.
- RBQA-F001: open — Distinct carried boundary finding retained.
- C4-FTQ-001: open — Ephemeral reservation release race unchanged; approved public seam required.
- FTQ-011: open — Independently verified exact C1 helper correction consumed byte-identically; owning C2 qualification pending, no self-closure.
- RQA9-001: open — Original actual C2 Windows failure retained; cause UNKNOWN; diagnostic receipt collection only.

C2-FTQ-006 remains retracted unchanged. Both merge conflict resolutions preserve complete owning C2 histories plus incoming C1 scoped evidence; startup qualification remains credited. Full C1 remains QA9 FAIL3B2I29/34 and full C2 owning QA/anchors/CI/merge acceptance pending. No self-closure, child agents, source workaround or PR merge.

All commands, identities, raw local negative-path receipts, original Windows failure, bounded hosted snapshots, diagnostics and hashes are in the matching JSON. Stopped for parent diagnosis/review.
