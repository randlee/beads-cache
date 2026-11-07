## Sprint report
C1 PR108 is pushed clean at `c4006f8d218b743aeabeddcfac9bb5c1c0af8907`, from prior C1 head `0c5893a5a9fa6496ba8d84e7190fab16ebe3eb09`. The candidate pins published registry `sc-cron =0.2.0` and `sc-command =0.2.0` with default features off; archive SHA256s match official index checksums and both VCS records identify `118ee72d831f79778f3584f57a48706111754673`. All 49 shipped SDK Rust files match qualified C5 commit `c3ce3d1b271b54fbb47efaef57ee1563a390cd9e` byte-for-byte, and all 262 Rust files match the prior C1 head. The existing public `sc_cron::observability` facade, SQLite/provider pins and prohibited-route graph restrictions remain. This does not establish upstream full registry consumer acceptance. The untouched sprint retains verbatim user authorization, approved scope, all eight AC rows, and v3 anchor metadata; no anchor/question/scope/exclusion/threshold edits. Exact-head CI is 15/15 `SUCCESS`. Developer STOP; no final acceptance, merge, or publication approval is claimed; REQ-QA-004 remains open.

## Files changed
 .github/workflows/ci.yml                           |   30 +-
 .github/workflows/python.yml                       |    6 +-
 .triage/c/findings/ARCH-001.ttl                    |   24 +
 .triage/c/findings/ARCH13-001.ttl                  |   17 +
 .triage/c/findings/CI-C2-RUNNER-001.ttl            |    4 +
 .triage/c/findings/FTQ-009.ttl                     |    6 +
 .triage/c/findings/FTQ-010.ttl                     |   14 +
 .triage/c/findings/QA2-001.ttl                     |   24 +
 .triage/c/findings/RBQA-F001.ttl                   |   26 +
 .triage/c/findings/REQ-QA-004.ttl                  |   30 +
 .triage/c/findings/RQA10-001.ttl                   |    6 +
 .triage/c/findings/RQA11-001.ttl                   |    4 +
 .triage/c/findings/RQA9-001.ttl                    |    6 +
 Cargo.lock                                         |   28 +-
 Cargo.toml                                         |   11 +-
 crates/bd-cache-cli/Cargo.toml                     |    2 +-
 crates/bd-cache-store/Cargo.toml                   |    4 +
 crates/bd-cache-store/src/codec.rs                 |    9 +
 crates/bd-cache-store/src/health.rs                |  690 ++++++++++++
 crates/bd-cache-store/src/lib.rs                   |   10 +
 crates/bd-cache-store/src/schedule.rs              |  805 +++++++++++++-
 crates/bd-cache-store/src/schedule_budget.rs       |  186 ++++
 crates/bd-cache-store/src/schema.rs                |    3 +-
 crates/bd-cache-store/src/store.rs                 |  175 ++-
 crates/bd-cache-store/tests/schedule_store.rs      |   52 +-
 .../bd-cache-store/tests/support/fixture_child.rs  |  257 +++++
 .../tests/support/schedule_protocol.rs             |  328 ++++++
 crates/bd-cache-sync/Cargo.toml                    |    3 +-
 crates/bd-cache-sync/src/compose.rs                |    4 +
 crates/bd-cache-sync/src/control.rs                |   15 +
 crates/bd-cache-sync/src/cron.rs                   |  204 +++-
 crates/bd-cache-sync/src/cron_log.rs               |   58 +
 crates/bd-cache-sync/src/history.rs                |   48 +-
 crates/bd-cache-sync/src/job.rs                    |  252 +++--
 crates/bd-cache-sync/src/launch.rs                 |   95 ++
 crates/bd-cache-sync/src/lib.rs                    |   10 +-
 crates/bd-cache-sync/src/mode_lock.rs              |   38 +-
 crates/bd-cache-sync/src/module_hook.rs            |   75 ++
 crates/bd-cache-sync/src/payload.rs                |   35 +-
 crates/bd-cache-sync/src/python_job.rs             |    5 +-
 crates/bd-cache-sync/src/run.rs                    |   94 +-
 crates/bd-cache-sync/src/schedule.rs               |  143 ++-
 crates/bd-cache-sync/tests/check_consumer_graph.py |  100 ++
 crates/bd-cache-sync/tests/common/mod.rs           |   56 +
 crates/bd-cache-sync/tests/common/repo.rs          |   24 +-
 .../bd-cache-sync/tests/common/session_fixture.rs  |  985 +++++++++++++++++
 crates/bd-cache-sync/tests/cron_hurry.rs           | 1126 +++++++++++++++++++-
 crates/bd-cache-sync/tests/cron_rows.rs            |    4 +-
 crates/bd-cache-sync/tests/e2e.rs                  |   84 ++
 crates/bd-cache-sync/tests/hook_runner.rs          |    4 +-
 crates/bd-cache-sync/tests/job_threading.rs        |   59 +
 crates/bd-cache-sync/tests/perf.rs                 |   75 ++
 crates/bd-cache-sync/tests/schedule_switch.rs      |  102 +-
 crates/bd-cache-sync/tests/test_consumer_graph.py  |  106 ++
 crates/sc-cron/Cargo.toml                          |   50 -
 crates/sc-cron/LICENSE                             |   21 -
 crates/sc-cron/README.md                           |  116 --
 crates/sc-cron/clippy.toml                         |    4 -
 crates/sc-cron/src/clock.rs                        |  331 ------
 crates/sc-cron/src/error.rs                        |  279 -----
 crates/sc-cron/src/expr.rs                         |  358 -------
 crates/sc-cron/src/hook.rs                         |  733 -------------
 crates/sc-cron/src/jitter.rs                       |  205 ----
 crates/sc-cron/src/job.rs                          |   46 -
 crates/sc-cron/src/lib.rs                          |  149 ---
 crates/sc-cron/src/limits.rs                       |   41 -
 crates/sc-cron/src/process.rs                      |  583 ----------
 crates/sc-cron/src/scheduler.rs                    |  734 -------------
 crates/sc-cron/src/sqlite.rs                       |  794 --------------
 crates/sc-cron/src/store.rs                        |  172 ---
 crates/sc-cron/src/timestamp.rs                    |  104 --
 crates/sc-cron/src/types.rs                        |  450 --------
 crates/sc-cron/src/worker.rs                       |  841 ---------------
 crates/sc-cron/tests/common/mod.rs                 |  626 -----------
 crates/sc-cron/tests/enabled.rs                    |  385 -------
 crates/sc-cron/tests/expr.rs                       |  312 ------
 crates/sc-cron/tests/process_api.rs                |  111 --
 crates/sc-cron/tests/python_hook.rs                |  280 -----
 crates/sc-cron/tests/reload_poll.rs                |  140 ---
 crates/sc-cron/tests/run_due_abort.rs              |   91 --
 crates/sc-cron/tests/sqlite_store.rs               |  962 -----------------
 crates/sc-cron/tests/stop_hook.rs                  |  416 --------
 crates/sc-cron/tests/target_resolver.rs            |  152 ---
 crates/sc-cron/tests/two_process.rs                |  252 -----
 crates/sc-cron/tests/worker.rs                     |  970 -----------------
 docs/architecture.md                               |   80 +-
 docs/bd-cache-api/requirements.md                  |    6 +-
 docs/bd-cache-cli/architecture.md                  |    8 +-
 docs/bd-cache-cli/requirements.md                  |    5 +-
 docs/bd-cache-py/architecture.md                   |    7 +-
 docs/bd-cache-store/architecture.md                |   18 +
 docs/bd-cache-sync/architecture.md                 |   52 +
 docs/plans/phase-c/anchors/anchors.json            |   11 +-
 docs/plans/phase-c/anchors/anchors.md              |    4 +-
 docs/plans/phase-c/phase-c-plan.md                 |   12 +-
 docs/plans/phase-c/sprint-c-1-runtime-migration.md |  504 ++++++++-
 docs/requirements.md                               |   10 +-
 docs/sc-cron/architecture.md                       |   10 +
 docs/sc-cron/requirements.md                       |   10 +
 99 files changed, 6982 insertions(+), 11024 deletions(-)

