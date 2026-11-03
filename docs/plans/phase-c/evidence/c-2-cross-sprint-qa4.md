# C2 cross-sprint QA4 correction and published API blockers

This correction starts at C2 `8bcd47fa364055879e06392a53cb250ebfe564f7` and
addresses the two C2-owned findings from the complete
[C4 QA1 FAIL](https://github.com/randlee/beads-cache/pull/110#issuecomment-6598761542).
The assignment is `/tmp/phase-c-execution/c-2/qa-4/cross-sprint-fix.xml`.
All ten assigned finding records retain `triage:status "open"`; developer
implementation is pending independent QA, required CI and parent qualification.

## Typed observability initialization

`C4-RBP-F001` is implemented pending independent QA. The public
`ObservabilityStart` owns the actual `ValueValidationError` or canonical
`InitError`, implements `std::error::Error::source`, and forwards the original
operation-specific diagnostic code and full remediation. Its source can be
downcast after propagation through an ordinary `Box<dyn Error>` boundary. The
service-name validator has no upstream remediation, so that case adds explicit
identifier guidance without replacing its native source/code.

No initialization path converts its upstream error to a string. Display delegates
to the retained cause when requested. The final daemon process boundary retains
the approved `DAEMON.OBSERVABILITY` envelope and bounded/control-escaped formatter,
and uses the actual first recovery action. Four public integration regressions
obtain actual upstream errors: invalid service name, builder rejection of zero
queue capacity, build rejection with both sinks disabled, and an actual daemon
process with disabled sinks. They check concrete source types, retained diagnostic
reference identity, complete remediation, empty stdout, exact single-line stderr,
exit 1, absence of secret config/path values and refusal before stores open.

## Endpoint ownership: concrete published blocker

`C4-FTQ-001` remains OPEN. The existing Unix lifecycle test still drops its
reservation before the runtime binds; it is retained without claiming that this
race is fixed. Its real `run_with_cleanup`, actual SIGTERM, concurrent `LockHeld`,
admitted reader join, instance retention and restart assertions remain intact.

Inspection of the pinned registry `sc-runtime 0.1.1` establishes the gap:

- `src/builder.rs`: `serve` binds the listener, obtains `listener.local_endpoint()`
  and publishes it through `control.ready` while the listener remains owned.
  `DaemonBuilder::with_control` is `pub(crate)`; public `run_with_cleanup` has
  no bound-endpoint notification or owned-listener input.
- `src/testing.rs`: public `DaemonFixture` uses the private control internally
  and reports the actual endpoint, but calls `builder.run()` without an
  application cleanup callback. Its detached control disables actual process
  signals and its start methods replace the instance root. Substituting it would
  lose required lifecycle observables.
- A store-start handshake precedes bind and cannot establish endpoint ownership.
  Configuring port zero alone does not expose the chosen endpoint to this public
  lifecycle caller. Discovering it through private state or logs is not the
  supported endpoint mechanism required by the finding.

The minimal compatible upstream surface is a public bound-endpoint/serving
notification usable with `run_with_cleanup` and `tcp:127.0.0.1:0`, preserving
production signal installation and the selected instance root. The callback or
notification must carry `local_endpoint()` only after successful bind while the
runtime retains the listener. A suitable upstream fixture may provide equivalent
cleanup/root/signal ownership. After an approved published version is authorized,
the consumer must use that actual endpoint and add a deterministic competing-bind
negative while ownership is held, then retain all existing lifecycle assertions.
No local fork, bind retry, random-port loop, sleep or timeout expansion is added.

## All remaining assigned findings

| ID | Developer disposition; every status remains OPEN |
|---|---|
| QA-C2-001 | Published production Windows CtrlC-only handling lacks isolated-group CtrlBreak graceful stop. Require upstream ctrl_break registration/polling before lock/bind and the same drain/cleanup path; hosted Windows must prove zero exit, readiness/worker joins, descendant reap, unlock and restart. |
| C2-REQ-003 | Exact final-head OS/Python/advisory CI, parent completion and current-candidate consumer qualification remain required. Historical source6232180 receipts are not retargeted. |
| C2-REQ-004 | Await approved published bounded worker/store compatibility through C1; safe owned joins remain and no detached work or padded deadline is introduced. |
| C2-REQ-007 | Consume only C1's approved published logging seam to remove the actual unauthorized sync-to-sc-command introducer; retain the corrected canonical fail-closed graph check and planted negative. |
| C2-REQ-009 | Real Unix signal/instance-lock proof remains, with endpoint race explicitly recorded above. Portable lifecycle, Windows acceptance and contended-store bounds remain required. |
| ARCH-001 | Approved direct-dependency policy is unchanged; logging-seam correction belongs to C1/upstream. |
| RSH-001 | Contended outstanding store work lacks the required bounded owned stop/join contract in the published baseline; await and qualify C1/upstream compatibility. |
| RBQA-F001 | Distinct unauthorized direct logging-edge finding retained independently; no ADR or dependency exception is expanded. |
| C4-RBP-F001 | Implemented pending independent QA as described above. |
| C4-FTQ-001 | Blocked on the concrete public bound-endpoint/cleanup seam above. |

## Qualification inputs and local validation

The earlier `/tmp/c2-next-candidate-qualification-inputs.json` remains the prepared
input contract for parent-owned fresh qualification. No candidate is selected
until approved compatible sources are frozen; final CLI/daemon/wheel artifacts
must identify one exact source and version with fresh hashes, native installation
and the existing scratch procedure. This worker does not transfer or execute
remote artifacts. No build, receipt or source from the old6232180 qualification
is relabeled as current.

Local execution uses one explicit `/tmp/c4-target` and `CARGO_INCREMENTAL=0`.
Focused initialization regressions, formatting and daemon all-target strict
Clippy are checked before commit. Exact pushed SHA and final focused
observability/lifecycle validation logs are delivered to the parent separately
under `/tmp/phase-c-execution/c-2/qa-4/`. A passing lifecycle sample will not close
the statically demonstrated endpoint race. The prior1171-test workspace result
is not rerun or claimed to validate this correction. Published runtime/sc-cron
remain0.1.1; no0.2 baseline adoption, sc-lint bootstrap, advisory suppression,
cross-owned C4 changes, self-merge or upstream publication occurs here.
