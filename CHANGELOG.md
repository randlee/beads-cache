# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
All crates in the workspace share one version.

## [Unreleased]

## [0.2.0] - unreleased

The `bdc` CLI, the `bd-cache-api` crate, Python 0.2 and the release channels
(phase b; contract `docs/bd-cache-cli/contract.md`, version 1.0.0).

### Added

- **`bd-cache` / `bdc` CLI** (crate `bd-cache-cli`, two binaries, one program),
  built on sc-command.
  - Two command groups. Beads: `issues get|list|update|close`, `ready`,
    `dependencies list|add`, `context`. Cache management: `cache
    status|health|scopes|refresh scopes|refresh issues|flush|evict|journal
    get|list|resolve|announce|tick`.
  - Beads commands match bd serve `/v0` in vocabulary only, not in behaviour.
    Every reply carries `x_cache`; a write reply says it is journaled, not
    applied.
  - Every command, `--version` and every failure prints one sc-command
    envelope on stdout (`{version, ok, data, error}`); logs go to stderr.
    Exit codes: 0 success, 2 `BD_CACHE.USAGE` (argv shape only), 3 any other
    typed error.
  - Configuration precedence is flag > `BD_CACHE__*` env > `.beads/bd-cache.toml`
    > default. `--bd` is never read from TOML. `--format text` is flag-only.
    There is no upward search for `.beads`.
  - Latency (NFR-BDC-0002): a new-process `get` cache hit, with nothing pending
    on the scope, is under 50 ms median and p95 (release build, `fake-bd`
    delayed 60 ms per call). CI gates it on Unix; on hosted Windows runners
    it is reported, not gated (S78, S79).
- **`bd-cache-api`** (internal, not published): the request, data and error
  vocabulary shared by every front end. One decoder (`UNSUPPORTED` and
  `BAD_ARGS` come from it), the operation layer, the error table and schemars
  schemas. It depends on neither sc-command nor clap. `CONTRACT_VERSION` is
  `1.0.0`.
- **Python 0.2** (`bd-cache-rs` 0.2.0): `cache.beads.*` and `cache.manage.*`
  over `bd-cache-api`, with milliseconds for timeouts. Additive: nothing in
  0.1 changes and 0.2 issues no `DeprecationWarning`.
  `bd_cache_rs.CONTRACT_VERSION` is the contract version the new namespaces
  implement.
- **Core additions** (`bd-cache`, `bd-cache-model`, `bd-cache-store`):
  `write_returning` (one transaction that returns the entry and the views
  before and after the write), `entry_by_key`, `journal_counts`,
  `GetReply.scope`, and `bd_cache_sync::compose_with_config`.
- **Store columns**: the additive nullable columns `last_error_code`, `note`
  and `changed` on the journal. `PRAGMA user_version` stays 3, and
  the v3 Python writer still reads the file. `WriteOutcome.changed` is
  `Option<bool>`, `None` when unknown, never guessed (S62, S68).
- **Release channels**: `bd-cache` and `bdc` binaries for Linux, macOS (arm and
  intel) and Windows on GitHub Releases, plus Homebrew (`randlee/tap`), Scoop
  (`randlee/scoop-bucket`) and WinGet (`randlee.bd-cache`). The stable channels
  go live with the first stable release, which waits for
  randlee/sc-publish#135.

### Changed

- **One workspace version, 0.2.0** (S61). `sc-beads`, `sc-cron`, `sc-bd-cli`
  and the wheel all move to 0.2.0.
- **Error codes** (S58, ADR-MODEL-0002; recorded under the phase-end fixes
  below) and **S68**: `changed` is unknown (`None`) rather than predicted for
  an entry stored without it, unless the entry is still pending.
- **Refresh outage (S75)**: `cache refresh scopes` reports a scope whose sync
  could not reach bd as a failed result (`BD_UNAVAILABLE`, retryable), counted
  in `data.failed`, even though the core returned a report with an `outage`.
  `cache refresh issues` fails as a whole with `BD_UNAVAILABLE`. The core is
  unchanged.

### Security

- `sc-bd-cli` (S76, S77): the `PATH` given to every `bd` child keeps only absolute
  directories. Empty segments (an unset or empty `PATH`, or a leading,
  trailing or doubled `:` / `;`), relative ones (`.`, `bin`, `./x`) and
  fallback directories under a relative or empty `HOME` / `GOPATH` (on
  Windows `USERPROFILE` / `LOCALAPPDATA`) are dropped. Each of them meant the
  working directory, which is the repository root, so a bare `bd` could run a
  committed `<repo>/bd` (QA RCR-002, lead rulings S76 and S77).