## Tests
- `cargo test --locked --workspace --exclude bd-cache-py --all-features`: exit 0, 1,179 reported pass, 0 fail, 21 existing ignored; doc tests included. Strict all-target/all-feature and default-feature Clippy plus `cargo fmt --all --check` passed.
- Causal tests passed with active-before/after proof: `cached_read_returns_without_bd_while_the_scheduled_python_sync_is_running` (cached runner argv 1→1) and `concurrent_cached_read_guard_rejects_a_real_cold_read_backend_lookup` (cold argv 1→2, rejected by the same guard); owned joins/reaping verified, no physical host bd/Dolt. The scheduled Python row used the managed PyStub shell interpreter and an OS child, not CPython.
- `crates/bd-cache-sync/tests/cron_rows.rs::a_sync_payload_on_another_row_is_refused_and_a_non_sync_row_coexists` passed: impostor payload refused, `plain.other` remains active/enabled, builtin succeeds. `crates/bd-cache-py/tests/test_run.py::test_the_hook_gets_every_result_marks_some_and_the_rest_is_offered_again` passed in the installed-wheel suite and asserts both scope results, failing result/error/failing_since keys, marking, reoffer, first-offered time and notification flags.
- Installed abi3 wheel Python suite: 226/226 pass, including 7 neutral corpus and 7 hook cases; Python strict Clippy/Rust tests 23/23. Graph unit tests 8/8; API/Python graph, server-negative expected exit 1, boundary, no-backend and consumer-lint checks passed.
- Existing release `migrated_incremental_sync_of_1000_beads_is_under_one_second`: 1 pass, 6.1545 ms against its 1,000 ms fixture threshold; no comparative benchmark claim.
- Initial sandbox attempts (metadata and workspace tests) exited 101; identical authorized native commands passed. No source/timer/fault changes. The sandbox result alone does not establish a lower-level cause.
