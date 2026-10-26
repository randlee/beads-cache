//! `cache sync use-python|use-builtin`: the switch between the sync rows
//! (REQ-BDC-0724; ADR-SYNC-0008), run over `bd-cache-sync`.
//!
//! Their argv rows; they run through `bd_cache_sync::execute`
//! (`commands::run`). A signal to `use-python` during its immediate run
//! kills the script's tree, restores the state held before the command, and
//! the process exits `128 + signal` (the stop flag is the run's kill flag).

use bd_cache_api::Command;

use crate::args::{ArgSpec, CommandSpec, Shape, Ty};

use Shape::{Many, One};
use Ty::{Int, Str};

/// The `cache sync` commands.
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "cache sync use-python",
        about: "Make a consumer's Python script the sync: write its row enabled, run it at once, roll back on failure.",
        command: Command::SyncUsePython,
        args: &[
            ArgSpec::flag(
                "python",
                "INTERPRETER",
                One,
                Str,
                true,
                "python",
                "The absolute interpreter.",
            ),
            ArgSpec::flag(
                "module",
                "MODULE",
                One,
                Str,
                true,
                "module",
                "The script's module, run as -I -m <module>.",
            ),
            ArgSpec::flag(
                "cron",
                "EXPR",
                One,
                Str,
                false,
                "cron",
                "The row's expression (default: the built-in row's).",
            ),
            ArgSpec::flag(
                "timeout-ms",
                "N",
                One,
                Int,
                false,
                "timeout_ms",
                "The script's timeout (default 60000, 1000-600000).",
            ),
            ArgSpec::flag(
                "env",
                "NAME",
                Many,
                Str,
                false,
                "env",
                "A variable the script gets (repeat).",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache sync use-builtin",
        about: "Make the built-in row the sync again (takes no lease; always works when a built-in row exists).",
        command: Command::SyncUseBuiltin,
        args: &[],
        one_of: None,
    },
];
