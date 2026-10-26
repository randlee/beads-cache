# sc-cron

A SQLite-backed cron scheduler for tokio, with generation fencing, per-schedule
jitter and missed-tick handling.

`sc-cron` runs a caller's `Job` on cron schedules kept in a `ScheduleStore`.
It was extracted from an earlier in-house scheduler and carries no
consumer-specific type: a schedule's input is an opaque JSON payload, and every
failure is a `CronError` with a stable `code()`. It is a prototype for sc-runtime, which it
is expected to move to unchanged.

- **Validated input.** `ScheduleName` (at most 128 bytes), `CronExpr` (in UTC:
  standard five-field crontab, minute first; or the `cron` crate's six or
  seven fields with seconds first; or `@hourly`, `@daily` and the like),
  `RunTimeout` (more than zero, at most 24 h) and the JSON payload (at most
  64 KiB).
- **Start.** `Scheduler::start` checks that the expression has a future
  occurrence before anything is written, draws a fixed jitter for the
  schedule and stores its first run time.
- **The worker.** `Scheduler::spawn_worker` starts one tokio task that runs
  every active, enabled schedule. Before each run it asks
  `Job::should_stop`, which answers `Continue`, `Disable(reason)` or
  `Delete(reason)`. It bounds the stop check and the run by the schedule's
  timeout on the monotonic clock (recording `RunOutcome::TimedOut` without
  delaying the next tick), skips a tick while the previous run is still in
  flight, and handles missed ticks per `MissedTickPolicy` (`Coalesce` by
  default, or `Skip`).
- **Enabled flag.** `Scheduler::set_enabled(name, bool)` turns a schedule off
  and on again without rewriting its row; a disabled schedule never fires.
- **Python hook target.** `Scheduler::spawn_worker_for(HookTarget::Python(..))`
  runs `<interpreter> -I -m <module>` from a temporary directory for the
  events `cron.stop` and `cron.tick`, with one JSON object on stdin. Every
  hook is killed, with everything it started, at the schedule's timeout
  capped by the time left before the run's deadline, and its output is
  capped.
- **Fencing.** Every run-state write names `(id, generation)`. A run from an
  older generation is fenced and records nothing, so a stale run never
  writes over a restarted schedule.
- **Stores read no clock.** Every instant is a `Timestamp` argument.
  `SqliteScheduleStore` is the rusqlite implementation (SQLite bundled), on a
  database file of its own (`PRAGMA user_version` = `SCHEMA_VERSION`, 2; a
  version 1 file gains the `enabled` column when opened).
  `list_active` skips a row that does not decode, with a `tracing` warning,
  so the other schedules keep running; `get` on that row still returns the
  `Corrupt` error.
- **Injected time and randomness**, through `Clock` and `JitterSource`.

## Example

The crate's `Job` trait uses `async-trait`, so a caller needs `async-trait`,
`serde_json` and `tokio` as well.

```rust,no_run
use std::sync::Arc;
use std::time::{Duration, Instant};

use sc_cron::{
    Job, RunOutcome, ScheduleSpec, Scheduler, SqliteScheduleStore, StopDecision, SystemClock,
    SystemJitter,
};

struct Sync;

#[async_trait::async_trait]
impl Job for Sync {
    async fn run(&self, payload: &serde_json::Value, deadline: Instant) -> RunOutcome {
        let _ = (payload, deadline); // do the work, finishing by `deadline`
        RunOutcome::Ok
    }

    async fn should_stop(&self, _payload: &serde_json::Value) -> StopDecision {
        StopDecision::Continue
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), sc_cron::CronError> {
    let scheduler = Scheduler::new(
        Arc::new(SqliteScheduleStore::open("cron.db")?),
        Arc::new(SystemClock),
        Arc::new(SystemJitter),
    );
    // Every 30 seconds; each run (stop check included) is cut off after 25 s.
    let spec = ScheduleSpec::new("sync", "*/30 * * * * *", Duration::from_secs(25), serde_json::json!({}))?;
    scheduler.start_async(spec).await?;
    let worker = scheduler.spawn_worker(Arc::new(Sync));
    // ... later
    worker.shutdown(tokio::time::Instant::now() + Duration::from_secs(30)).await;
    Ok(())
}
```

Runtime notes:

- `Scheduler::start` and `Scheduler::stop` call the store synchronously. From
  async code, use `start_async` and `stop_async`, which run on tokio's
  blocking pool.
- Run timeouts are cooperative: a job should finish by the `deadline` it is
  handed. Use a multi-thread tokio runtime so a misbehaving job cannot stall
  the worker.
- Stop the worker by awaiting `WorkerHandle::shutdown`. Dropping the handle
  drains detached, and in-flight outcomes can be lost at process exit.

## Feature flags

| Feature | Enables |
|---|---|
| `test-utils` | `ManualClock` (a deterministic clock whose parks tell a test the worker has settled) and `SeqJitter` (a scripted jitter source) |

## Links

- Repository: <https://github.com/randlee/beads-cache>
- Architecture: [`docs/sc-cron/architecture.md`](https://github.com/randlee/beads-cache/blob/develop/docs/sc-cron/architecture.md)
- Requirements: [`docs/sc-cron/requirements.md`](https://github.com/randlee/beads-cache/blob/develop/docs/sc-cron/requirements.md)

MSRV: Rust 1.98.1 (edition 2024). License: MIT.
