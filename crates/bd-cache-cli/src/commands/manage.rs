//! Surface (b), the cache management commands (contract §6): their argv rows.
//!
//! `cache tick` is the one surface (b) command that is not a `bd-cache-api`
//! operation; it lives in [`super::tick`].

use bd_cache_api::Command;

use crate::args::{ArgSpec, CommandSpec, Shape, Target, Ty};

use Shape::{Many, One};
use Ty::{Int, Str};

const SEQ: ArgSpec = ArgSpec::positional("seq", One, Int, true, "seq", "The journal entry.");

/// The surface (b) commands other than `cache tick`, in contract order (§6).
pub(crate) const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        path: "cache status",
        about: "The cache file, project, backend fingerprint, journal counts and scopes.",
        target: Target::Api(Command::CacheStatus),
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache health",
        about: "ok, degraded or down; with --probe, the backend check (may rebuild).",
        target: Target::Api(Command::CacheHealth),
        args: &[
            ArgSpec::switch(
                "probe",
                "probe",
                "Run the backend check: record bd's fingerprint; a changed schema rebuilds the cache.",
            ),
            ArgSpec::flag(
                "max-pending-age-s",
                "N",
                One,
                Int,
                false,
                "max_pending_age_s",
                "Degraded when the oldest pending entry is older than N seconds (default 300).",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache scopes",
        about: "Every scope the cache knows, with its sync state (no bd call).",
        target: Target::Api(Command::CacheScopes),
        args: &[],
        one_of: None,
    },
    CommandSpec {
        path: "cache refresh scopes",
        about: "Sync scopes from bd; with none named, every known scope.",
        target: Target::Api(Command::CacheRefreshScopes),
        args: &[
            ArgSpec::positional(
                "scopes",
                Many,
                Str,
                false,
                "scopes",
                "The scopes to sync, in this order.",
            ),
            ArgSpec::flag(
                "scope",
                "S",
                Many,
                Str,
                false,
                "scopes",
                "A scope to sync (the same as a positional scope).",
            ),
            ArgSpec::flag(
                "timeout-ms",
                "N",
                One,
                Int,
                false,
                "timeout_ms",
                "Each scope's sync deadline (default: the per-call cap).",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache refresh issues",
        about: "Re-read beads from bd (REQ-BDC-0907).",
        target: Target::Api(Command::CacheRefreshIssues),
        args: &[
            ArgSpec::positional("ids", Many, Str, true, "ids", "The beads to re-read."),
            ArgSpec::flag(
                "timeout-ms",
                "N",
                One,
                Int,
                false,
                "timeout_ms",
                "The read's timeout (default: the miss-fill timeout).",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache flush",
        about: "Apply journaled writes to bd, in seq order.",
        target: Target::Api(Command::CacheFlush),
        args: &[
            ArgSpec::flag(
                "start-gate-ms",
                "N",
                One,
                Int,
                false,
                "start_gate_ms",
                "No entry starts after N ms (default 25000).",
            ),
            ArgSpec::flag(
                "deadline-ms",
                "M",
                One,
                Int,
                false,
                "deadline_ms",
                "No bd call runs past M ms (default 28000; above the start gate).",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache evict",
        about: "Drop a scope's rows and sync state (prune).",
        target: Target::Api(Command::CacheEvict),
        args: &[ArgSpec::flag(
            "scope",
            "S",
            One,
            Str,
            true,
            "scope",
            "The scope to evict.",
        )],
        one_of: None,
    },
    CommandSpec {
        path: "cache journal get",
        about: "One journal entry, by seq or by idempotency key.",
        target: Target::Api(Command::JournalGet),
        args: &[
            ArgSpec::positional("seq", One, Int, false, "seq", "The entry's seq."),
            ArgSpec::flag(
                "idempotency-key",
                "K",
                One,
                Str,
                false,
                "idempotency_key",
                "The entry's idempotency key (after a lost reply).",
            ),
        ],
        one_of: Some(&["seq", "idempotency-key"]),
    },
    CommandSpec {
        path: "cache journal list",
        about: "Journal entries in one state (default escalated), by seq.",
        target: Target::Api(Command::JournalList),
        args: &[
            ArgSpec::flag(
                "state",
                "pending|escalated",
                One,
                Str,
                false,
                "state",
                "The state listed (default escalated).",
            ),
            ArgSpec::switch(
                "unannounced",
                "unannounced",
                "Only escalations not yet announced.",
            ),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache journal resolve",
        about: "Resolve an escalated entry with a note.",
        target: Target::Api(Command::JournalResolve),
        args: &[
            SEQ,
            ArgSpec::flag("note", "T", One, Str, true, "note", "The resolution note."),
            ArgSpec::flag("actor", "A", One, Str, true, "actor", "Who resolves."),
        ],
        one_of: None,
    },
    CommandSpec {
        path: "cache journal announce",
        about: "Mark an escalated entry as announced.",
        target: Target::Api(Command::JournalAnnounce),
        args: &[SEQ],
        one_of: None,
    },
];