- `sc-bd-cli` **behaviour change**: the inherited `PATH` now comes first and
  the fixed install directories (Homebrew, Linuxbrew, `/usr/bin`, `/bin`,
  `~/.local/bin`, `~/bin`, `$GOPATH/bin`, `~/.cargo/bin`, and the Windows
  equivalents) follow it as a fallback, where they used to come first. A `bd`
  named by the user's `PATH` now wins over one in a fixed directory, and each
  directory appears once (lead ruling S77, contract §2).
- `sc-bd-cli` (RSH-004, lead ruling S81): every deadline-bounded `bd` call
  also holds a cross-process advisory lock, `<project>/.beads/sc-bd-cli.lock`
  (or a file in the OS temp directory keyed by the canonical project
  directory), so two processes, such as a cron `cache tick` and an agent's
  `bdc get`, never run two `bd` on one embedded Dolt database. The wait is
  bounded by the call's deadline (`TimeoutPhase::Lock`) and the lock is
  released on every return and when the process dies (ADR-BDCLI-0006).
  Only an embedded-Dolt project takes it: one whose `.beads/metadata.json`
  says Dolt server mode (`dolt_mode: "server"`) runs concurrent `bd` by design
  and takes none (a missing or unreadable file counts as embedded). The
  protection is partial: agents and bd hooks that run `bd` directly do not
  take the lock. Add `sc-bd-cli.lock` to `.beads/.gitignore`.
- `sc-bd-cli`, `bd-cache-sync` (RSH-002): the timeout kill of a `bd` or messaging-command
  process group is `kill(-pgid, SIGKILL)` itself (`sc_bd_cli::process_group`),
  not the `kill` program, so a dolt grandchild dies with the group in an
  image without `/bin/kill` or with no process slot left.
- `bdc` (RSH-001): SIGTERM, SIGINT and SIGHUP no longer leave the `bd` / dolt
  group running. The running `bd` call's group is killed and reaped, the
  locks are released, `cache tick` and its messaging-command child stop, the envelope is
  written, and bdc exits `128 + signal` (143 for SIGTERM; contract §3,
  ADR-CLI-0005). On
  Linux a `bd` child also gets `PR_SET_PDEATHSIG`. New:
  `sc_bd_cli::shutdown`, `BoundedError::Interrupted`.
- `bd-cache-sync` (RSH-003): the messaging command's sender (removed in sprint b-8) searches
  the absolute directories of `PATH` only, so a messaging command committed in the working
  directory never runs for `:/usr/bin` or `.:/usr/bin` (S76/S77 applied to it).
- `sc-bd-cli`: a `PATH` with a segment that is not UTF-8 no longer empties the
  inherited `PATH`; the child gets every segment.

### Phase-end fixes

Phase-end fixes to the 0.1.0 code described below.

- `bd_cache_sync::compose_with(repo_root, backend, locks, clock, beads_dir_env)`:
  the backend is now built from the lock set
  (`backend: impl FnOnce(Arc<ProjectLocks>) -> BdCli`, `locks: Arc<ProjectLocks>`),
  and `compose()` shares one process-wide lock set. A non-UTF-8 repository root
  is refused. `bd-cache-sync` is internal (not published).
- `SyncJob`: after a panic the cache is dropped, not reused. `with_recompose`
  rebuilds it on the next tick, and a job without one reports `sync_cache_lost`.
  `with_cache` returns `Result<T, CacheLost>` and `try_with_cache` returns
  `Result<Option<T>, CacheLost>`. The new `join_idle(deadline)` waits for an
  in-flight tick at shutdown.
- Error codes, recorded in ADR-MODEL-0002 (approved, lead ruling S58). These reclassify existing triggers:
  - A second open of a cache file in one process is the new
    `BD_CACHE.STORE_IN_USE` (it was `STORE_FAILED`).
  - Unreadable or oversized `bd` output is the new
    `BeadsCacheError::Transport`, mapped to `BD_LIST_FAILED`, `BD_WRITE_FAILED`
    or `SYNC_FAILED` (it was `BD_CONTRACT`).
  - A guarded write to a bead `bd` no longer has is
    `BeadsCacheError::NotFound` and escalates as `NOT_FOUND`.
- Validation:
  - `bd-cache.toml` refuses zero timeouts and `max_attempts = 0`.
  - A new write refuses an empty or `=` metadata key, and args over 256 KiB, an
    idempotency key over 1 KiB or a note over 16 KiB, all as `BAD_ARGS` before
    anything is journaled. Stored entries are not re-checked.
- `sc-bd-cli`:
  - A timed-out `bd` takes its whole process tree with it before the project
    lock is released: the process group on Unix, a job object on Windows.
  - Reaping is bounded.
  - The `--version` probe has a 10 s bound.
  - Project locks key on the canonical directory.
