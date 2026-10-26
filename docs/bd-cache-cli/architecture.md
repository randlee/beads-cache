# bd-cache-cli Architecture

Status: Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25; written in sprints b-3a, b-3). Crate-local decisions (domain code `CLI`).
Repo-level context: [`../architecture.md`](../architecture.md) (ADR-BDC-0009
puts the crate in the Boundaries table). Requirements:
[`requirements.md`](requirements.md). Contract: [`contract.md`](contract.md).
Created by sprint b-3a.

`bd-cache-cli` holds the binaries `bd-cache` and `bdc` and the `fake-bd` test
harness. Sprint b-3a shipped the skeleton; sprint b-3 adds the commands over
`bd-cache-api`.

| Module / target | Contents |
|---|---|
| `lib` | `run(argv, env, invoker, clock) -> (i32, String)`, `run_with(…, Stdout)`, `run_process()`, `NAME`, `CONTRACT_VERSION`, `EXIT_USAGE`, `EXIT_ERROR`; dispatch: `--help`, `--version`, unserved operations, clap, request, command, render; the caught panic |
| `args` | The one table of commands (`CommandSpec`, `ArgSpec`, `GLOBALS`), the clap tree built from it, the word pre-scan |
| `request` | argv matches to the JSON request at each argument's member path; `--input FILE\|-` |
| `config` | flag > env > TOML > default, the `bd` and legacy-file rules, `metadata.json` from `$BEADS_DIR` |
| `commands` | decode, open (`compose_with_config`), `execute`; `beads` and `manage` hold their surface's rows (and `beads` the unserved operations); `tick` decodes and runs `cache tick`; `signal` takes over SIGTERM / SIGINT / SIGHUP (ADR-CLI-0005) |
| `render` | `json`: the sc-command envelope; `text`: `--format text` |
| `help` | `--help` as `{command, synopsis, arguments}` or text |
| `log` | the stderr `tracing` subscriber and the level rule |
| `scenario` (feature `fake-bd`) | `Scenario`, `Faults`, `RefuseWrite`, `serve`: the scenario file and one `fake-bd` invocation |
| `bin/bd-cache`, `bin/bdc` | `fn main() -> ExitCode { run_process() }`, identical |
| `bin/fake-bd` (feature `fake-bd`) | reads `$FAKE_BD_SCENARIO`, calls `serve`, prints, exits |
| `tests/support` | `TestRepo` (temp repo with `.beads/metadata.json` and a scenario), `parse_envelope`, process and in-process `BdCli` constructors |

## ADR-CLI-0001: sc-command is a git dependency, so the crate is `publish = false`

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

Context: the CLI replies in sc-command's `Envelope` (contract §4, ADR-BDC-0009).
sc-command is in randlee/sc-runtime and is not published to crates.io; a crate
with a git dependency cannot be published. The three crates that are published
(`sc-beads`, `sc-bd-cli`, `sc-cron`) have no such dependency, and CI dry-runs
their publication.

Decision:

- sc-command is a git dependency on `https://github.com/randlee/sc-runtime`
  at the stable tag `v0.1.0` (`ae3e10b`), declared once in
  `[workspace.dependencies]`. PR #84 moved the pin there from
  `prerelease/v0.1.0` (`ba971c1`) at the sc-runtime team's request; the crates
  at the two tags are byte-identical. A tag, not a branch, so the build does not move.
  If the tag fails to build on a CI OS, the dependency is pinned by `rev`
  instead and the failure is raised with the sc-runtime team; there is no local
  fork (phase b plan, P-7).
- `bd-cache-cli` is `publish = false`. The CLI ships as binaries (GitHub
  releases and package channels, sprint b-5), not as a crate.
- The crates other than this one (`bd-cache-api` and `bd-cache-py` especially)
  take neither sc-command nor clap, so the published and bound surfaces are
  unaffected. CI checks it.
- `sc-observability-types` (the type of an sc-command error code) is a normal
  crates.io dependency at `1.5`, the version sc-command itself uses.
- `Cargo.lock` is committed, so the git revision is fixed by the lock as well.

The pin was checked first on all three CI OSes (ubuntu, macOS, Windows),
and on a Windows machine over SSH, before the harness was built on it.

Consequences: a release needs network access to GitHub for the dependency.
Moving the pin is a deliberate edit to `Cargo.toml` and `Cargo.lock`. cargo
prints one line, `error: invalid key-value pair, expected key`, for the
`template/` directory of the sc-runtime checkout while it resolves the
dependency; it is cargo's note about a file that is not a manifest, the command
still exits 0, and it is not ours to fix.

## ADR-CLI-0002: `fake-bd` is a feature-gated binary over `FakeBd` with scenario-file state

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

Context: contract §8 "Testing" needs a real-process simulator: tests run `bdc`
as a process with `--bd <fake-bd>`, and the NFR-BDC-0002 latency check does too.
sc-bd-cli already has a stateful in-process `FakeBd` over `FakeBackend`
(feature `test-support`), which is the model of bd the rest of the tests use.

Decision:

