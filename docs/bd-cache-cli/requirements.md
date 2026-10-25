# bd-cache-cli Requirements

Status: Draft (sprint b-3a). Crate-local requirements (domain code `CLI`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Contract:
[`contract.md`](contract.md). Created by sprint b-3a, which owns the harness
requirements below (`REQ-CLI-0001`..`0005`); sprint b-3 adds the behaviour
requirements to this file.

Test files are under `crates/bd-cache-cli/tests/`; the shared harness is
`tests/support/mod.rs`.

## REQ-CLI-0001: The crate builds on all three CI OSes with sc-command pinned

**Status:** Draft

`bd-cache-cli` builds on ubuntu, macOS and Windows with sc-command taken as a
git dependency at sc-runtime tag `prerelease/v0.1.0` (`ba971c1`), and
`bdc --version` prints one sc-command envelope: `ok: true`,
`data: {name, version, contract_version}`, one line, a trailing newline.
Any other argv in this build is a `BD_CACHE.USAGE` envelope with exit 2, with
`details.valid`.

**Refines:** REQ-BDC-0805 (contract §1 rule 1, §3, §8)
**Acceptance evidence:** `version_is_one_envelope_with_ok_true_and_a_name`,
`plain_text_version_fails_the_envelope_parse`,
`usage_errors_are_envelopes_with_exit_2` (`tests/cli.rs`); the CI matrix.

## REQ-CLI-0002: `bd-cache` and `bdc` are one program

**Status:** Draft

The two binaries call one entry point, `bd_cache_cli::run_process`, and give
the same exit code, stdout and stderr for the same argv. The `name` in
`--version` is `bd-cache` for both and never `argv[0]`.

**Refines:** REQ-BDC-0805
**Acceptance evidence:** `bd_cache_and_bdc_give_identical_output_for_the_same_argv`,
`the_identity_check_detects_a_divergent_entry_point` (`tests/cli.rs`).

## REQ-CLI-0003: `run()` is the in-process entry point at its final signature

**Status:** Draft

`pub fn run(argv, env, invoker, clock) -> (i32, String)` returns the exit
code and the one-line envelope the process would print. `argv` has no program
name; `env` is a map; `invoker` is `Option<Box<dyn CliInvoker>>` (`None` = the
real `bd`); `clock` is `Arc<dyn Clock>`. The signature does not change in
sprint b-3.

**Refines:** contract §8 "Testing", layer "In process"
**Acceptance evidence:** `run_in_process_matches_the_real_process`
(`tests/cli.rs`); the doctests of `run`.

## REQ-CLI-0004: `fake-bd` is a stateful bd across processes

**Status:** Draft

The `fake-bd` executable runs sc-bd-cli's `FakeBd` behind the `bd` argv. It
reads its state from the scenario file named by `$FAKE_BD_SCENARIO` and writes
the rows back after each call that changed one, so a sequence of processes
sees one consistent bd. A scripted sequence (list, update, list) gives the
same rows through `BdCli` over the `fake-bd` process as through an in-process
`FakeBd`. The clock is the scenario's fixed `now`. A scenario that cannot be
read is exit 70, outside the exit codes of a real bd.

**Refines:** contract §8 "Testing", layer "Real process"; NFR-BDC-0005
**Acceptance evidence:**
`list_update_list_gives_the_same_rows_through_the_process_and_in_process`,
`a_fake_bd_that_forgets_its_writes_fails_the_sequence`,
`the_scenario_clock_is_fixed_across_processes`,
`version_schema_and_the_blocked_set_come_from_the_scenario`,
`a_missing_or_broken_scenario_is_a_harness_error_not_a_bd_answer`
(`tests/harness.rs`).

## REQ-CLI-0005: The harness injects faults, and a release build carries none of it

**Status:** Draft

A scenario can mark bd unavailable, slow by N ms (a real delay) or refusing
writes with an exit code. `BdCli` over `fake-bd` returns the unreachable error
for an unavailable scenario and times out for a delay longer than the call
timeout. The `fake-bd` binary has `required-features = ["fake-bd"]`;
`cargo build --release --bin bd-cache --bin bdc` does not build it and the
two artifacts contain no `test-support` code. The crate is `publish = false`.

**Refines:** contract §8 "Testing"; ADR-CLI-0001
**Acceptance evidence:**
`an_unavailable_scenario_makes_bdcli_return_the_unreachable_error`,
`slow_by_500_ms_with_a_100_ms_call_timeout_times_out`,
`a_refused_write_carries_its_exit_code_and_changes_nothing`
(`tests/harness.rs`); `fake_bd_is_a_gated_bin_and_the_crate_is_not_publishable`,
`an_ungated_fake_bd_fails_the_metadata_check`,
`a_release_build_has_no_fake_bd_and_no_test_support_code`
(`tests/release_hygiene.rs`).
