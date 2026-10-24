# sc-bd-cli Architecture

Status: draft (awaiting lead approval; new crate ids need a ruling before merge).
Crate-local decisions (domain code `BDCLI`). Repo-level context:
[`../architecture.md`](../architecture.md), ADR-BDC-0004, ADR-BDC-0005 and
ADR-BDC-0007; the contract it implements is
[`../sc-beads/architecture.md`](../sc-beads/architecture.md). Created by sprint a-2.

## Layout

| Module | Contents | Origin |
|---|---|---|
| `backend`, `dolt` | `BdCli` (`BeadsBackend`, `CliBackend`, `DoltOperations`), `BD_RELEASE_SOURCE`, Dolt detection, `DoltMode` | btit-bd, with the clock field and constructors of ADR-BDCLI-0002 |
| `runner`, `run`, `probe`, `path`, `command`, `locks`, `ops` | `CliInvoker` / `CliRunner`, argv assembly and spawns, the `--version` probe, the extended `PATH`, `ProjectLocks`, one function per issue operation | btit-cli, with `tracing` and `CliInvoker::run_bounded` |
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

**Status:** Draft

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
  `--if-*` flags (bd 1.3.0). This is two calls; a writer can change the bead
  between them. The cache's drain already reads the row before it writes, which
  bounds the window, and `update_guarded` uses bd's atomic `--if-*` flags. A guarded
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

**Status:** Draft

Context: btit's transport blocks until the child exits (`Command::output()`), which
is the behaviour REQ-BDC-0906 asks to keep for btit's operations. NFR-BDC-0004 needs
every cache call to stop at its `Deadline`, including the wait for the per-project
lock, which btit takes with a blocking `lock()`.

Decision:

- `CliInvoker` gains one provided method,
  `run_bounded(project, argv, timeout) -> Result<CliOutput, BoundedError>`. Its
  five required methods and every btit signature are unchanged, so an implementor
  written against btit still compiles and the ported API-freeze tests pass; the
  provided body refuses with `Unsupported` rather than falling back to an unbounded
  spawn. `CliRunner` implements it with `bounded::run_bounded`; `RecordingInvoker`
  and `FakeBd` implement it for tests.
- `run_bounded` polls: the project lock with `try_lock`, the child with `try_wait`
  every 5 ms (shortened to the time left), so no wait outlasts the timeout. At the
  timeout the child is killed and reaped. Two reader threads drain stdout and
  stderr; after a kill they are given 100 ms to hand back what was written and are
  then abandoned (they end when the pipes close, even if a grandchild holds them).
  Standard input is closed. The environment and working directory are btit's:
  the extended `PATH`, `BEADS_PATH` = the working directory.
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

**Status:** Draft

Decision: the feature keeps btit's name, `test-support` (it enables
`BdCli::with_invoker`, `CliRunner::with_version_probe` and `testing`, as in
btit), and additionally enables `sc-beads/testing` for `FakeBd`. The crate stays on
edition 2021 like `sc-beads` and for the same reason (ADR-BEADS-0001: the ported
bodies compile unchanged). `FakeBd` carries a byte-for-byte copy of the pinned
`bd schema` fixture (`src/testing/bd-schema-1.3.1.json`) so the package is
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
