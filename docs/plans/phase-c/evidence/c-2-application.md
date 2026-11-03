# C2 application development evidence

Development branch: `sprint/c-2-framework-application`. Parent development heads
consumed: C1 `4d27523d2919b8e0c0e6d0f9287c5e4fc9736e56`, C3
`794d5f0a2fab27e647b5f526d6f671392282d0d7`. Parent QA/CI merges are still
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
8 daemon tests and1 readiness ownership test. Inherited canonical logger adapter
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

## Actual scratch consumer acceptance status

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
