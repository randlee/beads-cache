# beads-cache

A local SQLite read/write cache in front of the beads `bd` CLI.

beads-cache was extracted from atm-core, where the cache runs today as part of
the `atm-bd-orchestration` skill (`bd_cache.py`, contract `BD-CACHE.md`). It
is a **filtered local replica**: it holds only the beads in its configured
sync scopes (`phase-<x>` labels today), because bd databases are too large to
copy whole.

- **Reads** come from the cache file and start no `bd` process. Each reply
  reports how fresh it is (`stale`, `last_ok_at`, the backend outage if any).
- **Writes** are validated, checked against a precondition and journaled with
  an idempotency key. The reply says the write is journaled, not that it has
  landed.
- **Drain** applies journaled writes through `bd` in order, with
  preconditions re-checked and a read-back. A write ends applied, refused,
  escalated, or still pending while the backend is unreachable.
- **Sync** refreshes each scope from its own cursor, then takes the scope's
  blocked and ready sets from `bd blocked` and `bd ready`. `ready` reflects
  the cache's own journaled writes without a `bd` call.
- The backend may be slow or unreachable. Every `bd` call has a deadline, and
  an outage leaves writes pending instead of escalating them.

All backend access goes through `bd`. The cache never reads dolt tables.

## Crates

| Crate | Published | What it is |
|---|---|---|
| [`sc-beads`](crates/sc-beads) | crates.io | The beads backend contract: btit's `BeadsBackend` traits unchanged, the `CacheBackend` extension with per-call deadlines, strict rows generated from `bd schema`, `Clock` and `Deadline`; `FakeBackend` behind `testing` |
| [`sc-bd-cli`](crates/sc-bd-cli) | crates.io | The `bd` CLI implementation of those traits (`BdCli`): process runner, deadline-bounded cache calls, exit 13 as a typed refusal, Dolt detection; `FakeBd` behind `test-support` |
| [`sc-cron`](crates/sc-cron) | crates.io | A SQLite-backed cron scheduler for tokio with generation fencing, jitter and missed-tick handling. No bd types; a prototype for sc-runtime |
| [`bd-cache-py`](crates/bd-cache-py) | PyPI, as `bd-cache-rs` | Python bindings (maturin / PyO3), module `bd_cache_rs`: the cache in process, over the real store and `bd` |
| `bd-cache-model` | no | Pure cache model: journal ops, replay, the store ports, configuration, replies, `CacheError` with every `BD_CACHE.*` code |
| `bd-cache-store` | no | The SQLite `CacheStore` (the BD-CACHE v3 file, shared with the Python implementation) and the `flock` drain lock |
| `bd-cache` | no | The `Cache` facade: get, list, write, drain, sync, ready, resolve, prune, status |
| `bd-cache-sync` | no | The sync task on `sc-cron` (drain, then sync each active phase), its announcements, and `compose`, which builds the real cache of a repository |

`sc-beads`, `sc-bd-cli` and `sc-cron` depend on no bd-cache crate, so they can
be used on their own.

## Quick start: Rust

```sh
cargo add sc-beads sc-bd-cli sc-cron
```

**sc-beads**: validate `bd list --json` output against the strict row
contract, and bound calls with deadlines.

```rust
use std::time::Duration;
use sc_beads::{parse_bd_rows, Clock, Deadline, SystemClock};

let rows = parse_bd_rows(r#"[{"id": "bk-1", "title": "t", "status": "open", "priority": 2,
  "issue_type": "task", "created_at": "2026-10-23T09:00:00Z",
  "updated_at": "2026-10-23T09:00:00Z"}]"#)?;
assert_eq!(rows[0].status.as_deref(), Some("open"));

let clock = SystemClock;
let d = Deadline::after(&clock, Duration::from_millis(500))
    .earliest(Deadline::after(&clock, Duration::from_secs(30)));
assert!(d.remaining(&clock) <= Duration::from_millis(500));
# Ok::<(), sc_beads::BeadsCacheError>(())
```