- `sc-cron`: an undecodable schedule row is skipped with a warning instead of
  stopping every schedule.

### Boundary extension: health, the neutral tests and the lint gate (sprint b-9)

- **`cache health`** (REQ-BDC-0732, #86): `reasons[]` for every degraded cause; the oldest
  pending entry's age (`oldest_pending_age_s`, `--max-pending-age-s N`, default 300 s), the last
  drain (`last_drain_at`, the newest applied time the journal holds, an index read) and the
  unresolved escalations; and the sync's state (`sync: {mode, last_run}`) with the reasons
  `no_sync_row_enabled`, `sync_stale` (three periods of the enabled row), `sync_failing` (three
  failed runs) and `hook_failing` (three hook failures, the count from which the hook is backed
  off), the same from `bdc` and `cache.manage.health()`: both run through
  `bd_cache_sync::execute_api`, which gives health the sync rows' state (ADR-BDC-0015
  amendment 1). A cache file with no sync row at all is `degraded` with `no_sync_row_enabled`;
  sync rows that cannot be read are `degraded` with `sync_state_unreadable`, not a failed health.
  The oldest pending age is read from the store's bounded summary, never the whole backlog.
- **`cache refresh scopes --timeout-ms N`** (NFR-BDC-0004): each scope's sync deadline, 1 to
  600000 ms (`0` or more than 600000 is `BAD_ARGS`, as for `cache refresh issues --timeout-ms`); every bd
  call is killed at its timeout capped by the time left.
- **Scope selection**: a 0.2 `issues list` or `ready` names its scope with `scope` only; no
  scope is derived from a legacy-prefixed label (ADR-BDC-0013, contract §5.1).
- **The contract** (`docs/bd-cache-cli/contract.md`) states what the phase-B extension built:
  the scope filters, the sync rows, the switch, the post-sync hook and the new verbs (§2, §4,
  §6), the 0.1 → 0.2 API differences (§10) and the classification of the extension's changes as
  pre-release edits of the unreleased 1.0.0 (§11).
- **Tests are neutral** (REQ-BDC-0812, ADR-STORE-0006, ADR-PY-0006): the vendored compat
  implementation, its call log and the mapping test are removed; a 0.1 call corpus generated in
  this repository against the fake bd (`crates/bd-cache-py/tests/corpus/`) pins every 0.1 verb,
  reply, error and bd argv, the 0.1 bare-name rule included. A CLI outage scenario shows
  `issues get` answering from the cache while the outage is recorded for the scope.
- **CI** (ADR-BDC-0010 as amended by ADR-BDC-0012 and its Amendment 2): the step "Strict
  consumer boundary (ADR-BDC-0010)" scans every tracked file except a declared out-of-scope
  list (agent tooling, the sc-publish kit, `.beads/` tracker data, this changelog,
  `Cargo.lock`), plans included, with no exemption inside the scope but the lint, its tests and
  the team protocol, in the lint job and on the three OSes; the lint gains the
  maintainer-handle rule, matches the scope prefix before any format or interpolation marker,
  reads UTF-16 text and symlink targets, reports an unreadable file instead of skipping it, and
  narrows `scope-prefix` for the one `LEGACY_SCOPE_PREFIX` declaration. The unit-test jobs
  assert that no `bd` or `dolt` is on `PATH` (NFR-BDC-0005), and the lint job that no crate
  depends on a Dolt or MySQL client (REQ-BDC-0006), both through `scripts/check_no_backend.py`.
  The compat test's `uv` setup is gone.

## [0.1.0] - unreleased

The first release: the beads SQLite cache extracted from its first
consumer (BD-CACHE v3), as Rust crates and Python
bindings. `sc-beads`, `sc-bd-cli` and `sc-cron` go to crates.io and
`bd-cache-rs` to PyPI; the other crates are internal (`publish = false`).

### Added

- **sc-beads**: the beads backend contract.
  - btit's `BeadsBackend`, `CliBackend`, `DoltOperations` and
    `CloseSuggestions` traits, `BeadsError` and data types, ported unchanged
    from beads-task-issue-tracker at `8d8e82ae9f85`; the `log_*!` macros emit
    through `tracing`.
  - `CacheBackend: BeadsBackend`: `fingerprint`, `list_by_ids`,
    `list_scope`, `list_blocked`, `list_ready`, `close_guarded`,
    `update_guarded` and `dep_add_typed`, each bounded by a `Deadline` and
    returning `BeadsCacheError`, with validated value types (`BeadId`,
    `Actor`, `Assignee`, `Patch`, `Guards`, `Sha256Hex`, `Timestamp`, ...).
  - Strict rows (`Bead`, `BlockedRow`) generated from bd 1.3.1's `bd schema`;
    a violation names the bead and the field path.
  - `Clock`, `SystemClock`, `Deadline` and RFC 3339 helpers.
  - Feature `testing`: `ManualClock`, `FakeBackend` and the `CacheBackend`
    contract suite (`backend_contract_tests!`).
