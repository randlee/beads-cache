//! `cache schedule set|show|enable|disable|clear`: the built-in sync row
//! (REQ-BDC-0720, 0731; ADR-SYNC-0008), run over `bd-cache-sync`.

use std::path::PathBuf;

use bd_cache_api::request::ScheduleSet as SetRequest;
use bd_cache_api::{ApiError, Command};
use bd_cache_sync::{Composed, ScheduleSet};
use serde_json::Value;

use crate::args::{ArgSpec, CommandSpec, Shape, Target, Ty};
use crate::commands::runtime::{parts, runtime};

use Shape::{Many, One};
use Ty::{Int, Str};

/// The `cache schedule` commands.
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "cache schedule set",
        about: "Write the built-in sync row (its enabled flag kept); everything is checked first.",
        target: Target::Sync(Command::ScheduleSet),
        args: &[
            ArgSpec::positional("cron", One, Str, true, "cron", "The cron expression (five-, six- or seven-field, UTC)."),
            ArgSpec::flag("budget-ms", "N", One, Int, false, "budget_ms", "The run's budget (default 25000, 1000-600000)."),
            ArgSpec::flag("hook-reserve-ms", "N", One, Int, false, "hook_reserve_ms", "Kept for the post-sync hook (default 5000)."),
            ArgSpec::flag("post-sync", "MODULE:FUNCTION", One, Str, false, "post_sync", "The post-sync hook."),
            ArgSpec::flag("python", "INTERPRETER", One, Str, false, "python", "The hook's absolute interpreter."),
            ArgSpec::flag("env", "NAME", Many, Str, false, "env", "A variable the hook gets (repeat)."),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule show",
        about: "Both sync rows, the mode, the hook, the lease and each scope's sync state.",
        target: Target::Sync(Command::ScheduleShow),
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule enable",
        about: "Enable the built-in row (SCHEDULE_INVALID while the Python row is enabled).",
        target: Target::Sync(Command::ScheduleEnable),
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule disable",
        about: "Disable the built-in row; its expression and payload are kept.",
        target: Target::Sync(Command::ScheduleDisable),
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule clear",
        about: "End the built-in row.",
        target: Target::Sync(Command::ScheduleClear),
        args: &[],
        one_of: None,
    },
];

/// `schedule set`.
pub(crate) fn set(composed: Composed, req: &SetRequest) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let input = ScheduleSet {
        expr: req.cron().to_owned(),
        budget_ms: req.budget_ms(),
        hook_reserve_ms: req.hook_reserve_ms(),
        post_sync: req.post_sync().map(str::to_owned),
        python: req.python().map(PathBuf::from),
        env: req.env().to_vec(),
    };
    let rt = runtime()?;
    rt.block_on(bd_cache_sync::schedule_set(&p.scheduler(), &input))
        .map_err(|e| ApiError::from(&e))
}

/// `schedule show`.
pub(crate) fn show(composed: Composed) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let rt = runtime()?;
    rt.block_on(bd_cache_sync::schedule_show(&p.store, &p.cache))
        .map_err(|e| ApiError::from(&e))
}

/// `schedule enable` / `disable`.
pub(crate) fn enable(composed: Composed, enabled: bool) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let rt = runtime()?;
    rt.block_on(bd_cache_sync::schedule_enable(&p.scheduler(), enabled))
        .map_err(|e| ApiError::from(&e))
}

/// `schedule clear`.
pub(crate) fn clear(composed: Composed) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let rt = runtime()?;
    rt.block_on(bd_cache_sync::schedule_clear(&p.scheduler()))
        .map_err(|e| ApiError::from(&e))
}
