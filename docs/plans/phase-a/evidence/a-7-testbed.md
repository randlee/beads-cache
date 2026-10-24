# a-7 testbed evidence

Sprint a-7 (`docs/plans/phase-a/sprint-a-7-sync-integration.md`, deliverable 5).
The testbed is the `m5-atmbench` account, run by the lead (bc-lead) (NFR-BDC-0006:
live tests run only there and in the bd-compat workflow).

## Status: final-code-commit run recorded (merge `7702b67`); bc-lead's merge-head run due 2026-10-31

The developer session that wrote this sprint has no access to `m5-atmbench`.
Under deliverable 5, the recording-invoker results (CI, below) close the
sprint, and the testbed items are deferred evidence:

| Item | Owner | Due | State |
|---|---|---|---|
| a-2 live probes (`crates/sc-bd-cli/tests/live.rs`, via `scripts/bd-compat/run-live.sh`) | bc-lead | 2026-10-31 | final-code-commit run recorded (7702b67); bc-lead merge-head run due 2026-10-31 |
| End-to-end run against a scratch `bd` (`tests/live.rs::a_journaled_write_lands_in_a_real_bd_through_the_tick`) | bc-lead | 2026-10-31 | final-code-commit run recorded (7702b67); bc-lead merge-head run due 2026-10-31 |
| Sync timing, 1000-bead phase (`tests/live.rs::an_incremental_sync_of_a_1000_bead_phase_is_incremental_and_its_time_is_reported`, NFR-BDC-0003) | bc-lead | 2026-10-31 | final-code-commit run recorded (7702b67); bc-lead merge-head run due 2026-10-31 |

Date of this record: 2026-10-24. The recorded runs used the `atmbench` account, run by the fix-a-7-6/7 developer. The run at `9f3ceb5` was made on the testbed account by the fix-a-7-6 developer (the testbed ran at `9f3ceb5`; the layer fix-a-7-7 changes tests only), and it was repeated at `7702b67`, the merge of `integrate/phase-a` (a-8 landed, `71084a5`) and now the last code-changing commit. bc-lead's run at the merge head is due 2026-10-31; no code changes between
`7702b67` and the head, only documents and the evidence script.

## Commands (on `m5-atmbench`)

```bash
# a-2's probes (scratch Dolt server, scratch workspace, removed on exit)
scripts/bd-compat/run-live.sh

# a-7's end-to-end run and the sync timing, same scratch scripts
crates/bd-cache-sync/tests/run-live.sh
```

`run-live.sh` starts a scratch `dolt sql-server` on a free loopback port with
its own Dolt configuration, runs `bd init` in a new directory, runs
`cargo test -p bd-cache-sync --all-features --test live -- --ignored
--test-threads=1 --nocapture`, and tears everything down. No shared database
is read or written (`tests/live.rs` refuses a workspace without the
`.beads/sc-bd-live-scratch` marker), and no `atm` message is sent (the sender
is a recorder). Set `SC_BD_BIN` for a `bd` other than the one on `PATH`, and
`SC_BD_PINNED_ANY=1` when it is not the pinned release.

What the two tests do:

- `a_journaled_write_lands_in_a_real_bd_through_the_tick`: creates a phase
  root and a bead with `bd create`, builds the composition with `compose`
  (`SqliteStore`, `FlockDrainLock`, `BdCli` running `bd`), journals a close
  through `Cache::write`, runs `SyncJob::run_blocking` (the scheduler's entry
  point), and checks the entry is `applied` with no read-back error and that
  `bd show` says `closed`.
- `an_incremental_sync_of_a_1000_bead_phase_is_incremental_and_its_time_is_reported`: imports 1000 beads
  with `updated_at` an hour in the past (`bd import`), runs a full tick, changes
  five beads with `bd update`, then times `Cache::sync`, asserts it stored
  fewer than 50 rows, and reports the time against the 1 s limit (PASS/OVER; the time is reported, not asserted).

## Recorded runs

### Developer smoke run (not testbed evidence)

To check that the two live tests are correct before handing them to the
testbed, they were run on the developer's machine through
`crates/bd-cache-sync/tests/run-live.sh` (a scratch Dolt 2.3.5 server on a
loopback port and a scratch workspace, both removed afterwards), macOS arm64,
`bd version 1.3.0`, `SC_BD_PINNED_ANY=1`, at the sprint's last code commit
`bc1e4ab` (one record; earlier runs are not kept):

```text
test a_journaled_write_lands_in_a_real_bd_through_the_tick ... ok
NFR-BDC-0003: full sync of 1000+ beads (whole tick) 572.762917ms; incremental sync 508.340584ms (6 rows stored, 0 blocked); limit 1s: PASS
test an_incremental_sync_of_a_1000_bead_phase_is_incremental_and_its_time_is_reported ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; finished in 8.90s
```