- **sc-bd-cli**: the `bd` CLI implementation of the sc-beads traits.
  - `BdCli` implements btit's traits as btit-bd does, and `CacheBackend` with a
    hard per-call deadline that also covers the per-project lock wait.
  - Error classification: exit 13 is `Refused(Precondition)`; a timeout or an
    unreachable server is `Unavailable`; a `bd` that cannot start is
    `Btit(Spawn)`, also reported by `is_unavailable()`.
  - `CliRunner` / `CliInvoker`, `ProjectLocks`, Dolt detection, and
    `with_child_var` for the environment of every spawned `bd`.
  - Feature `test-support`: `RecordingInvoker` and `FakeBd`.
- **sc-cron**: a SQLite-backed cron scheduler for tokio, extracted from
  the first consumer's cron scheduler, with no consumer or bd types.
  - Validated schedule specs, a future-occurrence check before anything is
    written, a fixed jitter per schedule, missed-tick `Coalesce` (default) or
    `Skip`, a stop check before every run, run timeouts on the monotonic clock,
    and `(id, generation)` fencing.
  - `SqliteScheduleStore` on its own database file; `list_active` skips a row
    that does not decode and keeps the other schedules running.
  - Feature `test-utils`: `ManualClock` and `SeqJitter`.
- **bd-cache-rs** (crate `bd-cache-py`, module `bd_cache_rs`): Python bindings
  built with maturin / PyO3, one abi3 wheel per platform for CPython 3.9 and
  later.
  - `bd_cache_rs.Cache` over the real SQLite store, `flock` drain lock and
    `BdCli`, with the verbs the first consumer uses: `get`, `refresh`, `write`, `list`,
    `ready`, `sync`, `drain`, `entry`, `resolve`, `prune`, `status`, and the
    announcement state `unannounced_escalations`, `mark_announced`,
    `sync_alert`, `set_sync_alert` and `open_work`.
  - Replies are builtin dicts and lists; every `BD_CACHE.*` code raises
    `bd_cache_rs.CacheError`; bad arguments raise `ValueError`.
  - The handle is usable from any thread and releases the GIL while a verb
    waits on `bd` or SQLite.
- **Internal crates** (not published):
  - `bd-cache-model`: journal ops, replay, the store and sender ports,
    `.beads/bd-cache.toml` configuration, replies and `CacheError`.
  - `bd-cache-store`: the SQLite `CacheStore` and the cross-process drain lock.
  - `bd-cache`: the `Cache` facade. Reads never wait on `bd`; writes are
    journaled with idempotency keys; the drain re-checks preconditions and
    reads back; `ready` adjusts bd's base for journaled writes without a `bd`
    call, with an optional bounded fresh answer; a slow or unreachable backend
    leaves writes pending instead of escalating them.
  - `bd-cache-sync`: the sync task on `sc-cron` (drain, then sync each active
    phase), its announcements, and `compose`, which builds the real cache of a
    repository.

### Schema and file versions

- The cache file is the BD-CACHE v3 file: `PRAGMA user_version` 3
  (`bd_cache_store::SCHEMA_VERSION`), shared with the baseline Python implementation.
  Additions are new tables and nullable columns only; a file at another
  nonzero version is refused, and the version is never changed on open.
- The strict rows are pinned to bd 1.3.1, `bd schema` version 1, sha256
  `caf8fe18e1117c067ff7bbd824484b24849677e2cbf86d793ddf6b64069a31a2`
  (`sc_beads::BD_VERSION`, `BD_SCHEMA_VERSION`, `BD_SCHEMA_SHA256`). A bd
  change that breaks the row contract is released as a semver-major version
  of `sc-beads` and `sc-bd-cli`.
- `sc-cron`'s schedule store is at `user_version` 1 (`sc_cron::SCHEMA_VERSION`).
- The Python reply shapes are versioned with the wheel (`bd_cache_rs.__version__`).

### Not included

- The `bdc` / `bd-cache` CLI binary, and with it any Homebrew, Scoop or WinGet
  package (added in 0.2.0).
- The HTTPS MCP server and the `bd serve` HTTP backend (`sc-bd-http`).
- Scope kinds other than `phase-<x>`, and user metadata schema packages
  (ADR-BDC-0008, draft).

[Unreleased]: https://github.com/randlee/beads-cache/commits/develop
[0.2.0]: https://github.com/randlee/beads-cache/commits/develop
[0.1.0]: https://github.com/randlee/beads-cache/commits/develop
