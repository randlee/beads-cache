# C2 terminal Windows evidence analysis

Exact PR head `37b37376329ff9ceb84319ea17d769f84d67fbf9`, CI run41792355909,
[Windows job126448419419](https://github.com/randlee/beads-cache/actions/runs/41792355909/job/126448419419)
completed FAILURE at2026-11-04T07:40:40Z (parent terminal metadata).
Raw checkout is synthetic PR merge `046ace2d262b89a705a074411fcfd7d9b225ae3a`,
merging that head into `c0b251da8c491cb32acf49950b87557bd5f86e8e`
(raw111-114). Synthetic object is unavailable locally; hosted blob equality is not claimed.

The actual failed step is **Daemon process acceptance (FakeBd only)**, not Test.
Traceback raw3043-3046 reaches the first stop at `consumer_checks.py:445` and
fails `stop()`'s zero-exit assertion at394. Windows stop sends CTRL_BREAK_EVENT
at391. Actual tuple is `(3221225786, '', '')` (raw3065-3067), hexadecimal
`0xC000013A` / `STATUS_CONTROL_C_EXIT`; step exits1 (raw3068). Both builds completed.
This is the first ordinary daemon stop after recovery, before restart or blocked-hook
stop cases. Their later acceptance cannot be credited.

The actual stack, signal call and exit support correlation with existing
**QA-C2-001 OPEN**, with C2-REQ-003/009 acceptance still incomplete. Cached published
sc-runtime0.1.1 source was independently read: Windows Signals/install/wait contain
only CtrlC, not CtrlBreak (`shutdown.rs:53-64,94-102,126-135`); actual build uses
sc-runtime0.1.1 (raw3032). This supports the existing signal-contract gap; no
OS signal trace or novel store/cancellation cause is asserted. No new finding is
supported. No successful graceful joins/reap/ownership release/restart is established.
No WinError32 appears in this current raw occurrence.

The Test step succeeds. Store reports **37 passed,0 failed** in16.56s
(raw1908-1949), including new cleanup-error, observer-unwind, planted-classifier and
stalled-family controls. `RQA9_PROTOCOL` records: **0**. The ordinary command has
no `--nocapture`; successful test output is captured. Passing names/results do not
provide detailed phase receipts or localize historical RQA9. RQA9 remains
OPEN/unknown. Earlier C1 macOS RQA10 stays OPEN/unknown and is separate from this
actual Windows daemon case. All12 IDs, prior scoped closures and retraction remain tracked.

Next action: parent correlates the exact failure with QA-C2-001 and preserves the
original assertions. A future correction requires the approved published Windows
graceful-stop seam and independent exact-source stop/join/reap/ownership/restart
qualification. No local handler, alternate signal, abnormal-exit acceptance, skip,
relaxed budget or green rerun is supported by this diagnosis. Return and STOP.

Raw log preserved unchanged:266161bytes,2322 ESC bytes, SHA256
`818a94d57a41b8298435e58498f70304ccfa3a8f746f387c6aaa1846e84d10a8`.
Only this analysis pair was written; no source/triage edits, Cargo, rerun, agents,
network polling or merge. Working tree remains clean; active commands0.