Incremental sync of a 1000-bead phase: 508 ms against the 1 s limit
(NFR-BDC-0003). This is a debug build on an idle laptop with a local Dolt
server; it is a sanity figure, not the recorded NFR evidence, which is the
testbed's.

### Testbed run at the last code-changing commit, the merge `7702b67` (m5-atmbench)

Same host, bd 1.3.2 (dev), 2026-10-24 19:05Z, the same recipe: `git archive` of `7702b67` into
`~/github/beads-cache-a7-7702b67`, remote scripts `~/run-a7-testbed-7702b67.sh` and `~/run-a2-pinnedany-7702b67.sh`.

| Item | Result | Figure |
|---|---|---|
| a-2 live probes | 6/7 pass | `fingerprint_reports_what_bd_prints` fails: bd 1.3.2 is not the pinned 1.3.1 (as designed) |
| a-2 live probes, `SC_BD_PINNED_ANY=1` | 7/7 pass | 7 probes |
| a-7 live (2 tests) | pass, 2/2 | applied, no read-back error |
| NFR-BDC-0003 incremental sync of 1000 beads | PASS | full tick 531.4 ms; incremental sync 391.0 ms (6 rows stored, 0 blocked); limit 1 s |

Logs on the testbed: `~/a7-testbed-7702b67.log`, `~/a2-pinnedany-7702b67.log`.

### Testbed run at `9f3ceb5` (m5-atmbench)

Host `m5-atmbench` (atmbench@rand-m5), bd 1.3.2 (dev) against the pinned 1.3.1, debug build, 2026-10-24. The tree
was `git archive` of `9f3ceb5` into `~/github/beads-cache-a7-9f3ceb5`, run by the remote script files
`~/run-a7-testbed-9f3ceb5.sh` (a-2 probes, then a-7 live) and `~/run-a2-pinnedany-9f3ceb5.sh`
(`SC_BD_PINNED_ANY=1`).

| Item | Command | Result | Figure |
|---|---|---|---|
| a-2 live probes | `scripts/bd-compat/run-live.sh` | 6/7 pass | `fingerprint_reports_what_bd_prints` fails: bd 1.3.2 is not the pinned 1.3.1 (as designed) |
| a-2 live probes, `SC_BD_PINNED_ANY=1` | same | 7/7 pass | 7 probes |
| a-7 write lands in a real bd | `crates/bd-cache-sync/tests/run-live.sh` | pass (2/2 with the timing test) | applied, no read-back error |
| NFR-BDC-0003 incremental sync of 1000 beads | same run | pass, PASS | full tick 525.0 ms; incremental sync 380.0 ms (6 rows stored, 0 blocked); limit 1 s |

Logs on the testbed: `~/a7-testbed-9f3ceb5.log`, `~/a2-pinnedany-9f3ceb5.log`.

### Earlier testbed runs (m5-atmbench), run by the lead (bc-lead), before the round 5 code

Host `m5-atmbench` (atmbench@rand-m5), bd 1.3.2 (dev), dolt 2.3.1, rust 1.98.1,
debug build, 2026-10-24.

| Item | Command | Commit / date | Result | Figure |
|---|---|---|---|---|
| a-2 live probes | `scripts/bd-compat/run-live.sh` | `2f7bd38`, 13:25Z; `aec3a12`, 14:41Z | 6/7 pass | `fingerprint_reports_what_bd_prints` fails: bd 1.3.2 is not the pinned 1.3.1 (as designed) |
| a-2 live probes, `SC_BD_PINNED_ANY=1` | same | `2f7bd38`; `aec3a12` | 7/7 pass | 7 probes |
| a-7 write lands in a real bd | `crates/bd-cache-sync/tests/run-live.sh` (`a_journaled_write_lands_in_a_real_bd_through_the_tick`) | `2f7bd38`; `aec3a12` | pass (2/2 with the timing test) | applied, no read-back error |
| NFR-BDC-0003 incremental sync of 1000 beads | same run (`an_incremental_sync_of_a_1000_bead_phase_is_incremental_and_its_time_is_reported`) | `2f7bd38`: 371.1 ms (full tick 496.6 ms); `aec3a12`: 382.7 ms (full tick 499.3 ms) | pass, PASS | 6 rows stored (< 50); 382.7 ms against the 1 s limit |

Logs on the testbed: `~/a7-testbed-2f7bd38.log`, `~/a7-testbed-aec3a12.log`,
`~/a2-pinnedany-aec3a12.log`.

The timing is reported with its limit and is not an assertion (a shared host
can stall any one call); the test asserts only that the sync was incremental.
`OVER` on the testbed is a finding to raise, not a silent pass.

## Recording-invoker results in CI

The same flow with the `bd` process replaced by `FakeBd` (a recording
invoker that parses the argv `BdCli` built and applies it to a fake bd) runs
in every CI build: `crates/bd-cache-sync/tests/e2e.rs`. The CI run is cited in
the sprint doc.
