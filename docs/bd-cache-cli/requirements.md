# bd-cache-cli Requirements

Status: Draft (sprints b-3a, b-3). Crate-local requirements (domain code `CLI`)
refining [`../requirements.md`](../requirements.md). Architecture:
[`architecture.md`](architecture.md). Contract:
[`contract.md`](contract.md). Created by sprint b-3a, which owns the harness
requirements below (`REQ-CLI-0001`..`0005`); sprint b-3 adds the behaviour
requirements (`REQ-CLI-0006`..`0012`).

Test files are under `crates/bd-cache-cli/tests/`; the shared harness is
`tests/support/mod.rs`.

## REQ-CLI-0001: The crate builds on all three CI OSes with sc-command pinned

**Status:** Draft

`bd-cache-cli` builds on ubuntu, macOS and Windows with sc-command taken as a
git dependency at sc-runtime tag `prerelease/v0.1.0` (`ba971c1`), and
`bdc --version` prints one sc-command envelope: `ok: true`,
`data: {name, version, contract_version}`, one line, a trailing newline.
An argv-shape error is a `BD_CACHE.USAGE` envelope with exit 2, with
`details.valid` (REQ-CLI-0007).

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

## REQ-CLI-0006: Every command of both surfaces runs its bd-cache-api operation

**Status:** Draft (sprint b-3)

Every command of contract §5.2 and §6 turns its argv into the command's JSON
request, which goes to `bd_cache_api::decode` and then `execute`; `--input
FILE|-` gives the request whole. `cache tick` runs `bd_cache_sync::tick`, each
`refused[]` item an `OpError` through `ApiError::tick_refusal`. A `/v0`
operation the cache does not serve (contract §5.4) is `UNSUPPORTED`
(`details.unsupported: "operation"`), exit 3. b-2's shared fixtures replay
through `run()` with the envelopes they expect.

**Refines:** REQ-BDC-0801, contract §1 rule 6
**Acceptance evidence:** `tests/fixtures.rs` (b-2's five files via `--input`;
`cli_commands.json`, every command as argv;
`every_contract_command_has_an_argv_step`).

## REQ-CLI-0007: Exit codes and one envelope on stdout

**Status:** Draft (sprint b-3)

Exit 0 is `ok: true`; exit 2 is `BD_CACHE.USAGE` for the shape of argv only
(an unknown command or flag, an abbreviated long option, a missing or
conflicting argument, `--input` mixed with request flags, a bare `cache
refresh`), with `details.did_you_mean` and `details.valid`; exit 3 is any
other typed error. stdout is exactly one envelope, unless `--format text` is
given; `USAGE` is JSON even then. A caught panic is `BD_CACHE.INTERNAL`.

**Refines:** REQ-BDC-0802, contract §3, §4
**Acceptance evidence:** `exit_0_is_ok_true`, `exit_3_is_any_other_typed_error`,
`exit_2_is_usage_for_the_shape_of_argv_only`, `usage_is_json_even_with_format_text`,
`a_caught_panic_is_internal_with_exit_3` (`tests/process.rs`).

## REQ-CLI-0008: Configuration precedence and file locations

**Status:** Draft (sprint b-3)

flag > `BD_CACHE__*` env > `.beads/bd-cache.toml` > default. `--bd` is never
read from TOML (a `bd` key there is ignored with a warning); `--format` comes
from the flag only. `.beads/bd-cache.toml` is read from `<repo>/.beads/`,
`metadata.json` from `$BEADS_DIR` when set. There is no upward search. The
legacy `.atm-bd/<db>.bd-cache.sqlite3` with no `cache_path` and no default
file is `CONFIG_INVALID` with `details.key: "cache_path"` and
`details.legacy_path`. The merged configuration is opened through
`bd_cache_sync::compose_with_config`.

**Refines:** REQ-BDC-0810, contract §2
**Acceptance evidence:** `flag_beats_env_beats_toml_beats_default`,
`bd_is_never_read_from_toml`, `a_subdirectory_does_not_find_the_parent_beads`,
`the_legacy_cache_file_is_config_invalid`, `metadata_comes_from_beads_dir_when_set`
(`tests/process.rs`); `config::tests`.

## REQ-CLI-0009: `--help` and `--version`

**Status:** Draft (sprint b-3)

`--help` prints text when stdout is a terminal and no `--format` is given,
and otherwise the envelope with `data: {command, synopsis, arguments: [{name,
flag, required, description}]}`; exit 0. `--version` returns `{name, version,
contract_version}` and follows `--format`.

**Refines:** contract §8
**Acceptance evidence:** `help_when_piped_is_the_envelope`,
`help_on_a_terminal_is_text` (Unix, through `script(1)`),
`text_help_where_the_envelope_is_due_fails_the_parse` (`tests/process.rs`).

## REQ-CLI-0010: The stderr log

**Status:** Draft (sprint b-3)

Logs go to stderr at `warn` by default; `-q` keeps errors, `-v` adds `info`
(more `v` for more), and either flag wins over `BD_CACHE__LOG`. stdout never
changes with the level. A `BD_CACHE__*` key bdc does not read is logged as
ignored.

**Refines:** contract §1 rule 1, §2
**Acceptance evidence:** `logs_go_to_stderr_and_never_change_stdout` (`tests/process.rs`).

## REQ-CLI-0011: The named negative scenarios pass through the process

**Status:** Draft (sprint b-3)

Each scenario of contract §8 "Testing" runs as real `bdc` processes against
`fake-bd`: an outage during a write, an escalation at flush, a partial
`refresh scopes`, `fresh_within` running out, a schema change, a cache file
of another project, a retry after a lost reply.

**Refines:** contract §8 "Testing"
**Acceptance evidence:** `tests/scenarios.rs`, one test per scenario.

## REQ-CLI-0012: A new-process `get` is under 50 ms

**Status:** Draft (sprint b-3)

In a release build, the median and the p95 of 50 new-process `bdc issues get`
cache hits with nothing pending on the scope are under 50 ms, with `fake-bd`
delayed by 60 ms per call (so no bd call fits). The same with 10 pending
entries in a 1,000-row scope is reported, not gated.

**Refines:** NFR-BDC-0002
**Acceptance evidence:** `a_new_process_get_is_under_50_ms` (`tests/latency.rs`,
CI step "Latency (NFR-BDC-0002, release profile)").
