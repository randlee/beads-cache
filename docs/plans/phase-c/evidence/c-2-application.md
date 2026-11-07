# C2 application development evidence

Current status: C2 consumes merged C1/C3 and published SDK 0.2.0; see the current
[consumption receipt](c-2-registry020-consumption.md). The actual scratch source
is `ab7b5bcf168d3abb0dcf2d0ecb1700d8859b58c6`, followed by the docs-only reviewed
`08f7cee0308f52eb0a5a0067cb4e209e2c82806e`. At that reviewed head all 11 Python
checks and lint/advisories passed; all three native jobs failed on the missing
published `testing::instrument_router` helper. Compatible public SDK consumption,
exact-candidate native lifecycle/Windows qualification, CI and independent C2
approval remain open. Later executable corrections require fresh evidence.

The checkpoints below are historical, including the original
`623218021d27ef247b389233650314896b69cbc1` scratch receipt and failed runs. They
are not current source/artifact or acceptance claims.

## Windows acceptance decoding correction

General CI run `41567572146` reached the daemon diagnostic assertion on Windows
and failed because subprocess text decoding used the host locale for UTF-8
product output. After merging integration `22f9d6b9ddc67274d95a23c16a2a8345c0785bc0`,
the acceptance script explicitly decodes all captured product processes as UTF-8,
including the long-running daemon pipes. The diagnostic prefix, em dash,
single-line and redaction assertions remain unchanged. System utility output
continues to use its platform decoder.

A focused subprocess regression emits raw UTF-8 diagnostics with a simulated
CP1252 default decoder: the control corrupts the text and the product boundary
preserves it exactly. Invalid UTF-8 still raises instead of being replaced.
Local validation passes: 99 script tests (one existing smoke skip), formatting,
all-target Clippy with warnings denied, and the complete FakeBd daemon process
acceptance (including blocked child/grandchild cleanup in 1.007 seconds).
This is a test-harness portability change; the native artifact sources qualified
above are unchanged. A new hosted Windows run remains required.

Development branch: `sprint/c-2-framework-application`. Parent development heads
consumed: C1 `4d27523d2919b8e0c0e6d0f9287c5e4fc9736e56`, C3
`ef181f69944cad0356bb5f3ee4a04f7c92656730`. Parent QA/CI merges are still
completion prerequisites. This is a development report, not phase closure.

## Pinned generation and actual adoption

2026-11-02 client date. `python3 -I scripts/runtime_templates.py generate --source
/Users/randlee/github/sc-runtime --output /tmp/c2-pinned-baseline` succeeded:
v0.1.1/5af24c862fe7ed8dae8710b7bd64a84a29506b51, cargo-generate0.25.1,
sc-compose1.7.0,38files, registry_dependencies=true, agent_documents=true.
A separately generated repeat baseline had zero changes.

A scratch clone of that exact upstream commit received one controlled comment
change in `template/crates/daemon/src/routes.rs`, committed/tagged locally as
`f82f85a740c4430b195877bf1c2551382f959fd6`/`c2-adoption-proof`.
A scratch alternate pin referenced that full commit. Running the same helper
`generate --pin /tmp/c2-upkeep-new-pin.toml --source /tmp/c2-upstream-proof
--output /tmp/c2-regenerated-changed` regenerated the changed scaffold. `diff`
against the pristine old baseline with `--mapping runtime/adoption.toml` reported
exactly one modified adopted path, generated `crates/daemon/src/routes.rs`, adopted
`crates/bd-cache-daemon/src/routes.rs`. The mapping caused no writes to application
files. The real checked-in runtime pin and answers stayed unchanged. This is a
planted upstream delta proof, not a claimed new upstream release.

## Local executable results

`cargo clippy --offline -p bd-cache-service -p bd-cache-daemon --all-targets -- -D
warnings` PASS. `cargo test --offline -p bd-cache-service -p bd-cache-daemon` PASS:
8 daemon tests and2 service/readiness ownership tests. Inherited canonical logger adapter
tests cover sink admission panic/saturation/disconnection and timeout child reap.
The service test cancels its response, checks the next probe remains busy, closes
admission and proves join waits for the actual work to finish.

`CARGO_NET_OFFLINE=true python3 -I scripts/consumer_checks.py --graph` PASS for
independent normal/build CLI, API and Python graphs: no runtime/transport/axum/MCP
or application assembly leakage; API/Python no clap; contract-only sc-command.

Built binaries were copied into isolated `/tmp/c2-accept-bin` before process
qualification to avoid another Cargo build replacing active test executables.
`python3 -I scripts/consumer_checks.py --bdc /tmp/c2-accept-bin/bdc --daemon
/tmp/c2-accept-bin/bd-cache-daemon --fake-bd /tmp/c2-accept-bin/fake-bd --fake-python
/tmp/c2-accept-bin/fake-python` PASS on macOS arm64. The real processes verified:

- malformed/missing/typed config, endpoint syntax and listener policy failures,
  bounded safe stderr/empty stdout/exit1, and no cache open before refusal;
- scheduled real FakeBd sync, v3 pre-existing file identity, cached row and
  schedule-row preservation; local supported journaling/drain/read-back;
- outage health503 while liveness stays200, readiness answered recorded state
  while backend calls were causally gated, recovery to200;
- duplicate-owner refusal, bind failure cleanup, immediate restart, and local
  cache health/read availability with daemon absent;
- blocked hook child/grandchild stopped and reaped, second owner refused during
  worker grace, then restart succeeded. Stops measured0.0027s,0.0015s and1.0051s
  for the blocked hook with T=1s. Bound asserted:31s HTTP +2T +1s read +1s tolerance.

