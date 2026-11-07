# Supported macOS runner CI assessment

Frozen C2 source `7fb224623280dae9cb056915484cb698ddf6bdf0`; Python run41794713901.
Two required pytest jobs still request retired `macos-14`, with no runner or steps:
[3.11job126457310615](https://github.com/randlee/beads-cache/actions/runs/41794713901/job/126457310615)
and [3.9job126457310735](https://github.com/randlee/beads-cache/actions/runs/41794713901/job/126457310735).
Queue cause is not proven solely by this snapshot: othermacos14 jobs executed.

| Job | Case | Status/conclusion | Runner ID | Steps |
|---|---|---|---|---|
| 126455973352 | clippy and test (macos-14) | completed/failure | 1000263238 | 13 |
| 126455973399 | wheel (macos-14) | completed/success | 1000263236 | 12 |
| 126457310615 | pytest (macos-14, python 3.11) | queued/None | 0 | 0 |
| 126457310716 | pytest (macos-14, python 3.13) | completed/success | 1000263243 | 10 |
| 126457310735 | pytest (macos-14, python 3.9) | queued/None | 0 | 0 |

REQ-PY-0010 (`requirements.md178-188`), Python architecture176-182 and C4
artifact matrix107-110 require **macOSarm64** wheels/pytest, Linuxx86_64 wheels,
Python3.9/3.11/3.13 and Rust tests on all3 OSes. Separate native binary archives
include Intel; that is not an implicit PythonIntel wheel requirement.

Direct raw evidence confirms intended/current architecture: successful wheel
job126455973399 uses `Image:macos-14-arm64` (raw16),
`1.98.1-aarch64-apple-darwin` (raw118/121), arm64 Python (raw218), and builds
`bd_cache_rs-0.2.0-cp39-abi3-macosx_11_0_arm64.whl` (raw249).
PythonRustmacOS job126455973352 also uses macos14arm64/aarch64/arm64Python.
Thus a blanket Intel replacement would change the actual and approved scope.
The official current README's retiredmacos14x64 row does not override these actual receipts.

[GitHub's retirement announcement](https://github.blog/changelog/2026-10-01-github-actions-macos-14-runner-image-retirement/)
and [official runner images](https://github.com/actions/runner-images#available-images)
confirm GitHub ActionsmacOS14 retirement November2,2026. The latter lists
`macos-15` asarm64 and `macos-latest`/`macos-26` asarm64; Intel alternatives
`macos-15-intel`/`macos-26-intel` exist for separately intendedx64 work.

Minimal proposal for parent triage: replace all3 `macos-14` matrix entries at
python.yml40/75/117 consistently with **macos-15**. This preserves actual/intended
arm64 and avoids implicit latestOS movement; macos-latest is an equivalentarm64
option if parent chooses nativeCI's latestpolicy. Keep all11 jobs, Pythonversions,
commands, guards and artifact lookup wiring. Upload/download use the same matrix
label, so all3 replacements must stay consistent. No other correction is proposed.

Completed PythonRust jobs fail the **Isolated Python/API dependency graph** step;
macOS raw263 explicitly rejects directsync->sc-command. That failure is separate
from retirement and must not be waived. Native CI41794713892 usesmacos-latest;
its macOS job126455975561 succeeds, while nativeWindows fails daemon acceptance.
Neither platform conclusion establishes a historical cause or closes an existing finding.

No source/triage edits, Cargo, rerun, commit/push/merge, waiver, SDK/publisher work
or nested agents. All12 histories untouched. Parent must triage any new runner
issue before dispatch. Working tree clean; activecommands0. Raw snapshots and
loghashes are in the matching JSON assessment.
