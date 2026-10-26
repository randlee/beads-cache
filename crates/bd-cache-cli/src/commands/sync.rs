//! `cache sync use-python|use-builtin`: the switch between the sync rows
//! (REQ-BDC-0724; ADR-SYNC-0008), run over `bd-cache-sync`.
//!
//! A signal to `use-python` during its immediate run kills the script's tree,
//! restores the state held before the command, and the process exits
//! `128 + signal` (the `StopFlag` is the run's kill flag here).

use std::path::PathBuf;

use bd_cache_api::request::SyncUsePython;
use bd_cache_api::{ApiError, Command};
use bd_cache_sync::{Composed, StopFlag, UsePython};
use serde_json::Value;

use crate::args::{ArgSpec, CommandSpec, Shape, Target, Ty};
use crate::commands::runtime::{parts, runtime};

use Shape::{Many, One};
use Ty::{Int, Str};

/// The `cache sync` commands.
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "cache sync use-python",
        about: "Make a consumer's Python script the sync: write its row enabled, run it at once, roll back on failure.",
        target: Target::Sync(Command::SyncUsePython),
        args: &[
            ArgSpec::flag("python", "INTERPRETER", One, Str, true, "python", "The absolute interpreter."),
            ArgSpec::flag("module", "MODULE", One, Str, true, "module", "The script's module, run as -I -m <module>."),
            ArgSpec::flag("cron", "EXPR", One, Str, false, "cron", "The row's expression (default: the built-in row's)."),
            ArgSpec::flag("timeout-ms", "N", One, Int, false, "timeout_ms", "The script's timeout (default 60000, 1000-600000)."),
            ArgSpec::flag("env", "NAME", Many, Str, false, "env", "A variable the script gets (repeat)."),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache sync use-builtin",
        about: "Make the built-in row the sync again (takes no lease; always works when a built-in row exists).",
        target: Target::Sync(Command::SyncUseBuiltin),
        args: &[],
        one_of: None,
    },
];

/// `sync use-python`.
pub(crate) fn use_python(composed: Composed, req: &SyncUsePython, stop: &StopFlag) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let input = UsePython {
        python: PathBuf::from(req.python()),
        module: req.module().to_owned(),
        cron: req.cron().map(str::to_owned),
        timeout_ms: req.timeout_ms(),
        env: req.env().to_vec(),
    };
    let ctx = p.sync_context(stop, stop.clone());
    let rt = runtime()?;
    rt.block_on(bd_cache_sync::sync_use_python(&p.scheduler(), &ctx, &input))
        .map_err(|e| ApiError::from(&e))
}

/// `sync use-builtin`.
pub(crate) fn use_builtin(composed: Composed) -> Result<Value, ApiError> {
    let p = parts(composed)?;
    let rt = runtime()?;
    rt.block_on(bd_cache_sync::sync_use_builtin(&p.scheduler()))
        .map_err(|e| ApiError::from(&e))
}
