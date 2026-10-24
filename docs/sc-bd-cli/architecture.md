# sc-bd-cli Architecture

Status: approved (lead ruling, bc-lead, 2026-10-24). Repo-level context:
[`../architecture.md`](../architecture.md), ADR-BDC-0004, ADR-BDC-0005 and
ADR-BDC-0007; the contract it implements is
[`../sc-beads/architecture.md`](../sc-beads/architecture.md). Created by sprint a-2.

## Layout

| Module | Contents | Origin |
|---|---|---|
| `backend`, `dolt` | `BdCli` (`BeadsBackend`, `CliBackend`, `DoltOperations`), `BD_RELEASE_SOURCE`, Dolt detection, `DoltMode` | btit-bd, with the clock field and constructors of ADR-BDCLI-0002 |
| `runner`, `run`, `probe`, `path`, `command`, `locks`, `ops` | `CliInvoker` / `CliRunner`, argv assembly and spawns, the `--version` probe, the extended `PATH`, `ProjectLocks`, the crate-private `ChildEnv` (ADR-BDCLI-0004), one function per issue operation | btit-cli, with `tracing`, `CliInvoker::run_bounded` and `CliInvoker::set_child_var` |
| `cache` | `CacheBackend for BdCli`: exit-status classification, guards on close, fingerprint parsing | new |
| `argv` | the argv of every cache call, as pure functions | new |
| `bounded` | `run_bounded` and `BoundedError`: the deadline-bounded process run | new |
| `testing` (feature `test-support`) | `RecordingInvoker` (btit's, plus `run_bounded`), `FakeBd` | btit-cli's `testing`, new |

The crate depends on `sc-beads` (workspace crate), `serde_json` and `tracing`
(both already workspace dependencies of `sc-beads`); with `test-support` it enables
`sc-beads/testing`. It does not depend on `bd-cache-model`, which the
architecture's Boundaries table forbids (the CI boundary check counts
dev-dependencies too).