**sc-bd-cli**: call `bd` with a hard deadline per call.

```rust,no_run
use std::sync::Arc;
use std::time::Duration;
use sc_beads::{CacheBackend, Clock, Deadline, ProjectRef, ScopeFilter, SystemClock};
use sc_bd_cli::{BdCli, ProjectLocks};

let clock: Arc<dyn Clock> = Arc::new(SystemClock);
let bd = BdCli::new_with_clock("bd", Arc::new(ProjectLocks::new()), Arc::clone(&clock));
let project = ProjectRef::local(Some("/work/project".to_owned()));
let phase = ScopeFilter::Label("phase-x".to_owned());
let ready = bd.list_ready(&project, &phase, Deadline::after(&*clock, Duration::from_secs(5)))?;
println!("{} ready", ready.len());
# Ok::<(), sc_beads::BeadsCacheError>(())
```

**sc-cron**: run a job on a cron schedule kept in SQLite.

```rust,no_run
use std::sync::Arc;
use std::time::{Duration, Instant};
use sc_cron::{Job, RunOutcome, ScheduleSpec, Scheduler, SqliteScheduleStore, StopDecision,
    SystemClock, SystemJitter};

struct Tick;

#[async_trait::async_trait]
impl Job for Tick {
    async fn run(&self, _payload: &serde_json::Value, _deadline: Instant) -> RunOutcome {
        RunOutcome::Ok
    }
    async fn should_stop(&self, _payload: &serde_json::Value) -> StopDecision {
        StopDecision::Continue
    }
}

# async fn example() -> Result<(), sc_cron::CronError> {
let scheduler = Scheduler::new(
    Arc::new(SqliteScheduleStore::open("cron.db")?),
    Arc::new(SystemClock),
    Arc::new(SystemJitter),
);
let spec = ScheduleSpec::new("tick", "*/30 * * * * *", Duration::from_secs(25), serde_json::json!({}))?;
scheduler.start_async(spec).await?;
let worker = scheduler.spawn_worker(Arc::new(Tick));
worker.shutdown(tokio::time::Instant::now() + Duration::from_secs(30)).await;
# Ok(())
# }
```

`sc-cron`'s `Job` trait uses `async-trait`, so that example also needs
`async-trait`, `serde_json` and `tokio` (multi-thread runtime). Each crate's
README has more, including its test doubles.

## Quick start: Python

```sh
pip install bd-cache-rs
```

```python
import bd_cache_rs

with bd_cache_rs.Cache("/path/to/repo", scopes=["x"]) as cache:   # phase-x
    cache.sync("x", 30)
    print(cache.get("bk-12")["row"]["bead"]["status"])
    cache.write("bk-12", "close", {"reason": "PASS"}, {"status": "open"},
                "close:bk-12:1", "dev-1")                         # journaled, not yet applied
    print([v["bead"]["id"] for v in cache.ready("x")["ready"]])
    cache.drain(25, 28)                                           # apply through bd
```

The repository needs `.beads/metadata.json` (written by `bd`). Settings are
read from `.beads/bd-cache.toml`, for example:

```toml
scopes = ["x"]                # phase names ("x" or "phase-x"): the beads labelled phase-x
# Optional, shown with their defaults:
# cache_path = ".bd-cache/<dolt_database>.bd-cache.sqlite3"
# call_timeout_s = 30         # per-call cap on every bd call
# miss_fill_timeout_ms = 2000 # a get that misses waits this long for bd
# max_attempts = 5            # failed writes to a reachable bd before escalation
```

Unknown keys are an error. The `[tick]` table configures the sync task in
`bd-cache-sync`.

See [`crates/bd-cache-py/README.md`](crates/bd-cache-py/README.md) for the
verbs, errors and limits.

## Install

### crates.io

