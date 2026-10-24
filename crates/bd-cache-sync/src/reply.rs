//! The tick's reply (REQ-BDC-0706).

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
/// use bd_cache_sync::{TickOutcome, TickReply};
/// let reply = TickReply::refused_with("[tick] team is missing");
/// assert_eq!(reply.outcome, TickOutcome::Refused);
/// assert_eq!(serde_json::to_value(&reply).unwrap()["outcome"], "refused");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TickReply {
    /// `pass-through`, or `refused` with the list in [`refused`](Self::refused).
    pub outcome: TickOutcome,
    /// What was refused: configuration problems, unusable phase files, and
    /// messages not delivered or with no lead to go to.
    pub refused: Vec<String>,
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
            busy: false,
            poisoned: false,
        }
    }

    /// A tick that refused before doing any work, for `reason`.
    #[must_use]
    pub fn refused_with(reason: impl Into<String>) -> Self {
        let mut reply = Self::empty(TickOutcome::Refused);
        reply.refused.push(reason.into());
        reply
    }

    /// A tick skipped because an earlier tick still holds the cache.
    #[must_use]
    pub fn busy() -> Self {
        let mut reply = Self::empty(TickOutcome::Refused);
        reply.busy = true;
        reply
            .refused
            .push("busy: an earlier tick still holds the cache".to_owned());
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
