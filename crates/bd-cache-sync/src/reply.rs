//! The tick's reply (REQ-BDC-0706).

use std::fmt;

use bd_cache_model::{DrainReport, PhaseName, Seq, SyncReport};
use serde::Serialize;

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
    /// A phase file, or a configured phase's file, is unusable.
    PhaseFile,
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
            Self::PhaseFile => "phase_file",
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
            Self::BadMember => "use a name without spaces, '@' or a leading '-'",
            Self::MalformedPhases => {
                "use letters, digits, '-', '_' and '.', with or without the phase- prefix"
            }
            Self::EmptyPhases => {
                "list at least one phase, or remove [tick] phases to sync every phase"
            }
            Self::PhasesDirUnreadable => "create the directory or set [tick] phases_dir",
            Self::PhaseFile => "correct or remove the file named, or add the missing phase file",
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
            detail: detail.into(),
            fix: code.fix().to_owned(),
        }
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
        /// The stable cause key now recorded as the phase's alert.
        cause: String,
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
    pub lead: String,
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
    /// Phases whose root could not be read this tick, so they count as inactive.
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
