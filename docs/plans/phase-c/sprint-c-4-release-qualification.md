---
phase: c
status: in_progress
recommended_agent: rust-developer
recommended_model: gpt-6.1-sol
id: c-4
closure_type: integration
target_boundary: artifact distribution
track: distribution
wave: 3
branch: sprint/c-4-release-qualification
worktree: sprint/c-4-release-qualification
must_follow:
- c-2
parallel_safe: []
requirements:
  owns:
  - REQ-BDC-1005
  constrained_by:
  - REQ-BDC-0813
  - REQ-BDC-0812
  - REQ-BDC-0805
  - REQ-BDC-0910
  - REQ-BDCLI-0008
  - NFR-BDC-0007
  - REQ-PY-0010
  - REQ-PY-0014
  - NFR-BDC-0005
  - NFR-BDC-0006
adrs:
- ADR-BDC-0006
- ADR-PY-0004
- ADR-PY-0006
- ADR-CLI-0001
- ADR-BDC-0010
- ADR-BDC-0012
- ADR-BDC-0016
owned_paths:
- release/**
- install.py
- README.sc-publish.md
- .github/workflows/release*.yml
- .github/workflows/prerelease-archive.yml
- .github/workflows/crates-publish.yml
- .github/workflows/pypi-publish.yml
- .github/workflows/homebrew-publish.yml
- .github/workflows/scoop-publish.yml
- .github/workflows/winget-publish.yml
- .github/workflows/npm-publish.yml
- .github/scripts/**
- .github/actions/**
- scripts/check_release_candidate.py
- scripts/test_release_candidate.py
- README.md
- docs/development/release.md
- docs/getting-started.md
- docs/plans/phase-c/evidence/c-4-consumer-qualification.md
- docs/plans/phase-c/evidence/c-4-consumer-qualification.json
inherited_bounds:
- ANC-001
---

# Sprint c-4 — Installable candidate artifacts and truthful release gates

## Outcome

The final application is distributed as usable candidate artifacts and the
stable-release decision is backed by preflight. The cache no longer attempts to
publish sc-runtime's sc-cron crate under its former local ownership.

## Recorded owner approval

The project owner's explicit 2026-11-02 instruction to execute all approved
Phase C sprints satisfies this sprint's dispatch prerequisite. See the phase
plan's [recorded approval](phase-c-plan.md#implementation-dispatch-gate) and
ADR-BDC-0016 for provenance and the unchanged bounds. ADR-BDC-0016 is now in the
binding `adrs` list. c-1 applies the approved CLI/API/CRON amendments in its owned
files; no other existing rule is waived. Independent implementation QA, applicable
anchor gates and green CI remain required before merge.

## Deliverables

1. REQ-BDC-1005, REQ-BDC-0813: update the consumer release input for published
   sc-beads/sc-bd-cli, the Python wheel and binary archives containing bd-cache,
   bdc and the c-2 daemon. Remove local sc-cron publication. Pin a qualified
   sc-publish revision and regenerate kit files only through its installer;
   preserve c-2 CI and application tooling.
2. REQ-BDC-1005: use the kit's existing consumer preflight extension for manifest,
   archive membership/checksums and installed-binary/wheel smoke checks. Include
   the daemon startup/shutdown procedure and cached CLI/Python access. A candidate
   built from another commit does not satisfy this sprint.
3. REQ-BDC-1005: execute complete preflight and build/install candidate artifacts
   on supported platforms. Record an explicit post-merge follow-up to cut a prerelease from develop/main
   after the phase head is qualified; hosted publication is not this sprint's
   deliverable or completion criterion. Record build-only candidates separately
   from published prereleases. Stable publication waits for a verified #135 fix and
   actual credentials/settings; channels with missing setup stay explicitly
   pending, not complete. Never publish immutable stable assets just to test them.

## Acceptance criteria

| Root | Positive observable | Planted negative |
|---|---|---|
| req:REQ-BDC-1005 | each supported target installs its declared binary/wheel set from the same commit/version; original CLI and new daemon smokes pass | omit daemon or mix wheel/CLI commits; installed-artifact check fails |
| req:REQ-BDC-0813 | checksums cover exactly intended assets; no release configuration is uploaded; local sc-cron is absent from publish manifest | stage consumer release config as an asset or restore local sc-cron publication; check fails |
| req:REQ-BDC-1005 | stable preflight refuses an unfixed #135 kit and reports missing channel setup; supported prerelease path stays usable | use the old staging behavior or ignore missing setup; negative preflight fixture fails |
| req:REQ-BDC-1005 | procedure rerun from installed artifacts produces the consumer outcomes c-2 established | use only build-tree binaries or an uninstalled source import; smoke detects it |

## Artifact matrix

Binary archives (all three executables): Linux x86_64, macOS arm64, macOS x86_64,
Windows x86_64. Python abi3 wheels: Linux x86_64 and macOS arm64 only, exercised
with Python 3.9/3.11/3.13 as in current CI. No Windows or macOS-x86_64 wheel is
added implicitly. Match version/commit across binaries and wheels where both ship.

## Required validation

Unit tests here exercise template/packaging helpers, not sync jobs, hooks, ingest
or CLI behavior. Installed-artifact smoke checks remain integration procedures.

Canonical installer dry-run/manifest validation; kit script tests affected by the
qualified pin; release-candidate archive/wheel smoke and checksum tests; full
Release Preflight on the exact final commit. Run channel dry-runs where setup is
available and report each outcome. Do not substitute green PR CI for release
preflight or synthetic tests for actual installed-artifact behavior.

## Dependencies and closure

A c-2 development push triggers merge-forward into c-4 before every child
development/fix round; do not wait for c-2 QA to begin that work. The c-2 PR must
merge before c-4's PR can complete. This relation consumes the real application
and consumer procedure below, not merely a shared release/version baseline.

c-2 produces the real bd-cache-daemon target and consumer procedure, so this sprint
cannot close against the earlier CLI-only release baseline. Candidate archives may
be local/CI artifacts while external stable gates remain blocked. A hosted
prerelease is deferred until the phase has merged to an allowed release branch;
the phase report must name that deferred action and may not claim it happened.
Stable/channel publication is not required for phase closure. Credentials and
first WinGet submission are owner setup, not implementation deliverables to
silently mark complete. Upstream kit changes stay upstream; this sprint does not
hand-edit vendored workflows to bypass them.

## Installer ownership handoff

On 2026-11-02 the phase orchestrator authorized the narrow C2-to-C4 handoff
of `docs/getting-started.md` solely for canonical sc-publish installer output.
The C2-added optional-daemon paragraph is preserved faithfully in the
consumer-owned root README with its link to `docs/development/runtime.md`;
the runtime operation and external consumer procedure remain there unchanged.
The kit-owned document is never hand-edited.

## Development qualification state

The consumer input now publishes only sc-beads/sc-bd-cli and declares all three
executables for archives, prerelease selectors and Homebrew/Scoop installation.
The canonical immutable kit candidate is `5fd4d0db1b7cbb4ebf9316588047391a8e4be496`;
repeat installer dry-run and manifest validation pass. Kit regression tests pass
(347 passed, 13 skipped, 52 subtests); repository scripts pass (108 tests, one
explicit real-template smoke skipped). Negative candidate fixtures reject an
omitted daemon, mixed source receipt, corrupt assets, configuration staged as
an asset, missing checksums, old kit/staging and restored local sc-cron.

`Release artifact qualification` is a build-only workflow with four binary
platforms and six installed abi3/Python combinations. Full source receipts,
checksums and installed results are attached to its actual run. No supported
platform completion is claimed until those jobs execute successfully at the
reviewed final SHA. The existing consumer preflight extension admits only exact
SHA successful complete matrix evidence with unexpired archives/receipts.

Full Release Preflight and channel outcomes remain pending actual execution.
The upstream runtime0.1.1 paste advisory stays unsuppressed; owner steering
explicitly defers its workaround/update to runtime0.2.0. Issue135 source staging
regressions are tested, but its open issue and external immutable/channel setup
are not silently marked resolved. C2 merge and its real consumer procedure remain
closure prerequisites. This sprint remains in progress until actual qualification
and independent QA satisfy its acceptance criteria.

Post-merge follow-up: qualify the merged Phase C head on develop/main, then obtain
separate authorization to cut a hosted prerelease and exercise installed release
and channel legs. No release/tag/registry/channel publication has been performed.

## First hosted qualification evidence

At source `6a98b8c17bce85f68fb2b718e9200d749d363943`, the actual build-only
[qualification run 41567572170](https://github.com/randlee/beads-cache/actions/runs/41567572170)
passed installed archive acceptance on Linux x86_64, macOS arm64 and macOS
x86_64 (including the declared wheel corpus on the first two). Windows built
all three production executables but failed installed acceptance because the
Python procedure decoded native UTF-8 diagnostics using the Windows locale.
The C4 caller now selects `-X utf8 -I` explicitly; an actual subprocess fixture
covers decoding the native em-dash diagnostic. The six downstream Python matrix
jobs were skipped after the failed Windows prerequisite, so this run is not a
complete matrix PASS. Qualification must rerun at the fix head.

The full read-only
[Release Preflight run 41567884775](https://github.com/randlee/beads-cache/actions/runs/41567884775)
completed its summary and denied release with exactly these failed checks:

- `github-environments`: the declared `crates-io` environment is absent.
- `consumer-preflight`: the anonymous public GitHub API returned HTTP403 rate limit exceeded.
- `release-candidate-provenance`: `release-candidate-v0.2.0` does not exist;
  creating that publication-provenance tag is deliberately deferred.

Repository secret presence and credential liveness, immutable-release admission,
registry-state, manifest, publish order, version lockstep, dependency-aware Cargo
package checks and release permissions passed in that run. Protected Python
environment-secret metadata was inaccessible to the workflow token and reported
informational; this is not proof of configured secret validity or publication.
The paste advisory is a separate unsuppressed ordinary CI gate, not one of this
Release Preflight run's three final failures. No tag, release or channel write
was performed to alter any outcome.

## QA1 current-source preparation

The resumed development round consumes the reviewed, committed C2 parent
`63cb429eb1a65ab25a17923fded8de5ecf390a5a`, including native/application corrections
and C2's own byte-capture/strict UTF-8 decoding. Parent work that was still
uncommitted is excluded. The C4 canonical kit, release inputs and binary/wheel
matrix remain unchanged. C4's own command helper now makes one termination
request per process tree, preserves an initial termination-permission failure,
and closes pipes even if final cleanup fails. Its independent fixtures cover
single-request timeout cleanup and both initial/final permission failures.
Installed version probes parse exact product versions instead of accepting a
version prefix. These corrections affect only C4's owned helper/tests; they do
not close C3-owned CI-C4-001 or alter the shared template helper.

A fresh current-source archive/wheel build and isolated installed smoke are
required after the final source commit. The parent receives a new scratch-only
qualification input manifest whose executable files come from the validated
archive, whose wheel hash matches that candidate receipt, and whose external
modules and approved bounded driver are hash-verified immutable inputs. No
private consumer identifiers or code are added to this repository. The parent
alone performs the authorized testbed transport and returns its actual result;
source6232180's historical PASS is not relabeled as a current-source PASS.

Actual Windows CTRL_BREAK acceptance, the published logging/graph seam, bounded
store shutdown, runtime instance-lock qualification, the unsuppressed paste
advisory, inherited C3 cleanup CI, current-head platform/Python evidence, full
Release Preflight, parent merges and independent zero-finding QA remain required.
No timer increase, skip, guard suppression, native baseline change or hosted
publication is introduced to bypass them. This record remains in_progress until
those completion conditions have actual evidence.

## Exact-source installed consumer result for QA1

The parent-executed fresh macOS arm64 scratch procedure passed at exact source
`65250a9ee6de25cf591e85dd2ee5ec4dbc4c584c`. The neutral
[evidence receipt](evidence/c-4-consumer-qualification.md) and its
[machine-readable record](evidence/c-4-consumer-qualification.json) bind all three
archive-installed executable hashes, the matching abi3 wheel, payload and raw-log
hashes, actual update/close/read-back, outage/pending/recovery, Python replacement
and rollback, blocked-hook stop/restart and successful marked teardown. The live
driver took 9.9981 seconds; transport returned 0. The disclosed external signature
and exception bridge remains required; no private source or identifiers are
added to this repository.

C2's committed Unix lifecycle proof `8bcd47fa364055879e06392a53cb250ebfe564f7`
was merged afterward without conflicts. Its two changed Rust paths are under
cfg(test); production input blobs match the tested source. That comparison does
not claim a new native build or qualify the later receipt head. Independent Rust
QA must validate the added tests; no full native workspace build was repeated
after the earlier ENOSPC attempt. The shared C3 cleanup helper remains untouched.

The sprint stays in_progress for full initial QA1, current-head hosted evidence,
remaining upstream/C3 dependencies, preflight and parent merge gates. The actual
consumer PASS is not a claim of Windows or complete matrix qualification and no
publication occurred. The parent owns pushing this frozen receipt head.

## QA1 local corrections and retained gates

The QA1 manager reported FAIL at source `e7bafc2e5bdda1bf480823c7dd3ac73d0e16b702`.
C4-REQ-003 is implemented pending independent QA in the existing consumer
build/pack seam: it builds from one immutable committed-source snapshot with its
lockfile into fresh target/wheel directories and no longer accepts arbitrary
prebuilt inputs. Same-version fixture artifacts produced from another real Git
commit cannot enter through the former shared directories or removed options;
missing owned outputs fail instead of falling back to foreign builds. A repeated
unchanged-source fixture passes. These are packaging tests, not native or
installed qualification evidence. The existing receipt format and canonical
84-asset kit remain unchanged, with zero installer drift.

C4-ARCH-001 is implemented pending independent QA by adding ADR-PY-0006 to the
single governing ADR list. Its mixed native package, run/sync modules and neutral
installed corpus remain part of qualification. Every one of the 15 QA1 findings
has a developer disposition appended to its existing `.triage/c/findings/*.ttl`
record; original reviewer records and open statuses are preserved. No finding
is closed by this developer update.

The C3 helper correction awaits its qualified integration merge. The C2-owned
typed observability error and endpoint ownership fixes are separately assigned;
neither is copied into C4. Published logging/graph, Windows graceful shutdown,
contended worker bounds and portable lock/readiness qualification remain upstream
or parent gates. Exact final-source installed matrix, fresh applicable consumer
evidence, full Release Preflight, parent merge, green CI and zero-finding QA
remain required. The actual source652 consumer receipt is unchanged and is not
retargeted to this later execution. No native build, remote scratch execution,
publication, settings mutation or baseline update is claimed in this local round.

## QA-2 checkpoint

At reviewed source `69554386cfff1074fa606aed114331685e3760f5`, six independent
reviewers verified the source-owned packaging correction, ADR-PY-0006 reference,
qualified C3 cleanup and typed observability errors. Their consumer findings
C4-REQ-003, C4-ARCH-001, CI-C4-001 and C4-RBP-F001 are closed with QA-2 provenance.
The [QA-2 report](https://github.com/randlee/beads-cache/pull/110#issuecomment-6612817877)
still records FAIL: 16/22 deliverables, ten Blocking and one Important finding.
Actual Linux candidate validation passed at that source; Windows still exits
3221225786 and the dependent abi3 matrix is skipped. Historical source652
consumer evidence remains distinct from these executions.

The canonical consumer-preflight step omits GH_TOKEN although the consumer
checker supports it; correcting the canonical source and regenerating through
the official installer is required. Full final-source preflight must execute
and locally satisfiable auth/candidate/staging gates must pass. Publication-only
missing-tag and owner-setup refusals may be recorded separately; publication is
not required to close this sprint. The old environment probe conflates API
failure with absence, so inaccessible metadata is not proof of missing setup.
Public runtime fixes remain unimplemented/unpublished according to their owner.
All eleven remaining findings and the phase merge gates remain open.