## ADR-BDCLI-0001: Argv mapping and error classification

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: the cache calls of `sc_beads::CacheBackend` must map to `bd` command
lines that match the BD-CACHE baseline (REQ-BDC-0201), and every way a `bd`
process can end must map to one `BeadsCacheError` the cache classifies
(`bd-cache`'s `call` helper, ADR-BDC-0005).

Decision, argv (`argv.rs`; each form is pinned as a literal in its unit test and
in `tests/cache_calls.rs`):

| Call | argv |
|---|---|
| `fingerprint` | `version`, then `schema` (never `context`) |
| `list_by_ids` | `list --id a,b,... --status open,in_progress,blocked,deferred,closed,pinned,hooked -n 0 --json`, ids deduplicated, at most 100 per call, one call per chunk |
| `list_scope` | `list -l <label> --status <all seven> -n 0 [--updated-after <cursor>] --json` |
| `list_blocked` | `blocked -l <label> --json` |
| `list_ready` | `ready -l <label> -n 0 --json` |
| `close_guarded` | `close <id> [--reason R] --actor A` |
| `update_guarded` | `update <id> [--status S] [--assignee U] [--add-label L]... [--remove-label L]... [--set-metadata k=v]... [--unset-metadata k]... [--append-notes T] [--if-status S] [--if-assignee U] --actor A` |
| `dep_add_typed` | `dep add <id> <on> --type T --actor A` |

Reads end in `--json`; writes do not, because bd prints a human line on success and
the exit status is the result. Every flag value is its own argv word (no quoting):
unassign is `--assignee ""` and `--if-assignee ""` requires unassigned. Metadata is
`key=value` with `MetaValue::to_bd_arg` text; a key that is empty or contains `=`
cannot be carried by `--set-metadata` and is refused before any call
(`Btit(Unsupported)`). The seven statuses are always named because bd omits closed
beads from `--id` and label lists otherwise (P-6; live probe
`list_by_id_returns_a_closed_bead_only_with_the_status_list`). The chunk size
keeps the command line under Windows' 32 767-character limit.

Decision, behaviour that bd does not provide itself:

- `close_guarded` with guards reads the bead first (`list_by_ids`) and refuses
  with `Refused(Precondition)` when a guard is unmet, because `bd close` has no
  `--if-*` flags (bd 1.3.0). This is two calls and **not atomic**: a writer can change the bead
  between them. The cache's drain already reads the row before it writes, which
  bounds the window (the drain's precondition re-check on the fresh pre-read row is the safety net), and `update_guarded` uses bd's atomic `--if-*` flags. A guarded
  close of a bead bd does not have is `Failed { code: 1 }` (bd's own exit code for an
  unknown bead), not a refusal.
- `update_guarded` with an empty patch makes no call: `bd update` with no field
  prints "No updates specified" and exits 0 without checking guards.
- A write that times out may have landed. The cache's `achieved` check on the
  next drain is what makes that safe (REQ-BDC-0305).

Decision, classification (`cache::check_exit`, in this order):

| Outcome | `BeadsCacheError` |
|---|---|
| exit 0 | the stdout |
| exit 13 | `Refused(Refusal::Precondition)` |
| stderr names an unreachable server (`UNREACHABLE_MARKERS`: connection refused, server unreachable, i/o timeout, no such host, connection reset by peer, network is unreachable, no route to host) | `Unavailable { timed_out: false }`, reason = first stderr line |
| stderr contains `no such column: spec_id` | `Btit(SchemaMigration)` (btit's check, kept) |
| any other nonzero exit | `Failed { code, stderr }` (`-1` when ended by a signal); in `fingerprint`, `CheckFailed { step, code, stderr }` |
| deadline passed before the call, or reached during it | `Unavailable { timed_out: true }` |
| bd cannot be started, or the project is not local | `Btit(Spawn)` / `Btit(Unsupported)` |
| stdout breaks the strict row contract | `Contract`, from `parse_bd_rows` / `parse_blocked_rows` |

Why this shape: exit 13 is the only typed signal bd gives (`bd update --help`).
bd exits 1 for every other failure, an unreachable database included, so the
"unreachable" case reads stderr: it is the one place this crate does, kept to a
named constant, checked against bd 1.3.0's wording ("Dolt server unreachable at
...: dial tcp ...: connect: connection refused") and re-checked by the live probe
`unreachable_server_is_unavailable` on every bd release (REQ-BDC-0905). A wording
change degrades to `Failed`, never to a success. A bd that cannot be started is
`Btit(Spawn)` rather than `Unavailable` so the `io::Error` stays in the `source`
chain (`Unavailable` carries only text); `BeadsCacheError::is_unavailable()`
covers it by design (ADR-BEADS-0002), so the cache treats it as an outage.

Consequences: any new flag or form is one function in `argv.rs` with a literal
test; a bd release that changes a form or a message fails the compat workflow's live
probes before it fails a running cache.

## ADR-BDCLI-0002: Deadline-bounded calls beside btit's unbounded ones

**Status:** Approved (lead ruling, bc-lead, 2026-10-24); amended by ADR-BDCLI-0004 (lead rulings S40/S47)

Context: btit's transport blocks until the child exits (`Command::output()`), which
is the behaviour REQ-BDC-0906 asks to keep for btit's operations. NFR-BDC-0004 needs
every cache call to stop at its `Deadline`, including the wait for the per-project
lock, which btit takes with a blocking `lock()`.

Decision:

- `CliInvoker` gains one provided method,
  `run_bounded(project, argv, timeout) -> Result<CliOutput, BoundedError>`
  (plus `CliInvoker::set_child_var`, added by ADR-BDCLI-0004). Its
  five required methods and every btit signature are unchanged, so an implementor
  written against btit still compiles and the ported API-freeze tests pass; the
  provided body refuses with `Unsupported` rather than falling back to an unbounded
  spawn. `CliRunner` implements it with `bounded::run_bounded`; `RecordingInvoker`
  and `FakeBd` implement it for tests.
- `run_bounded` polls: the project lock with `try_lock`, the child with `try_wait`
  every 5 ms (shortened to the time left), so no wait outlasts the timeout. A timeout
  names its phase (`TimeoutPhase`: `Lock`, `Child`, `Output`) and the phase is in the
  `Unavailable` reason. At the timeout the child is killed and reaped
  (`kill_and_reap`). Two reader threads drain stdout and stderr and send
  `io::Result<Vec<u8>>`, so a read error is `BoundedError::Output` with the real
  `io::Error` as its source (the cache reports it as `Contract`, field `output`, not as
  a spawn failure). Each stream is capped at `MAX_OUTPUT_BYTES` (64 MiB): the excess is
  read and dropped so the child is not stalled, and the call fails with a message naming
  the cap. After a kill the readers are abandoned, not waited for: they end when the
  pipes close, even if a grandchild holds them, and nothing they hold is needed. After a
  normal exit, waiting for them is bounded by the same deadline (`Output` phase).
  Standard input is closed. The environment and working directory are btit's: the
  extended `PATH`, `BEADS_PATH` = the working directory, plus any child variables set
  through `CliInvoker::set_child_var` (ADR-BDCLI-0004).
- The lock key is a `WorkingDir`, built by `run::resolve_project_dir` and normalised
  (trailing separators dropped, a root kept), so `/p` and `/p/` are one lock;
  `ProjectLocks::guard(&str)` keeps btit's signature and normalises too.
- The cache path never probes `--version` (that spawn is unbounded); the crate is
  the bd backend, so it assumes `CliClient::Bd`.
- `BdCli` holds the `Arc<dyn Clock>` the cache and its store share and turns each
  `Deadline` into the time left with it (`Deadline::remaining`); the child is then
  limited with real elapsed time, so a test clock decides whether a deadline has
  already passed and real time decides when a running child is killed. btit's
  constructors (`new`, `with_seeded_probe`, `with_invoker`) use `SystemClock`;
  `new_with_clock` and `with_clock` are the cache's. A deadline already passed
  starts nothing.
- The lock's `Mutex<()>` guards no data; a poisoned lock is recovered (the same rule
  as btit's `ProjectLocks`), commented where it is taken.

Alternatives rejected: a worker thread per call that abandons the child on timeout
(leaves processes running); a new trait beside `CliInvoker` (a second set of
constructors and a second test double for no gain).

Consequences: btit's own operations (`ops`, `run_json`, `run_raw`, the probe) remain
unbounded exactly as in btit; only the cache calls carry NFR-BDC-0004.

## ADR-BDCLI-0003: Test doubles, features and edition

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Decision: the feature keeps btit's name, `test-support` (it enables
`BdCli::with_invoker`, `CliRunner::with_version_probe` and `testing`, as in
btit), and additionally enables `sc-beads/testing` for `FakeBd`. The crate stays on
edition 2021 like `sc-beads` and for the same reason (ADR-BEADS-0001: the ported
bodies compile unchanged). `FakeBd` carries a byte-for-byte copy of the pinned
`bd schema` fixture (`tests/fixtures/bd-schema-1.3.1.json`, LF-pinned by `.gitattributes`) so the package is
self-contained; a unit test fails if its sha drifts from `BD_SCHEMA_SHA256`.
`tests/process.rs` has no libtest harness: the test binary is its own fake `bd`
(selected by the working directory name through `BEADS_PATH`), which needs no
shell script and runs on every CI platform. The metadata round-trip test restates
the validator's accepted set instead of calling `bd_cache_model::op::meta_value`,
because the dependency would break the CI boundary check; the live probe repeats
the round trip against a real bd.

Consequences: a bd release that changes metadata coercion or any message the
classification reads is caught by the compat workflow's live probes, and
`FakeBd`'s coercion rule (`coerce_metadata`) is the single place to update.

## ADR-BDCLI-0004: A child environment variable set or removed by the runner

**Status:** Approved (lead ruling S40, bc-lead, 2026-10-24; fix-a-7-12, RSH-033); amends ADR-BDCLI-0002 (lead ruling S47, fix-a-7-14, ARCH-903); forwarding rule narrowed by lead ruling S52 and the platform rule stated by lead ruling S50 (fix-a-7-15, ARCH-1002, RSH-040, QA10-002)
**Traces to:** REQ-BDCLI-0009

Context: `bd-cache-sync`'s `compose_with` resolves the cache from an explicit
`$BEADS_DIR` value (`None` = unset; lead ruling S36), not the process
environment, so a test does not depend on the machine. Every `bd` the backend
spawns inherited the process's `BEADS_DIR`, so the cache and `bd` could name
two beads directories.

Decision: one provided method on `CliInvoker`,
`set_child_var(&mut self, key: &OsStr, value: Option<&OsStr>)`, sets (`Some`)
or removes (`None`) `key` in the environment of every child the invoker spawns
from then on; a later call for the same key replaces the earlier one. The
provided body does nothing, which is right for an invoker that spawns no
process (`RecordingInvoker`, `FakeBd`). The forwarding rule: an invoker that
wraps one which spawns processes or acts on child variables (a `CliRunner`, or
another wrapper that forwards) overrides `set_child_var` and forwards it to the
wrapped one, as it forwards the spawning methods; the provided no-op would
otherwise drop the change silently and the wrapped runner's children would
inherit the process's value (fix-a-7-14, RSH-036; the rule is stated on the
trait, and `bd-cache-sync`'s test wrapper `Switched` follows it). A wrapper over
a no-op invoker is exempt, since there is no child to give the variable to:
for example `sc-bd-cli`'s test wrapper `Shared(Arc<RecordingInvoker>)` keeps
the provided body (lead ruling S52, ARCH-1002). `CliRunner` keeps the changes in a crate-private `ChildEnv` and
applies them to `run_json`, `run_raw` and `run_bounded` right after
`new_command`, so the spawn's own `PATH` and `BEADS_PATH` still win for those
two names; the `--version` probe, which runs in the temp directory for no
project, does not get them. `CliRunner::with_child_var` and
`BdCli::with_child_var` are the builders. One entry per name: `ChildEnv` dedups
names exactly on Unix and ASCII case-insensitively on Windows (fix-a-7-14,
QA9-002). Names are ASCII (`ChildEnv::set` has `debug_assert!` that the name is
ASCII), so that fold is all of Windows' rule for them; and whatever the dedup
leaves, `std::process::Command` applies the platform's full rule when it builds
the child's environment, where the last change of a variable wins, as the last
`set` does (lead ruling S50, RSH-040, QA10-002). btit's public functions
(`run::spawn_json`, `run::run_raw`, `bounded::run_bounded`,
`bounded::run_bounded_capped`) keep their signatures and an inherited
environment; the runner calls crate-private variants that take the
`ChildEnv`. The API-freeze tests are unchanged.

Alternatives rejected: checking at composition that the explicit value equals
the process's (a test passing `None` on a machine with `BEADS_DIR` set would
fail, which S36 exists to prevent); changing the public spawn functions'
signatures (breaks btit's API freeze).

Consequences: `compose_with` sets `BEADS_DIR` on the backend's children to the
value the cache resolved from, or removes it. ADR-BDCLI-0002's "one provided
method" and its `run_bounded` environment now include this method and these
variables (S47). Evidence (REQ-BDCLI-0009):
`the_child_environment_reaches_run_raw_and_run_bounded`,
`the_child_environment_reaches_run_json` (fix-a-7-14, QA9-001),
`the_child_environment_reaches_every_spawn_on_every_platform` (fix-a-7-14,
QA9-002: re-executes the unit-test binary as `child_env_helper`, so the
windows-latest leg runs it too; the helper is `#[ignore]` and run with
`--ignored --exact`, so it is not counted as a passing test, QA10-004; each
re-executed leg runs on a worker thread bounded by 60 s, FTQ-032). That
end-to-end test checks the variables' delivery; the dedup of names is checked
by the unit test `names_compare_as_the_platform_environment_does`, since on
Windows `Command` would deliver the same environment without it (QA10-004),
`without_a_child_variable_the_environment_is_inherited` (`src/runner.rs`),
`a_later_set_of_a_name_replaces_the_earlier_one`,
`names_compare_as_the_platform_environment_does`,
`a_non_ascii_name_is_refused_in_debug_builds` (S50) (`src/command.rs`); in
`bd-cache-sync`, `tests/compose.rs` (S40 and S44 cases).