- `fake-bd` runs that same `FakeBd`; it does not model bd a second time.
- The binary has `required-features = ["fake-bd"]`, and the feature enables
  `sc-bd-cli/test-support` and `sc-beads/testing`. A **self dev-dependency**
  with the feature (`bd-cache-cli = { path = ".", features = ["fake-bd"] }`)
  turns it on for `cargo test`, so the integration tests can use
  `CARGO_BIN_EXE_fake-bd`. A release build (`cargo build --release --bin
  bd-cache --bin bdc`) does not build dev-dependencies and does not build
  `fake-bd`. The boundary check does not count a crate's dev-dependency on
  itself as an edge.
- The state is a file, so that a sequence of processes sees one bd. It is
  loaded on every start and written back (temporary file, then rename) after
  a call that changed a row. Calls are sequential; there is no file lock.
- Faults are fields of the scenario, not flags: they apply to every call of
  the scenario, and a test changes them by rewriting the file.
- The harness code that is not the binary (`scenario`) is part of the library
  behind the feature, so the tests build scenarios with the same type that
  `fake-bd` reads, and a typo in a hand-written scenario is an error
  (`deny_unknown_fields`).

Consequences: `fake-bd` answers only the command forms `FakeBd` serves (the
cache's calls, `version`, `schema`; `--version` as `version`). A bd command it
does not serve is exit 2, as a usage error.

## ADR-CLI-0003: One command table; argv becomes the request the decoder reads

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

Context: contract §1 rule 6 puts validation in `bd-cache-api`'s decoder, so
every front end returns the same code for the same input; contract §3 keeps
`USAGE` for the shape of argv.

Decision:

- Each command is one row (`CommandSpec`): path, target, and arguments, each
  naming the `/v0` request member it fills (`patch.status`,
  `edges.0.issue_id`). The clap tree, the request builder and the help come
  from the rows.
- The CLI checks only the shape of argv: clap's required arguments, unknown
  or abbreviated flags (`infer_long_args` off), a member filled twice, and
  `--input` with request flags are `USAGE`. Values go to the decoder as
  given: an integer flag that does not parse is passed as text, so the
  decoder answers `BAD_ARGS` naming the member.
- With `--input`, clap's `required` is lifted and the file is the request.
- The words of argv are pre-scanned for `--help`, `--version` and the `/v0`
  operations the cache does not serve, which are `UNSUPPORTED` before clap
  sees them (`bdc ready count`, `bdc issues create`).
- The request is decoded before the cache is opened.

Consequences: a request member the decoder learns is a flag with one row;
`--help` cannot drift from the grammar.

## ADR-CLI-0004: Configuration is merged here and opened by bd-cache-sync

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25; phase B closed, 2026-10-25)

Context: contract §2 needs `BD_CACHE__*` env overrides and flags over
`.beads/bd-cache.toml`, never reads `bd` from the file, and refuses the
legacy file. b-1 added `bd_cache_sync::compose_with_config` for a merged
configuration; b-3 adds `compose_with_config_and_backend` for a merged
configuration with a caller's backend (S72).

Decision:

