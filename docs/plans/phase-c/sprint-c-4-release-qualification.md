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