```sh
cargo add sc-beads sc-bd-cli sc-cron
cargo add --dev sc-beads --features testing        # FakeBackend, ManualClock, contract suite
cargo add --dev sc-bd-cli --features test-support  # FakeBd, RecordingInvoker
```

### PyPI

```sh
pip install bd-cache-rs        # import bd_cache_rs
```

### GitHub

The crates as git dependencies, from `develop`:

```toml
[dependencies]
sc-beads = { git = "https://github.com/randlee/beads-cache", branch = "develop" }
sc-bd-cli = { git = "https://github.com/randlee/beads-cache", branch = "develop" }
sc-cron = { git = "https://github.com/randlee/beads-cache", branch = "develop" }
```

The Python module, built from source. This needs a Rust toolchain: rustup
installs the pinned 1.98.1 from `rust-toolchain.toml`, and pip fetches
maturin as the build backend.

```sh
pip install "git+https://github.com/randlee/beads-cache@develop#subdirectory=crates/bd-cache-py"
```

No GitHub Release has been cut yet, so there are no prebuilt wheels to
download. The `python` CI workflow builds an abi3 wheel for macOS arm64 and
Linux x86_64 on every push, kept as a workflow artifact.

## Requirements

- **bd:** 1.x, pinned to **bd 1.3.1**. The strict rows are generated from bd
  1.3.1's `bd schema` (schema version 1, sha256 `caf8fe18...`). Every sync and
  every non-empty drain runs `bd version` and `bd schema`. `model_current` in the reply says
  whether the live schema matches the pin, and when the schema's hash changes
  from the one recorded in the cache file, the cache drops its cached rows and
  keeps the journal. Rows from any bd must still pass the strict contract. The
  `bd-compat` workflow checks a new bd release against the pin.
- **Rust:** 1.98.1 (`rust-toolchain.toml`, `rust-version` in `Cargo.toml`).
  SQLite is bundled through rusqlite.
- **Python:** CPython 3.9 or later (`requires-python = ">=3.9"`, abi3 wheel).
- **Platforms:** the Rust crates are tested on Linux, macOS and Windows. The
  Python wheel is built and tested on macOS arm64 and Linux x86_64 (Python
  3.9, 3.11 and 3.13), since atm-bd's hooks run on POSIX only.

## Versioning and file formats

- All crates are version 0.1.0. A bd change that breaks the row contract is
  taken up in `sc-beads` and `sc-bd-cli` and released as a semver-major
  version of both.
- The cache file is the BD-CACHE v3 file (`PRAGMA user_version` 3), shared
  with atm-core's `bd_cache.py`; extensions are additive tables and nullable
  columns only. A file at another nonzero version is refused
  (`BD_CACHE.SCHEMA_VERSION`), and the version is never changed on open.
- `sc-cron`'s schedule store has its own file at `user_version` 1.
- The Python reply shapes are versioned with the wheel
  (`bd_cache_rs.__version__`).

## Status

0.1.0, pre-release. Nothing is published yet. The design baseline is
atm-core's BD-CACHE contract; where beads-cache departs from it, an ADR in
[`docs/architecture.md`](docs/architecture.md) records why.

Not yet:

- No `bdc` / `bd-cache` CLI binary. It is planned after sc-runtime
  integration, so there are no Homebrew, Scoop or WinGet packages either.
- No HTTPS MCP server and no `bd serve` HTTP backend (`sc-bd-http`).
- One scope kind only: `phase-<x>` labels.
- No user metadata schema packages (ADR-BDC-0008 is a draft).

## Documentation

- [`docs/architecture.md`](docs/architecture.md): design, crate boundaries
  and ADRs. Each crate's own decisions are in `docs/<crate>/architecture.md`.
- [`docs/requirements.md`](docs/requirements.md): requirements and their
  acceptance evidence, refined in `docs/<crate>/requirements.md`.
- [`CHANGELOG.md`](CHANGELOG.md)

## License

MIT. See [LICENSE](LICENSE).