- `config` reads `<repo>/.beads/bd-cache.toml` by the store loader's rules
  (missing is the defaults, unknown keys and bad scopes refused with the
  store's `store_config_*` codes), except that a top-level `bd` key is
  dropped with a warning: a checked-in file cannot choose an executable.
- Env overrides use sc-config's naming rule; a value that does not parse is
  `CONFIG_INVALID` with `details.source: "bdc_env_invalid"`; the legacy file
  is `CONFIG_INVALID` with `details.source: "bdc_legacy_cache_file"`. The
  merged configuration is validated (`RepoConfig::validate`, scopes) and
  opened through `compose_with_config` with `$BEADS_DIR` from the given
  environment. `ComposeError` maps through `ApiError::config_invalid` (or the
  cache error's own code).
- An injected `invoker` (`run`'s test seam, feature `fake-bd`) is opened
  with `compose_with_config_and_backend` over the same merged configuration
  (lead ruling S72), so env and flag overrides apply on both paths; the test
  `overrides_apply_through_an_injected_invoker` (`tests/fixtures.rs`) holds
  that.
- `--help` reads whether stdout is a terminal from `run_with`'s `Stdout`;
  `run` answers as for a pipe, so in-process tests do not depend on the
  terminal they run in.

Consequences: the configuration a command opens never depends on whether
its `bd` is the configured binary or an injected one. The ruled decisions
are also recorded in the contract (S69 in §5.4, S70 in §2, S71 in §4).

## ADR-CLI-0005: SIGTERM, SIGINT and SIGHUP stop the bd calls, then bdc exits 128 + signal

**Status:** Approved (phase-end fix RSH-001, bc-lead, 2026-10-25)

Context: every bounded `bd` child leads its own process group (PE-RSH-001), so
a signal sent to bdc, or a terminal's Ctrl-C to its foreground group, never
reaches the `bd`. With the default action bdc died and left the `bd` and a
`dolt` it started running, holding no lock, so the next call on the project
collided with them (RSH-001).

Decision:

- On Unix `run_process` installs `signal-hook`'s iterator for SIGTERM, SIGINT
  and SIGHUP before any command runs (`commands::signal`, a Unix-only
  dependency, `signal-hook = "0.3"` with only the `iterator` feature). Signals
  are received on a thread of their own; no code of ours runs in signal
  context.
- A signal calls `sc_bd_cli::shutdown::request(signal)`, which every bounded
  call observes: a running child's group is killed and reaped as on a
  timeout, a waiting call starts nothing, and both project locks (the mutex
  and the cross-process file, ADR-BDCLI-0006) are released by the call's
  normal return. It also sets the command's `StopFlag`: `cache tick` runs
  `tick_with` and an `AtmSender::with_stop` on it, so the tick ends early and
  an `atm` child is killed with its group.
- The command then unwinds normally and writes its envelope (whatever it
  reports: a write already journaled is not hidden); the exit code is
  `128 + signal` (143 for SIGTERM), whatever the envelope says, with one line
  on stderr. A second signal changes nothing: the shutdown is bounded by the
  calls' kill bound (1 s); SIGKILL remains. On Linux the `bd` child also has
  `PR_SET_PDEATHSIG` (SIGKILL), for a bdc killed outright.
- `run` and `run_with` (the in-process entry points) install nothing: the
  host process owns its signals.
- Windows: nothing is installed (no such signals); a console Ctrl-C is out
  of scope.

Consequences: the exit codes of contract §3 gain the shell's signal
convention, `128 + N`, for a bdc stopped by a signal (listed there). Test:
`commands::signal::process_tests::sigterm_kills_the_bd_group_and_exits_143`
(a `bdc` whose fake `bd` hangs with a TERM-ignoring grandchild, causal on pid
files), which runs the `bdc` binary `cargo test` builds next to the test.

### The scenario file

One JSON object. Every key is optional.

| Key | Type | Meaning |
|---|---|---|
| `now` | RFC 3339 string | The fixed UTC time of every process. A write stamps `updated_at` with it. Default `2026-10-23T10:00:00Z`. An invalid value panics (a scenario bug). |
| `version` | string | What `bd version` prints. Default: the pinned release. |
| `schema` | string | What `bd schema` prints. The schema fingerprint the cache sees is the sha256 of this text. Default: the pinned bd 1.3.1 schema. |
| `rows` | array of bd rows | The beads, in `sc_beads::Bead` form (`id`, `title`, `status`, `priority`, `issue_type`, `created_at`, `updated_at` required; `labels`, `dependencies` of `{issue_id, depends_on_id, type}` and the others optional). Rewritten after a call that changed a row. |
| `blocked` | object: id → array of ids | Each bead is blocked by the beads listed: applied as `blocks` edges on top of `rows`. The blocked set that `bd blocked` reports follows bd's rule, so a closed blocker stops blocking. An id that is not a row is a harness error. |
| `faults.unavailable` | bool | Every call that reaches the database fails as bd does with its Dolt server down: exit 1, "Dolt server unreachable ... connection refused". `bd version` and `bd schema` still answer. |
| `faults.slow_ms` | integer | Every call sleeps this long before it answers. The delay is real, so the caller's own timeout ends it. |
| `faults.fail_labels` | array of labels | Every scope read (`bd list -l`, `bd blocked -l`, `bd ready -l`) of one of these labels fails with exit 1, a failure that is not an outage; other labels answer. Used for a partial `cache refresh scopes`. |
| `faults.refuse_write` | `{exit_code}` | Every write (`close`, `update`, `dep add`) fails with this exit code and changes nothing. `13` is bd's refused guard (precondition); another code is a plain failure with that code. |
| `faults.read_status` | object: id → status | Reads (`bd list`, `bd show`, `bd ready`, `bd blocked`) report each bead with this status; writes check their `--if-*` guards against the true rows, and a read never writes its view back. Used so a guard holds on the cache's fresh read at flush and misses in bd itself (QA RCR-001). An id that is not a row is a harness error. |
| `faults.refuse_close` | `{exit_code}` | As `refuse_write`, for `bd close` only; `update` and `dep add` run and check their own `--if-*` guards. Used so the escalation scenario tests a guard miss and a refused close apart (QA RCR-001). |
| `faults.gate_file` | path | Every gated call creates `<gate_file>.started`, waits for `gate_file` to exist (at most 60 s), then creates `<gate_file>.passed` before it answers (PR #84, `src/scenario.rs`). Tests release the gate only after the caller has returned, and assert that `.passed` is absent: the caller gave up without waiting for the call. |

Any other key is rejected. A scenario that cannot be read, parsed or applied
is exit 70 with the reason on stderr; bd's own codes are 0, 1, 2 and 13.

Example:

```json
{
  "now": "2026-10-23T10:00:00Z",
  "rows": [
    {"id": "b-1", "title": "b-1", "status": "open", "priority": 2,
     "issue_type": "task", "labels": ["phase-x"],
     "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"},
    {"id": "b-2", "title": "b-2", "status": "open", "priority": 2,
     "issue_type": "task", "labels": ["phase-x"],
     "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"}
  ],
  "blocked": {"b-2": ["b-1"]},
  "faults": {"slow_ms": 500}
}
```
