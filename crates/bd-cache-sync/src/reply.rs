//! The tick's reply (REQ-BDC-0706).

use std::fmt;

use bd_cache_model::{DrainReport, PhaseName, SendError, SendErrorKind, Seq, SyncReport};
use serde::Serialize;

use crate::announce::AlertCause;
use crate::phases::MemberName;

/// How a tick ended, in the words of the BD-CACHE cron module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TickOutcome {
    /// Every message was delivered and the configuration is sound.
    #[serde(rename = "pass-through")]
    PassThrough,
    /// Something was refused: the reply's `refused` lists what, and the next
    /// tick retries it (REQ-BDC-0706).
    #[serde(rename = "refused")]
    Refused,
}

/// What kind of thing a tick refused. The code is stable; the detail is text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RefusalCode {
    /// `[tick] team` is missing (REQ-BDC-0701).
    MissingTeam,
    /// `[tick] fallback_lead` is missing (REQ-BDC-0701).
    MissingFallbackLead,
    /// A name cannot be part of an `atm send` address.
    BadMember,
    /// A `[tick] phases` entry is not a phase name (REQ-BDC-0702).
    MalformedPhases,
    /// `[tick] phases` is an empty list.
    EmptyPhases,
    /// The phases directory cannot be read.
    PhasesDirUnreadable,
    /// A `phase-*.toml` name after the prefix is not a phase name.
    PhaseNotNamed,
    /// A phase file cannot be read.
    PhaseFileUnreadable,
    /// A phase file is larger than the cap.
    PhaseFileTooLarge,
    /// A phase file is not a regular file.
    PhaseFileNotRegular,
    /// A phase file is not TOML, has no `root`, or a bad `root` or `lead`.
    PhaseFileInvalid,
    /// A phase in `[tick] phases` has no file.
    PhaseFileMissing,
    /// The phases directory holds more files than are read.
    PhaseFilesTooMany,
    /// A sync-failure message was not delivered (REQ-BDC-0704).
    SyncFailureUndelivered,
    /// An outage message was not delivered (REQ-BDC-0704).
    OutageUndelivered,
    /// An escalation was not delivered (REQ-BDC-0706).
    EscalationUndelivered,
    /// An escalated entry's phase has no lead to tell (REQ-BDC-0706).
    EscalationNoLead,
    /// An earlier tick still holds the cache (REQ-BDC-0703).
    Busy,
    /// The cache mutex was poisoned by an earlier panic and recovered.
    MutexPoisoned,
}

impl RefusalCode {
    /// The stable name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingTeam => "missing_team",
            Self::MissingFallbackLead => "missing_fallback_lead",
            Self::BadMember => "bad_member",
            Self::MalformedPhases => "malformed_phases",
            Self::EmptyPhases => "empty_phases",
            Self::PhasesDirUnreadable => "phases_dir_unreadable",
            Self::PhaseNotNamed => "phase_not_named",
            Self::PhaseFileUnreadable => "phase_file_unreadable",
            Self::PhaseFileTooLarge => "phase_file_too_large",
            Self::PhaseFileNotRegular => "phase_file_not_regular",
            Self::PhaseFileInvalid => "phase_file_invalid",
            Self::PhaseFileMissing => "phase_file_missing",
            Self::PhaseFilesTooMany => "phase_files_too_many",
            Self::SyncFailureUndelivered => "sync_failure_undelivered",
            Self::OutageUndelivered => "outage_undelivered",
            Self::EscalationUndelivered => "escalation_undelivered",
            Self::EscalationNoLead => "escalation_no_lead",
            Self::Busy => "busy",
            Self::MutexPoisoned => "mutex_poisoned",
        }
    }

    /// What the operator can do about it.
    #[must_use]
    pub fn fix(self) -> &'static str {
        match self {
            Self::MissingTeam => {
                "set [tick] team to the ATM team of the phase leads in .beads/bd-cache.toml"
            }
            Self::MissingFallbackLead => {
                "set [tick] fallback_lead to the member told of escalations that belong to no phase"
            }
            Self::BadMember => "use a member name that follows the rule the detail states",
            Self::MalformedPhases => {
                "use letters, digits, '-', '_' and '.', with or without the phase- prefix"
            }
            Self::EmptyPhases => {
                "list at least one phase, or remove [tick] phases to sync every phase"
            }
            Self::PhasesDirUnreadable => "create the directory or set [tick] phases_dir",
            Self::PhaseNotNamed => {
                "rename the file phase-<name>.toml with letters, digits, '-', '_' or '.'"
            }
            Self::PhaseFileUnreadable => "make the file readable by the user running the tick",
            Self::PhaseFileTooLarge => {
                "a phase file holds a root and a lead; remove what else is in it"
            }
            Self::PhaseFileNotRegular => {
                "replace it with a regular file (not a pipe, device or directory)"
            }
            Self::PhaseFileInvalid => {
                "write `root = \"<bead id>\"` and, optionally, `lead = \"<member>\"` as TOML"
            }
            Self::PhaseFileMissing => {
                "add phase-<name>.toml to the phases directory, or remove the name from [tick] phases"
            }
            Self::PhaseFilesTooMany => "remove the phase files that are not in use",
            Self::SyncFailureUndelivered
            | Self::OutageUndelivered
            | Self::EscalationUndelivered => {
                "check that atm and its daemon answer; the message is retried next tick"
            }
            Self::EscalationNoLead => {
                "add a lead to the phase file; the entry is retried next tick"
            }
            Self::Busy => "none; an earlier tick is still running and this one was skipped",
            Self::MutexPoisoned => {
                "none; the cache was recovered, look for the earlier panic in the log"
            }
        }
    }
}

