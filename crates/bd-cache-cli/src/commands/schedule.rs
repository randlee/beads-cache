//! `cache schedule set|show|enable|disable|clear`: the built-in sync row
//! (REQ-BDC-0720, 0731; ADR-SYNC-0008). Their argv rows; they run through
//! `bd_cache_sync::execute` (`commands::run`).

use bd_cache_api::Command;

use crate::args::{ArgSpec, CommandSpec, Shape, Ty};

use Shape::{Many, One};
use Ty::{Int, Str};

/// The `cache schedule` commands.
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "cache schedule set",
        about: "Write the built-in sync row (its enabled flag kept); everything is checked first.",
        command: Command::ScheduleSet,
        args: &[
            ArgSpec::positional(
                "cron",
                One,
                Str,
                true,
                "cron",
                "The cron expression (five-, six- or seven-field, UTC).",
            ),
            ArgSpec::flag(
                "budget-ms",
                "N",
                One,
                Int,
                false,
                "budget_ms",
                "The run's budget (default 25000, 1000-600000).",
            ),
            ArgSpec::flag(
                "hook-reserve-ms",
                "N",
                One,
                Int,
                false,
                "hook_reserve_ms",
                "Kept for the post-sync hook (default 5000).",
            ),
            ArgSpec::flag(
                "post-sync",
                "MODULE:FUNCTION",
                One,
                Str,
                false,
                "post_sync",
                "The post-sync hook.",
            ),
            ArgSpec::flag(
                "python",
                "INTERPRETER",
                One,
                Str,
                false,
                "python",
                "The hook's absolute interpreter.",
            ),
            ArgSpec::flag(
                "env",
                "NAME",
                Many,
                Str,
                false,
                "env",
                "A variable the hook gets (repeat).",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule show",
        about: "Both sync rows, the mode, the hook, the lease and each scope's sync state.",
        command: Command::ScheduleShow,
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule enable",
        about: "Enable the built-in row (SCHEDULE_INVALID while the Python row is enabled).",
        command: Command::ScheduleEnable,
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule disable",
        about: "Disable the built-in row; its expression and payload are kept.",
        command: Command::ScheduleDisable,
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache schedule clear",
        about: "End the built-in row.",
        command: Command::ScheduleClear,
        args: &[],
        one_of: None,
    },
];