## Historical scratch consumer prerequisite checkpoint

Read-only designated-testbed checks confirmed bd1.3.2(dev), Dolt2.3.1 and
Python3.11.16. The existing checksum-verifying installer placed bd1.3.1
(c1c4b642a) in owned remote temporary storage; no live bd workspace command ran.
The generated strict model declares BD_VERSION1.3.1 and schema sha256
caf8fe18e1117c067ff7bbd824484b24849677e2cbf86d793ddf6b64069a31a2.

Actual consumer execution is pending explicit export approval. Automatic approval
review rejected the proposed external-source/native-artifact transfer and scratch
workspace initialization, because user authorization did not explicitly name that
payload and destination. No denied transfer or scratch setup was retried. A
minimal local-only manifest and command plan were prepared outside this repo for
review: seven unchanged external import modules at source commit
`d21a1b60de465b2f5abde8b36f8b2cbf2155011c`, same-commit native artifact rebuilds,
and marker-qualified scratch scripts. The supported real write/drain/read-back
boundary is selected; provider substitution is explicit. No synthetic test is
claimed as actual consumer proof. Issue104 create behavior remains excluded.

## Historical post-push validation and blockers

Initial application head `39c8587cc40c412f74734a8f6d797458e04daebb`:
full workspace suite1157passed/0failed/20ignored; installed abi3 wheel corpus
224passed/2skipped; cached native get median74.3us/p9986.8us. Original release
CLI gate median5.8ms/p9510.4ms (50ms median gate), pending-case median6.6ms.
Full/default-library Clippy, rustdoc warnings-denied, formatting, workspace edge
table, isolated consumer graphs and no-backend-client/strict-neutrality gates pass.
The new process acceptance also checks --help/usage; it caught the missing
clap/help feature inherited from workspace default-feature-off configuration.
The daemon now explicitly enables help; existing local CLI remains unchanged.
C3 QA1 fixed helper head was subsequently merged cleanly before this fix round.

Required `cargo deny check advisories` FAILS: RUSTSEC-2024-0436,
[paste is unmaintained](https://rustsec.org/advisories/RUSTSEC-2024-0436.html),
paste1.0.15 ← utoipa-axum0.2.0 ← pinned sc-runtime0.1.1 (also direct daemon).
Both runtime's utoipa-axum and utoipa-axum's paste dependencies are unconditional;
features cannot remove them. The current public crates.io index, fetched during
qualification, has only0.2.0 in the0.2series. Version0.3.0 uses pastey but requires
utoipa6 and is outside runtime's ^0.2 range. No compatible published resolution
removes the advisory. [The released0.2manifest](https://raw.githubusercontent.com/juhaku/utoipa/utoipa-axum-0.2.0/utoipa-axum/Cargo.toml)
confirms the unconditional paste dependency. No ignore, patch, fork or unapproved
pin change is applied. A compatible upstream maintenance release switching the
macro dependency, or an approved new framework baseline, is required for green CI.

Actual consumer integration has an additional observed call-signature mismatch:
the selected unchanged method omits native write's required `precondition`, and
calls drain with no arguments while native drain requires `start_gate_s` and
`hard_s`. Direct native provider substitution cannot run unchanged. An explicit
external signature/exception adapter has been proposed outside this product tree;
it supplies None and bounded25s/28s budgets and preserves native return dicts.
Its use is pending review/authorization with the export. No consumer adaptation
or actual live acceptance is claimed complete, and native0.1 signatures are
preserved. The sprint must not close while these blockers remain.

## Approved qualification preparation

The owner explicitly approved transfer and scratch qualification, including the
bounded external provider bridge, on the current chat. C3 fixture isolation fix
`ef181f69944cad0356bb5f3ee4a04f7c92656730` was merged before this round;
the97-script suite now passes (one separately exercised real-generation smoke
skipped). Candidate native artifacts will be built from the exact pushed source
head recorded in the transport manifest. Authorized transport is performed by
the parent chat; the developer analyzes the returned raw execution logs. This
approval does not waive the unsuppressed advisory or claim live proof yet.


## QA1 correction source boundary

The [owning sprint dispositions](../sprint-c-2-framework-application.md#qa1-correction-delivery-and-open-gates)
retain all 36 manager FAIL IDs. C1 native corrections (73ced71 plus test-only
96ce326) are now consumed; the old actual consumer artifact receipt remains tied
to 623218021d27ef247b389233650314896b69cbc1 and does not qualify this changed source.
No scratch transport or remote rerun occurred during these corrections.

The new real HTTP/service fixture proves typed busy admission, actual upstream
503/RUNTIME.REQUEST_TIMEOUT response cancellation, retained capacity/cache claim,
actual reader cleanup and restart under an owned process watchdog. Its explicit
testing-feature observer holds one real health evaluation inside the existing
one-second deadline. It does not claim runtime daemon.lock ordering: that exact
lifecycle proof remains open because the published runtime test control is private.
The earlier sleep-dependent duplicate launch inside finite worker grace is removed;
real duplicate refusal before stop, blocked-tree reap and restart remain.

Windows acceptance still sends its original isolated-group CTRL_BREAK and requires
zero exit plus cleanup. Published runtime 0.1.1 handles only CtrlC, so QA-C2-001 is
still blocked upstream. Fixture cleanup now owns all descendants independently of
whether production graceful stop succeeds. The strict-byte decoding regression
runs on the caller thread rather than Windows subprocess reader threads.