impl fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One thing a tick refused, with a stable code, the detail and the way out.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::{Refusal, RefusalCode};
/// let r = Refusal::new(RefusalCode::MissingTeam, "[tick] team is missing");
/// assert_eq!(r.code, RefusalCode::MissingTeam);
/// assert!(r.to_string().contains("fix:"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    /// What kind of refusal.
    pub code: RefusalCode,
    /// For an undelivered message, why the sender failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_kind: Option<SendErrorKind>,
    /// What exactly, for a person.
    pub detail: String,
    /// What to do about it.
    pub fix: String,
}

impl Refusal {
    /// A refusal with the code's standard fix.
    #[must_use]
    pub fn new(code: RefusalCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            send_kind: None,
            detail: detail.into(),
            fix: code.fix().to_owned(),
        }
    }

    /// This refusal for a message the sender could not deliver: it keeps the
    /// sender's kind and fix.
    #[must_use]
    pub fn undelivered(code: RefusalCode, what: &str, e: &SendError) -> Self {
        let mut r = Self::new(code, format!("{what}: {e}")).with_fix(e.fix());
        r.send_kind = Some(e.kind);
        r
    }

    /// This refusal with another fix text.
    #[must_use]
    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = fix.into();
        self
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}; fix: {}", self.code, self.detail, self.fix)
    }
}

/// What an announcement told the lead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnnouncementKind {
    /// An escalated journal entry, with the `resolve` command (REQ-BDC-0705).
    Escalation {
        /// The entry.
        seq: Seq,
    },
    /// A sync failure whose cause differs from the phase's alert (REQ-BDC-0704).
    SyncFailure {
        /// The phase.
        phase: PhaseName,
        /// The cause now recorded as the phase's alert.
        cause: AlertCause,
    },
    /// The start of a backend outage (REQ-BDC-0704, ADR-BDC-0005).
    Outage {
        /// The phases whose alert now reads `backend_unavailable`.
        phases: Vec<PhaseName>,
    },
}

/// One message the tick delivered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Announcement {
    /// What it was about.
    #[serde(flatten)]
    pub kind: AnnouncementKind,
    /// Who it went to.
    pub lead: MemberName,
}

/// The result of one tick.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::{Refusal, RefusalCode, TickOutcome, TickReply};
/// let reply = TickReply::refused_with(Refusal::new(RefusalCode::MissingTeam, "[tick] team is missing"));
/// assert_eq!(reply.outcome, TickOutcome::Refused);
/// assert_eq!(serde_json::to_value(&reply).unwrap()["outcome"], "refused");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TickReply {
    /// `pass-through`, or `refused` with the list in [`refused`](Self::refused).
    pub outcome: TickOutcome,
    /// What was refused: configuration problems, unusable phase files, and
    /// messages not delivered or with no lead to go to.
    pub refused: Vec<Refusal>,
    /// The drain; `None` when the tick did not get as far.
    pub drain: Option<DrainReport>,
    /// The phases found active, in sync order.
    pub active: Vec<PhaseName>,
    /// One report per phase that was synced.
    pub synced: Vec<SyncReport>,
    /// Active phases whose sync did not start before the budget's margin (REQ-BDC-0703).
    pub deferred: Vec<PhaseName>,
    /// Phases whose root could not be read this tick, or that bd did not
    /// return, so they count as inactive (and never as closed: ADR-SYNC-0004).
    pub unreadable_roots: Vec<PhaseName>,
    /// The messages delivered this tick.
    pub sent: Vec<Announcement>,
    /// Messages not started because the tick had too little time left or was
    /// asked to stop; they stay unmarked and are sent by the next tick.
    pub unsent: u32,
    /// The tick was asked to stop (`StopFlag`) and ended early.
    pub stopped: bool,
    /// The cache was in use by an earlier tick's blocking task; this tick did
    /// nothing (REQ-BDC-0703).
    pub busy: bool,
    /// The cache's mutex had been poisoned by a panic; it was recovered for
    /// this tick (ADR-SYNC-0002).
    pub poisoned: bool,
}

impl TickReply {
    /// A reply that did nothing, with this outcome.
    fn empty(outcome: TickOutcome) -> Self {
        Self {
            outcome,
            refused: Vec::new(),
            drain: None,
            active: Vec::new(),
            synced: Vec::new(),
            deferred: Vec::new(),
            unreadable_roots: Vec::new(),
            sent: Vec::new(),
            unsent: 0,
            stopped: false,
            busy: false,
            poisoned: false,
        }
    }

    /// A tick that refused before doing any work, for `reason`.
    #[must_use]
    pub fn refused_with(reason: Refusal) -> Self {
        let mut reply = Self::empty(TickOutcome::Refused);
        reply.refused.push(reason);
        reply
    }

    /// A tick skipped because an earlier tick still holds the cache.
    #[must_use]
    pub fn busy() -> Self {
        let mut reply = Self::refused_with(Refusal::new(
            RefusalCode::Busy,
            "an earlier tick still holds the cache",
        ));
        reply.busy = true;
        reply
    }

    /// A tick that has done nothing yet; `finish` sets the outcome.
    pub(crate) fn started() -> Self {
        Self::empty(TickOutcome::PassThrough)
    }

    /// Sets the outcome from the refusals collected.
    pub(crate) fn finish(mut self) -> Self {
        self.outcome = if self.refused.is_empty() {
            TickOutcome::PassThrough
        } else {
            TickOutcome::Refused
        };
        self
    }
}
