# sc-bd-cli

The `bd` CLI implementation of the [`sc-beads`](https://crates.io/crates/sc-beads)
backend traits. It runs the beads `bd` binary as a child process.

## Contents

- **`BdCli`**, the backend. It implements btit's `BeadsBackend`, `CliBackend`
  and `DoltOperations` exactly as btit-bd does (ported from btit-cli and
  btit-bd at `8d8e82ae9f85`), and the cache's `CacheBackend`.
- **Deadline-bounded cache calls.** Every `CacheBackend` call stops at the
  `Deadline` it is handed, including the wait for the per-project lock: at the
  deadline the child is killed and reaped. btit's own operations keep btit's
  behaviour and block until `bd` exits.
- **The argv of each cache call** (`argv`), as pure functions. For example,
  `list_by_ids` is `bd list --id a,b,... --status
  open,in_progress,blocked,deferred,closed,pinned,hooked -n 0 --json`, in
  chunks of at most 100 ids. `fingerprint` runs `bd version` and `bd schema`,
  and never `bd context`.
- **Error classification.** Exit 13 (a refused `--if-status` /
  `--if-assignee` guard) is `Refused(Refusal::Precondition)`. A timeout, or
  stderr naming an unreachable server, is `Unavailable`. A `bd` that cannot
  start is `Btit(Spawn)`, which `BeadsCacheError::is_unavailable()` also
  reports as an outage. Any other nonzero exit is `Failed { code, stderr }`.
  Output that breaks the strict row contract is `Contract`; output that
  cannot be read, or is over the size cap, is `Transport`. A guarded write to
  a bead `bd` does not have is `NotFound`.
- **The process layer.** `CliInvoker` / `CliRunner` (one configured binary,
  with its `--version` probe cached), the extended `PATH`, `ProjectLocks`
  (one lock per normalised working directory), Dolt detection
  (`project_uses_dolt_for`, `DoltMode`), and `BdCli::with_child_var` to set
  or remove a variable such as `BEADS_DIR` in every child's environment.
- The crate holds no process-global state: a `CliRunner` owns its binary and
  probe cache, and the `ProjectLocks` it uses is passed in.

Two limits come from bd itself. `bd close` has no `--if-*` flags, so a
guarded `close_guarded` reads the bead first and then closes it, which is
**not atomic**; `update_guarded` uses bd's atomic `--if-*` flags. A write that
times out may still have landed.

## Example

```rust,no_run
use std::sync::Arc;
use std::time::Duration;
use sc_beads::{CacheBackend, Clock, Deadline, ProjectRef, SystemClock};
use sc_bd_cli::{BdCli, ProjectLocks};

let clock: Arc<dyn Clock> = Arc::new(SystemClock);
let bd = BdCli::new_with_clock("bd", Arc::new(ProjectLocks::new()), Arc::clone(&clock));
let project = ProjectRef::local(Some("/work/project".to_owned()));
let deadline = Deadline::after(&*clock, Duration::from_secs(30));
let fingerprint = bd.fingerprint(&project, deadline)?;
println!("bd {} schema {}", fingerprint.bd_version, fingerprint.schema_sha256.as_str());
# Ok::<(), sc_beads::BeadsCacheError>(())
```

## Feature flags

| Feature | Enables |
|---|---|
| `test-support` | `testing::RecordingInvoker` (a scripted invoker that spawns nothing and records every argv), `testing::FakeBd` (an invoker that behaves like `bd` over `sc_beads::testing::FakeBackend`), `BdCli::with_invoker` and `CliRunner::with_version_probe`. Also enables `sc-beads/testing`. |

With `test-support`, `BdCli` runs over `FakeBd`, so tests go through the real
argv builders, output parsing and error mapping without a `bd` binary:

```rust
use std::sync::Arc;
use std::time::Duration;
use sc_bd_cli::testing::FakeBd;
use sc_bd_cli::BdCli;
use sc_beads::testing::{bead, id, ManualClock};
use sc_beads::{CacheBackend, Deadline, ProjectRef};

let clock = Arc::new(ManualClock::new("2026-10-23T09:00:00Z"));
let fake = FakeBd::new(Arc::clone(&clock));
fake.backend().insert(bead("bk-1"));
let bd = BdCli::with_invoker(Box::new(fake)).with_clock(clock.clone());

let project = ProjectRef::local(Some("/work/project".to_owned()));
let rows = bd.list_by_ids(&project, &[id("bk-1")], Deadline::after(&*clock, Duration::from_secs(5)))?;
assert_eq!(rows[0].id.as_str(), "bk-1");
# Ok::<(), sc_beads::BeadsCacheError>(())
```

## Compatibility

The cache calls are pinned to bd 1.3.1: the strict rows are generated from
its `bd schema` (`sc_beads::BD_VERSION`, `BD_SCHEMA_SHA256`). Another bd works
as long as its rows still validate; one whose schema hashes differently is
reported through `Fingerprint`, so the cache can rebuild and report
`model_current: false`. btit's version gates target bd 1.x and also recognise
`br` (beads_rust). The repository's `bd-compat` workflow checks a bd release
against the pinned fingerprint and runs live probes against it. MSRV: Rust
1.98.1. Edition 2021, as in `sc-beads`.

## Links

- Repository: <https://github.com/randlee/beads-cache>
- Architecture: [`docs/sc-bd-cli/architecture.md`](https://github.com/randlee/beads-cache/blob/develop/docs/sc-bd-cli/architecture.md)
- Requirements: [`docs/sc-bd-cli/requirements.md`](https://github.com/randlee/beads-cache/blob/develop/docs/sc-bd-cli/requirements.md)

License: MIT.
