//! `cache cron run [--once] [--shutdown-timeout-ms N]` (REQ-BDC-0720, 0730; ADR-SYNC-0008).
//!
//! `cron run` runs sc-cron's `Scheduler` worker over the cache file's
//! schedule table, with bd-cache-sync's resolver and `MissedTickPolicy::Skip`,
//! on a multi-thread runtime, until `SIGINT`, `SIGTERM` or `SIGHUP` (Ctrl-C
//! or Ctrl-Break on Windows): then it starts no new run, waits for the runs
//! in flight up to `--shutdown-timeout-ms`, kills those still going with
//! their process trees, releases the lease, and the process exits `128 +
//! signal` (130 on Windows). Its stderr subscriber is the process default, so
//! sc-cron's events from every thread are logged. `--once` runs what is due,
//! waits for each run to its own deadline, and exits 0, or 3 with
//! `TICK_RUN_FAILED` naming each failed row.

use std::time::Duration;

use bd_cache_api::request::CronRun;
use bd_cache_api::{ApiError, Command};
use bd_cache_sync::{Composed, StopFlag, cron_run, cron_run_once};
use serde_json::{Value, json};

use crate::args::{ArgSpec, CommandSpec, Shape, Target, Ty};
use crate::commands::runtime::{parts, runtime};

/// The `cache cron` commands.
pub(crate) const COMMANDS: &[CommandSpec] = &[CommandSpec {
    path: "cache cron run",
    about: "Run the cache file's cron rows until a signal; --once runs what is due and exits.",
    target: Target::Sync(Command::CronRun),
    args: &[
        ArgSpec::switch("once", "once", "Run what is due now, wait for it, and exit."),
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

/// Runs `cron run` or `cron run --once`.
pub(crate) fn run(composed: Composed, req: &CronRun, stop: &StopFlag) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let rt = runtime()?;
    if req.once() {
        let ctx = std::sync::Arc::new(p.sync_context(stop, stop.clone()));
        let runs = rt
            .block_on(cron_run_once(&p.scheduler(), ctx))
            .map_err(|e| ApiError::from(&e))?;
        let ran: Vec<Value> = runs
            .iter()
            .map(|r| json!({"row": r.name.as_str(), "outcome": r.outcome.to_string()}))
            .collect();
        return Ok(json!({"once": true, "ran": ran}));
    }
    // The kill flag of `cron run` is its own: a run in flight is waited for
    // up to the shutdown timeout, then aborted (REQ-BDC-0730).
    let ctx = std::sync::Arc::new(p.sync_context(stop, StopFlag::new()));
    let timeout = Duration::from_millis(req.shutdown_timeout_ms());
    let scheduler = p.scheduler();
    let stopped = rt.block_on(async move {
        #[cfg(windows)]
        let _console = console_stop(ctx.stop.clone());
        cron_run(scheduler, ctx, timeout).await
    });
    drop(rt);
    Ok(json!({"once": false, "stopped": true, "drained": stopped.drained, "idle": stopped.idle}))
}

/// Ctrl-C and Ctrl-Break stop `cron run` on Windows (exit 130, REQ-BDC-0730).
#[cfg(windows)]
fn console_stop(stop: StopFlag) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut c = tokio::signal::windows::ctrl_c().ok();
        let mut b = tokio::signal::windows::ctrl_break().ok();
        let ctrl_c = async {
            match c.as_mut() {
                Some(c) => {
                    c.recv().await;
                }
                None => std::future::pending().await,
            }
        };
        let ctrl_break = async {
            match b.as_mut() {
                Some(b) => {
                    b.recv().await;
                }
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            () = ctrl_c => {}
            () = ctrl_break => {}
        }
        // SIGINT's number, so the exit is 130 as for Ctrl-C (REQ-BDC-0730).
        sc_bd_cli::shutdown::request(2);
        stop.stop();
    })
}
