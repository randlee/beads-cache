//! `cache tick`: the built-in row's run, once, on demand (REQ-BDC-0720, ADR-SYNC-0008).
//!
//! It runs the same Rust code as the built-in row's scheduled run (lease,
//! drain, every configured scope by its filter, the post-sync hook, the
//! reply), with that row's budget and hook when the row exists, else the
//! default budget and no hook. While the Python row is enabled it is
//! `PRECONDITION` (`details.reason: "python_sync_enabled"`) and starts no bd
//! process. Another run in flight makes it reply `skipped: "in_flight"`
//! (exit 0). Scheduled times are not changed. A signal stops it between
//! steps and kills its hook's tree.

use bd_cache_api::{ApiError, Command};
use bd_cache_sync::{Composed, StopFlag};
use serde_json::Value;

use crate::args::{CommandSpec, Target};
use crate::commands::runtime::{json, parts, runtime, sync_error};

/// `cache tick`.
pub(crate) const COMMAND: CommandSpec = CommandSpec {
    path: "cache tick",
    about: "Run the built-in sync once now: lease, drain, sync every scope, post-sync hook (PRECONDITION while the Python row is the sync).",
    target: Target::Sync(Command::CacheTick),
    args: &[],
    one_of: None,
};

/// Runs one tick and returns its reply.
pub(crate) fn run(composed: Composed, stop: &StopFlag) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let ctx = p.run_context(stop);
    let rt = runtime()?;
    let reply = rt
        .block_on(bd_cache_sync::tick(&p.store, &p.cache, &ctx))
        .map_err(|e| sync_error(&e))?;
    json(&reply)
}
