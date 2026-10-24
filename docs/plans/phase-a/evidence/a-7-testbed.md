# a-7 testbed evidence

Sprint a-7 (`docs/plans/phase-a/sprint-a-7-sync-integration.md`, deliverable 5).
The testbed is the `m5-atmbench` account, run by the team-lead (NFR-BDC-0006:
live tests run only there and in the bd-compat workflow).

## Status: deferred, owner team-lead (to be run after fix-a-7-1 lands)

The developer session that wrote this sprint has no access to `m5-atmbench`.
Under deliverable 5, the recording-invoker results (CI, below) close the
sprint, and the testbed items are deferred evidence:

| Item | Owner | Due | State |
|---|---|---|---|
| a-2 live probes (`crates/sc-bd-cli/tests/live.rs`, via `scripts/bd-compat/run-live.sh`) | team-lead | before the phase-a merge to `develop` | deferred |
| End-to-end run against a scratch `bd` (`tests/live.rs::a_journaled_write_lands_in_a_real_bd_through_the_tick`) | team-lead | before the phase-a merge to `develop` | deferred |
| Sync timing, 1000-bead phase (`tests/live.rs::an_incremental_sync_of_a_1000_bead_phase_is_under_1_s`, NFR-BDC-0003) | team-lead | before the phase-a merge to `develop` | deferred |

Date of this record: 2026-10-24. The team-lead replaces the "deferred" state
with the figures below when the run is done.

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
- `an_incremental_sync_of_a_1000_bead_phase_is_under_1_s`: imports 1000 beads
  with `updated_at` an hour in the past (`bd import`), runs a full tick, changes
  five beads with `bd update`, then times `Cache::sync` and asserts it stored
  fewer than 50 rows and took under 1 s.

## Recorded runs

### Developer smoke run (not testbed evidence)

To check that the two live tests are correct before handing them to the
testbed, they were run once on the developer's machine through
`crates/bd-cache-sync/tests/run-live.sh` (a scratch Dolt 2.3.5 server on a
loopback port and a scratch workspace, both removed afterwards), macOS arm64,
`bd version 1.3.0`, `SC_BD_PINNED_ANY=1`, at sprint commit `0332079` (the last code commit):

```text
test a_journaled_write_lands_in_a_real_bd_through_the_tick ... ok
full sync of 1000+ beads (whole tick): 600ms; incremental sync: 544ms (6 rows stored, 0 blocked)
test an_incremental_sync_of_a_1000_bead_phase_is_under_1_s ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; finished in 11.35s
```

Incremental sync of a 1000-bead phase: 544 ms against the 1 s limit (459 ms in an earlier run at `2bad273`)
(NFR-BDC-0003). This is a debug build on an idle laptop with a local Dolt
server; it is a sanity figure, not the recorded NFR evidence, which is the
testbed's.

### Testbed run (m5-atmbench) - to be filled in by the team-lead

Run both scripts from a checkout of `sprint/a-7-sync-integration` (or of the
merge), each with a clean `cargo` target, and paste the output here.
`LIVE_TIMEOUT_S` (default 1800) bounds `crates/bd-cache-sync/tests/run-live.sh`;
every `bd` call of the live test's own setup is bounded at 60 s, and the tests
run one at a time, each with its own cache file.

| Item | Command | Date | Host / bd / dolt versions | Result | Figure |
|---|---|---|---|---|---|
| a-2 live probes | `scripts/bd-compat/run-live.sh` | | | | 7 probes |
| a-7 write lands in a real bd | `crates/bd-cache-sync/tests/run-live.sh` (`a_journaled_write_lands_in_a_real_bd_through_the_tick`) | | | | applied, no read-back error |
| NFR-BDC-0003 incremental sync of 1000 beads | same run (`an_incremental_sync_of_a_1000_bead_phase_is_under_1_s`) | | | | the `NFR-BDC-0003:` line: incremental ms and `PASS`/`OVER` against 1 s; rows stored < 50 |

The timing is reported with its limit and is not an assertion (a shared host
can stall any one call); the test asserts only that the sync was incremental.
`OVER` on the testbed is a finding to raise, not a silent pass.

## Recording-invoker results in CI

The same flow with the `bd` process replaced by `FakeBd` (a recording
invoker that parses the argv `BdCli` built and applies it to a fake bd) runs
in every CI build: `crates/bd-cache-sync/tests/e2e.rs`. The CI run is cited in
the sprint doc.
