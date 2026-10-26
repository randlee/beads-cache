//! `cache cron run [--once] [--shutdown-timeout-ms N]` (REQ-BDC-0720, 0730; ADR-SYNC-0008).
//!
//! `cron run` runs `bd_cache_sync`'s `Schedules::cron_run` (sc-cron's
//! worker over the cache file's schedule table, with the sync resolver and
//! `MissedTickPolicy::Skip`) on a multi-thread runtime until `SIGINT`,
//! `SIGTERM` or `SIGHUP` (Ctrl-C or Ctrl-Break on Windows). On a signal it
//! starts no new run and gives the runs in flight, their `bd` calls, hooks
//! and scripts, up to `--shutdown-timeout-ms` (`T`); only then are the `bd`
//! calls stopped and the trees still going killed, and the lease released;
//! the process exits `128 + signal` (130 on Windows). A stop takes at most
//! `2 × T`: the runtime, its blocking pool included, is shut down within the
//! second `T` (`Runtime::shutdown_timeout`). Its stderr subscriber is the
//! process default, at `INFO` unless `-v`, `-q` or `BD_CACHE__LOG` says
//! otherwise, so sc-cron's events and each run's steps are logged from every
//! thread. `--once` runs what is due, waits for each run to its own deadline,
//! and exits 0, or 3 with `TICK_RUN_FAILED` naming each failed row
//! (`details.rows`).

use std::time::{Duration, Instant};

use bd_cache_api::request::CronRun;
use bd_cache_api::{ApiError, Command};
use bd_cache_sync::{FrontEnd, SystemSchedules};
use serde_json::{Value, json};

use crate::args::{ArgSpec, CommandSpec, Shape, Ty};
use crate::commands::runtime::runtime;
use crate::commands::signal::Signals;

/// The `cache cron` commands.
pub(crate) const COMMANDS: &[CommandSpec] = &[CommandSpec {
    path: "cache cron run",
    about: "Run the cache file's cron rows until a signal; --once runs what is due and exits.",
    command: Command::CronRun,
    args: &[
        ArgSpec::switch(
            "once",
            "once",
            "Run what is due now, wait for it, and exit.",
        ),
        ArgSpec::flag(
            "shutdown-timeout-ms",
            "N",
            Shape::One,
            Ty::Int,
            false,
            "shutdown_timeout_ms",
            "On a signal, how long the runs in flight may finish (default 10000, 0-600000).",
        ),
    ],
    one_of: None,
}];

/// Runs `cron run` or `cron run --once` over `schedules`.
pub(crate) fn run(
    schedules: &SystemSchedules,
    req: &CronRun,
    signals: &Signals,
) -> Result<Value, ApiError> {
    let rt = runtime()?;
    if req.once() {
        let once = rt
            .block_on(schedules.cron_run_once(signals.stop().clone()))
            .map_err(|e| FrontEnd::Cli.api_error(&e))?;
        return match once.error() {
            Some(e) => Err(e),
            None => Ok(once.data()),
        };
    }
    // The runs in flight get the grace period with their bd calls running.
    signals.defer_bd_kill();
    let timeout = Duration::from_millis(req.shutdown_timeout_ms());
    let stopped = rt.block_on(async {
        #[cfg(windows)]
        let _console = crate::commands::signal::console_stop(signals);
        schedules
            .cron_run(signals.stop().clone(), timeout, || signals.kill_bd())
            .await
    });
    // The blocking pool (a killed step, a lease release) within the rest of the bound.
    rt.shutdown_timeout(stopped.finish_by.saturating_duration_since(Instant::now()));
    if stopped.worker_ended {
        return Err(ApiError::internal(
            "the cron worker ended without a signal (see the log); cron run stopped",
        ));
    }
    Ok(json!({"once": false, "stopped": true, "drained": stopped.drained, "idle": stopped.idle}))
}
