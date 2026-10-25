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
| `shutdown` | the process-wide shutdown request every bounded call observes (RSH-001) | new |
| `process_group` (Unix) | `kill` and `exists` of a process group through `kill(2)` (RSH-002) | new |
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
| bd's output cannot be read (`BoundedError::Output`) | `Transport`, the read error as its `source` (PE-RBP-F001) |
| a stream exceeds `MAX_OUTPUT_BYTES` (`BoundedError::OutputTooLarge { stream, limit }`) | `Transport`, its `source` an `io::ErrorKind::FileTooLarge` naming the stream and the cap (PE-RBP-F012) |
| bd started but could not be polled for its exit or reaped (`BoundedError::Wait`) | `Transport`, the OS error as its `source` (PE-RBP-F012) |
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
A wait failure is not a spawn failure and not an outage: bd did start and may have
finished, a write included, so what it did is unknown. It is `Transport`, like output
that could not be read, which the cache maps by call kind (`BD_WRITE_FAILED` for a
write) instead of treating it as a safe-to-retry `BD_UNAVAILABLE` (PE-RBP-F012).

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
- `run_bounded` polls: the project lock with `try_lock`, the child's exit every 5 ms
  (shortened to the time left), so no wait outlasts the timeout. A timeout
  names its phase (`TimeoutPhase`: `Lock`, `Child`, `Output`) and the phase is in the
  `Unavailable` reason. The child is spawned as the leader of its own process group
  (`process_group(0)` on Unix, `CREATE_NEW_PROCESS_GROUP` on Windows). On Windows it
  is also created suspended, put in a job object of its own
  (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` while the call runs) and then resumed, so
  everything it starts is in the job (RCR-001). At the timeout the whole group is
  killed (`kill -KILL -- -<pid>` on Unix, a program; `TerminateJobObject` on
  Windows, through the job's handle, with `taskkill /T /F` only when no job could
  be set up), then the child, and the child is reaped by polling `try_wait`; the
  call then waits until the group has no member left (Unix: `kill -0`; Windows: the
  job's active-process count). All of it shares one `KILL_BOUND` (1 s); a kill
  program or child still running at the bound is handed to a detached reaper thread
  instead of being waited for (PE-RSH-001, PE-RSH-002). The project lock is held
  throughout, so the next call on the project starts only after the group is gone
  or the bound has passed. On an `Output` timeout (the child exited, a process it
  left in its group holds a pipe) that group is killed the same way, before the
  child is reaped (RCR-001): on Unix the exit is noticed with `waitid(P_PID, pid,
  WEXITED | WNOHANG | WNOWAIT)`, which leaves the child a zombie, and POSIX reuses
  neither a pid nor a process group id equal to it while that process exists (XBD
  4.14), so the group kill can only reach the group the child led; on Windows the
  job reaches the helper although its parent has exited, and no pid is named. These
  are the crate's only `unsafe` blocks (libc `waitid`; the Win32 job and
  thread-resume calls in `bounded::job`), each with a `SAFETY` comment. After a
  normal exit the child is reaped and, on Windows, the job's kill-on-close is
  cleared, so a process the child left running that holds none of its pipes
  outlives the call on both platforms. The public `kill_and_reap(&mut Child)` is
  bounded by `KILL_BOUND` as well; it cannot move a borrowed child to the reaper, so a
  child still there at the bound stays with its owner. Two reader threads drain stdout and stderr and send
  the bytes or the error, so a read error is `BoundedError::Output` with the real
  `io::Error` as its source (the cache reports it as `Transport`, not as a spawn
  failure). Each stream is capped at `MAX_OUTPUT_BYTES` (64 MiB): the excess is read and
  dropped so the child is not stalled, and the call fails with
  `BoundedError::OutputTooLarge { stream, limit }`, distinct from a read error
  (PE-RBP-F012). A failed exit check or reap of a started child is
  `BoundedError::Wait`, never `Failed(Spawn)`; the child's group (job) is killed first. After a kill the readers are abandoned, not waited for: they end when the
  pipes close, even if a grandchild holds them, and nothing they hold is needed. After a
  normal exit, waiting for them is bounded by the same deadline (`Output` phase).
  Standard input is closed. The environment and working directory are btit's: the
  extended `PATH` (ADR-BDCLI-0005), `BEADS_PATH` = the working directory, plus any child variables set
  through `CliInvoker::set_child_var` (ADR-BDCLI-0004).
- The lock is named by a `WorkingDir`, built by `run::resolve_project_dir` and
  normalised lexically (trailing separators dropped, a root kept). The map key is
  the directory's canonical path, resolved when the lock is taken
  (`ProjectLocks::try_guard_dir`, PE-RBP-F010), so `/p`, `/p/`, `/p/sub/..` and a
  symlink to `/p` are one lock; a directory that does not exist keys on its lexical
  form, and one whose canonical path is not UTF-8 is refused (`Unsupported`).
  `ProjectLocks::guard(&str)` and `guard_dir` keep their signatures and canonicalize
  too (falling back to the lexical form for a non-UTF-8 path, as they cannot fail).
- The cache path never probes `--version` (that probe has its own bound,
  `VERSION_PROBE_TIMEOUT`, not the call's deadline); the crate is the bd backend, so
  it assumes `CliClient::Bd`.
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

Consequences: btit's own operations (`ops`, `run_json`, `run_raw`) remain
unbounded exactly as in btit; only the cache calls carry NFR-BDC-0004. The
`--version` probe is bounded by `VERSION_PROBE_TIMEOUT` (10 s) and kills its process
group at the bound (PE-RSH-012); its signature is btit's. Because a btit JSON call
waits for and holds the project lock without a bound, a hung btit call would make
every cache call on that project time out at the `Lock` phase: a `ProjectLocks` used
by btit's calls must not be shared with the cache (documented on `ProjectLocks`,
`BdCli` and `CliRunner`). Evidence: `a_hanging_version_probe_is_stopped_at_its_bound`
(`run.rs`). A probe that cannot be spawned is logged with its cause before `Failed` is
cached (PE-RBP-F021; `a_probe_spawn_failure_is_logged_before_it_is_cached`,
`runner.rs`).

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

## ADR-BDCLI-0005: The extended `PATH`: absolute entries, the user's first

**Status:** Accepted (lead rulings S76 and S77, QA RCR-002)

Context: every `bd` child gets `PATH` = `path::get_extended_path()` and runs
in the repository root. btit's version put fixed install directories
(Homebrew, Linuxbrew, `/usr/bin`, `/bin`, `~/.local/bin`, `~/bin`,
`$GOPATH/bin`, `~/.cargo/bin`; on Windows those under `USERPROFILE` and
`LOCALAPPDATA`) ahead of the inherited `PATH`, and kept the inherited
segments as they were. An empty or relative segment, or a directory built
from a relative `HOME` or `GOPATH`, resolves against the working directory,
so a bare `bd` could run a `bd` committed in the repository. And a `bd`
installed in a fixed directory beat the user's own `PATH` (contract §2 says
`bd` on `PATH`).

Decision:

- Only absolute segments are kept (`Path::is_absolute`; on Windows a drive or
  UNC root), from the inherited `PATH` and from the fallback alike. An unset
  `PATH` adds nothing.
- The inherited `PATH` comes first; the fixed install directories follow as
  a fallback, for GUI launches with a minimal `PATH`.
- A fallback directory under an unset, empty or relative `HOME`, `GOPATH`,
  `USERPROFILE` or `LOCALAPPDATA` is not built.
- Each directory appears once, at its first occurrence.

Consequences: a behaviour change for hosts whose `PATH` names a different
`bd` than a fixed directory does: the `PATH` one now runs. A relative entry a
user put on `PATH` on purpose is no longer searched for `bd`.

Tests: `src/path.rs` (`the_inherited_path_keeps_only_absolute_segments`,
`the_inherited_path_comes_first_and_duplicates_are_removed`,
`a_relative_or_empty_home_or_gopath_adds_no_fallback_directory` and its
Windows twin); in `bd-cache-cli`, `tests/process.rs`
(`an_empty_or_relative_path_segment_never_runs_a_bd_committed_in_the_repo`,
`the_users_path_beats_a_fallback_directory`).

## ADR-BDCLI-0006: A cross-process project lock, and process control without helper programs

**Status:** Approved (lead ruling S81, bc-lead, 2026-10-25; phase-end fixes RSH-001..004)

Context: embedded Dolt crashes when two `bd` processes open one database at
once. The only exclusion was `ProjectLocks`, an in-process mutex, so two bdc
processes (a cron `cache tick` and an agent's `get`) could run two `bd` on one
project (RSH-004). The timeout kill of a `bd` group ran `/bin/kill` or `kill`
from `PATH`; in a minimal image that program is missing or cannot start, and
only the leader died (RSH-002). And a host that died of SIGTERM left its `bd`
group running, since the group does not get the host's signals (RSH-001).

Decision:

- **Cross-process lock (S81).** Every bounded call (`run_bounded*`,
  `CliRunner::run_bounded`) on an embedded-Dolt project takes, after the
  in-process mutex and for as long as it, an exclusive advisory lock on a
  per-project file:
  `<canonical project dir>/.beads/sc-bd-cli.lock`, or, when that cannot be
  opened (no `.beads/`, not writable and no existing file to open read-only),
  `<OS temp dir>/sc-bd-cli-<FNV-1a 64 of the canonical dir>.lock`. `.beads/`
  first, because it is the project's own directory that every process on the
  project can see, whoever runs it; the temp fallback keys on a stable hash so
  every process without `.beads/` meets in one file. The lock is
  `std::fs::File::try_lock` (`flock` / `LockFileEx`, the primitive the drain
  lock uses through fs4), polled with the call's other waits against its
  deadline, so a wait ends as `TimeoutPhase::Lock`. It is released by closing
  the file: on every return (success, timeout, failure, shutdown, after the
  kill of the child's group) and when the process dies. The file is never
  deleted. btit's unbounded calls keep the mutex only. A lock file that can
  be neither opened nor locked fails the call as a spawn failure (nothing
  started).
- **Server mode takes no file lock** (a consumer's input on S81): when
  the project's `.beads/metadata.json` resolves to `DoltMode::Server` by bd's
  own rule (`dolt_mode: "server"`, or a non-local `dolt_server_host` with no
  mode), many `bd` processes are concurrent by bd's design, and the lock would
  only make every miss fill wait behind a long sync; a shared server runs bd that way.
  A missing or unreadable `metadata.json`, `embedded`, `proxied-server` and any
  other value take the lock. The in-process mutex is unchanged in every mode.
  The protection is partial in embedded mode: agents and bd's git hooks call
  `bd` directly without this lock, so it orders bdc's own calls only.
- **Release lag.** A child forked by another thread while a guard is held
  inherits the locked file until it execs (files are close-on-exec), so a
  dropped guard can release the lock a moment late; callers poll for it, as
  the bounded calls do against their deadline (RCR-001).
- **Kill with `kill(2)`.** On Unix the group kill and the "group is gone"
  probe are `kill(-pgid, SIGKILL)` and `kill(-pgid, 0)` (`process_group::kill`,
  `process_group::exists`, safe wrappers that refuse the pgids 0 and 1). No
  program is started. Windows keeps the job object and `taskkill /T`.
  Another crate that leads a process group uses the same wrappers, so it
  stays free of `unsafe`.
- **Shutdown.** `shutdown::request(signal)` (a process-wide atomic; signals are
  process-wide) makes every bounded call stop as on a timeout, with
  `BoundedError::Interrupted { phase, signal }`. The cache maps it to
  `Unavailable` (not a timeout) when no `bd` started, and to `Transport` when
  one was killed. On Linux a bounded child also gets `PR_SET_PDEATHSIG`
  (SIGKILL) in a `pre_exec` hook; the spawning thread polls the child until it
  is gone, so the thread-scoped death signal never fires early.
- `PATH` (finding 5): the inherited `PATH` is read with `var_os` and
  `split_paths`, so one non-UTF-8 segment no longer drops the rest; the
  child's `PATH` is an `OsString`. `path::absolute_path_entries` is the
  S76/S77 rule for another crate's own `PATH` search (RSH-003).

Consequences: on an embedded-Dolt project a `bd` call waits for another
process's call on the same project, bounded by its deadline. A `.beads/sc-bd-cli.lock` file appears in
an embedded-Dolt project and is never removed: add `sc-bd-cli.lock` to the
project's `.beads/.gitignore` (contract §3, "Concurrent `bd` calls"). Two processes that
disagree on whether `.beads/` is usable would use different files; an existing
file is opened read-only to narrow that case.

Tests (`tests/process.rs`, each a separate process where the case needs one):
`two_processes_never_run_bd_on_one_project_at_once` (an explicit embedded
project), `a_server_mode_project_takes_no_cross_process_lock`,
`the_cross_process_lock_is_released_when_a_call_is_cut`,
`the_group_kill_needs_no_kill_program` (a host with an empty `PATH` and
`RLIMIT_NPROC` 0), `a_shutdown_request_stops_the_calls`,
`a_non_utf8_path_reaches_the_child`; FTQ-002: the two process-group timeout
tests trigger their timeout through `run_bounded_expiring` (feature
`test-support`) only once the grandchild runs, under a 60 s fail-safe.
