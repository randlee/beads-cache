# Candidate qualification and release gates

Phase C uses build-only GitHub Actions artifacts for release qualification.
`Release artifact qualification` (`release-qualification.yml`) checks out the
exact requested commit, builds version 0.2.0, and never creates a tag, GitHub
Release, registry upload or channel update. PR runs use the PR head SHA rather
than GitHub's synthetic merge SHA. Dispatch the workflow again after any source
change; a candidate from an earlier commit is not evidence for a later head.

The binary archive matrix is Linux x86_64, macOS arm64, macOS x86_64 and Windows
x86_64. Each contains exactly `bin/bd-cache`, `bin/bdc`, `bin/bd-cache-daemon`
(with `.exe` on Windows) and `provenance.json`. Linux x86_64 and macOS arm64
also build a single CPython3.9 abi3 wheel from that checkout. The wheel is
installed and exercised on Python3.9, 3.11 and 3.13; other wheels are not promised.
Production binaries are built without test features. Fake backend/interpreter
helpers are separate CI artifacts and never enter a distribution archive.

Each target's `candidate.json` records the full source commit, version, target,
binary hashes and archive/wheel hashes. `checksums.txt` covers exactly the archive
and declared wheel. Release configuration, receipts and acceptance helpers are
excluded from that checksum asset set. The archive embeds its provenance and
binary hashes. Verification rejects omitted executables, mixed commit/version
receipts, changed wheel/archive bytes, unsafe/extra archive members, missing or
extra checksums and configuration accidentally staged as an asset. The trusted
same-source CI job binds wheel hashes to the checkout; a wheel filename alone
cannot establish its source commit.

After extracting into a new temporary installation, the workflow probes all
three executable versions and runs the existing C2 daemon acceptance procedure
with the installed `bdc` and daemon: scheduling, cached CLI access, readiness,
outage/recovery, stop/restart, schema/schedule preservation and bounded shutdown
including a blocked descendant hook. On wheel targets, a new venv installs only
the hashed wheel and runs the Python corpus with the extracted CLI. The isolated
import probe confirms the package lives under that venv. The corpus exercises
cached Python access, Python replacement sync and hooks. These checks use FakeBd;
they do not replace the separately authorized real-backend scratch procedure in
[runtime.md](runtime.md). External consumer source stays outside this repository.

For a downloaded candidate directory, checks are reproducible:

```sh
python3 -I scripts/check_release_candidate.py check --directory candidate \
  --commit <full-sha> --version 0.2.0 --target aarch64-apple-darwin
python3 -I scripts/check_release_candidate.py smoke --directory candidate \
  --commit <full-sha> --version 0.2.0 --target aarch64-apple-darwin \
  --fake-bd /absolute/path/fake-bd --fake-python /absolute/path/fake-python
```

The smoke needs Python3.9+, venv/pip, network access to install the declared pytest
helper dependency and same-platform acceptance helper binaries. Those helpers
must come from the same candidate workflow. It changes only fresh temporary
installations/scratch state, then removes them; it does not use a real backend.
A failure prints `CANDIDATE.QUALIFICATION` with a recovery action. Commands have
bounded timeouts and timed-out process groups are killed and reaped.

## Consumer preflight and kit pin

The consumer input declares `installed-candidate` through sc-publish's existing
`preflight_checks` extension. Canonical installation renders
`release/publish-preflight.toml`; no shared workflow is patched by hand. The
check validates both consumer/rendered manifests, local sc-cron removal, all
three binaries and reviewed isolated staging in the pinned kit. It then reads
public GitHub run/job/artifact metadata and requires a successful qualification
run from this exact commit, all four installed binary jobs, all six installed
Python jobs and unexpired candidate/evidence artifacts. The actual membership,
checksums and installed smokes execute within those successful jobs. No token
is needed for public metadata; `GH_TOKEN` may be supplied without logging it.
Missing/expired evidence fails with a rerun instruction. API denial/rate limits
fail closed. This is evidence admission through the existing preflight extension,
not a new publication framework.

The immutable candidate kit revision is
`5fd4d0db1b7cbb4ebf9316588047391a8e4be496`. Its normal release workflow initializes
separate runner-temporary download and upload asset directories, stages only
archive/wheel patterns into the latter, computes checksums there and uploads
that asset directory. Upstream issue135 remains open; its source change alone
is not an organization-wide publication qualification. C4 records canonical
installer/dry-run and kit regression evidence without claiming a hosted release
or channel retry happened. The consumer negative fixtures refuse the old pin
and old `release/*` upload behavior. Preserve the normal workflow's initialized
asset directory; a lexical ban on every historical `release` fallback would not
be an accurate test of the upstream fix.

Regenerate only with the official pinned installer and its pinned sc-compose
bootstrap, then require a clean repeat `--dry-run`. Channel templates are
consumer-owned copies at declared paths; Scoop's `beads-cache.json.j2` and the
Homebrew binary list expose all three archive executables. The kit-owned
getting-started document stays canonical; consumer daemon guidance lives in the
root README and runtime documentation.

## Actual qualification and publication state

No candidate or supported-platform pass is claimed by this document alone.
Record exact workflow run URLs and head SHAs in the C4 sprint evidence after
execution. Run the full `Release Preflight` workflow at the same final SHA with
`version=0.2.0` and `run_by_agent=publisher`; preserve every failed gate. General
PR CI is not a substitute. The pinned runtime0.1.1 dependency's paste advisory
remains unsuppressed pending the owner's deferred runtime0.2.0 update; do not
patch a checkout or silently advance that pin to make a gate green.

Stable publication and channels are separate gates. Verify issue135's actual
fix qualification and immutable-release behavior, credentials, protected
environments and destination setup before authorizing any publication. Missing
crates/PyPI/Homebrew/Scoop/WinGet setup stays pending. The first WinGet submission
is owner setup. Channel dry-runs require an actual immutable release/tag and
configured channel access; absent that state, report them pending and do not
create stable assets to test them.

Post-merge follow-up: after Phase C lands on develop/main and that phase head is
qualified, obtain separate publication authorization, cut a hosted prerelease
from that allowed branch, and run installed prerelease/channel qualification.
No hosted prerelease, stable release, tag, PyPI/crates upload or channel activation
is part of this sprint's candidate build.
