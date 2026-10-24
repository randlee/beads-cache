//! Announcements: sync failures, outages and escalations (REQ-BDC-0704..0706).
//!
//! Every message goes through the [`EscalationSender`] port, inside the
//! tick's budget: before each send the tick checks its stop flag and the time
//! left before `T`, passes the sender the time left, and defers the message
//! when there is too little (it stays unmarked and the next tick sends it).
//! A message that is not delivered leaves its marker unset (the entry stays
//! unannounced, the alert is not recorded), so the next tick sends it again,
//! and the tick replies `refused` with the list (REQ-BDC-0706).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainLock, EscalationSender, JournalEntry, Outage, PhaseName, ScopeId,
    SendError, SyncCause,
};
use sc_beads::CacheBackend;

use crate::control::Budget;
use crate::phases::{MemberName, Phase, TickSettings};
use crate::reply::{Announcement, AnnouncementKind, Refusal, RefusalCode, TickReply};

/// The alert cause key of an outage (ADR-BDC-0005): each active scope's lead
/// hears it once, and the alert is reset when the outage ends.
pub const OUTAGE_CAUSE: &str = "backend_unavailable";

/// The key recorded for a failed sync that names no underlying code.
const SYNC_FAILED_KEY: &str = "BD_CACHE.SYNC_FAILED";

/// The most bytes of an error text embedded in a message (a backend's stderr
/// can be megabytes; a lead needs the first lines).
const MAX_TEXT_BYTES: usize = 2048;

/// What a scope's sync alert says was last announced (`sync_alert:<phase>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertCause {
    /// The backend is unreachable ([`OUTAGE_CAUSE`]).
    BackendUnavailable,
    /// A sync failed with this underlying code.
    Sync(SyncCause),
    /// A sync failed and named no underlying code.
    SyncFailed,
}

impl AlertCause {
    /// The stable key stored in the alert.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::BackendUnavailable => OUTAGE_CAUSE,
            Self::Sync(c) => c.as_str(),
            Self::SyncFailed => SYNC_FAILED_KEY,
        }
    }

    /// The cause of a failed sync.
    #[must_use]
    pub fn of(e: &CacheError) -> Self {
        match e {
            CacheError::SyncFailed { cause: Some(c), .. } => Self::Sync(*c),
            other => SyncCause::of(other).map_or(Self::SyncFailed, Self::Sync),
        }
    }
}

/// Clips `text` to [`MAX_TEXT_BYTES`] on a character boundary.
fn clip(text: &str) -> String {
    if text.len() <= MAX_TEXT_BYTES {
        return text.to_owned();
    }
    let mut end = MAX_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} ... (truncated)", text.get(..end).unwrap_or_default())
}

/// The lead of a phase for alerts about the backend: its own, else the fallback.
fn alert_lead<'a>(phase: &'a Phase, settings: &'a TickSettings) -> &'a MemberName {
    phase
        .lead
        .as_ref()
        .unwrap_or_else(|| settings.fallback_lead())
}

/// A refusal for a message the sender could not deliver, with the sender's own fix.
fn undelivered(code: RefusalCode, what: &str, e: &SendError) -> Refusal {
    Refusal::new(code, format!("{what}: {e}")).with_fix(e.fix())
}

/// Sends one message within the tick's budget.
///
/// `None` when the budget is spent or the tick was asked to stop: the message
/// is counted in `reply.unsent`, not sent, not refused.
fn deliver(
    sender: &dyn EscalationSender,
    budget: &Budget<'_>,
    lead: &MemberName,
    settings: &TickSettings,
    body: &str,
    reply: &mut TickReply,
) -> Option<Result<(), SendError>> {
    let Some(left) = budget.send_budget() else {
        reply.unsent = reply.unsent.saturating_add(1);
        reply.stopped |= budget.stop.is_stopped();
        return None;
    };
    Some(sender.send_within(lead.as_str(), settings.team().as_str(), body, left))
}

/// Announces a failed sync of `phase` once per cause (REQ-BDC-0704).
///
/// A cause that is unreachability is the outage's to announce, not the
/// phase's.
pub(crate) fn sync_failure<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    sender: &dyn EscalationSender,
    settings: &TickSettings,
    budget: &Budget<'_>,
    phase: &Phase,
    error: &CacheError,
    reply: &mut TickReply,
) -> Result<(), CacheError> {
    let cause = AlertCause::of(error);
    if cause == AlertCause::Sync(SyncCause::BdUnavailable) {
        return Ok(());
    }
    let scope = phase.scope();
    if cache.sync_alert(&scope)?.as_deref() == Some(cause.key()) {
        return Ok(());
    }
    let lead = alert_lead(phase, settings);
    let body = format!(
        "bd-cache: the sync of phase {name} failed.\n\
         Error: {error}\n\
         Cause: {key}\n\
         This cause is reported once; you will hear again only if the cause changes or the \
         sync recovers and fails again.",
        name = phase.name,
        error = clip(&error.to_string()),
        key = cause.key(),
    );
    match deliver(sender, budget, lead, settings, &body, reply) {
        None => {}
        Some(Ok(())) => {
            cache.set_sync_alert(&scope, Some(cause.key()))?;
            reply.sent.push(Announcement {
                kind: AnnouncementKind::SyncFailure {
                    phase: phase.name.clone(),
                    cause: cause.key().to_owned(),
                },
                lead: lead.to_string(),
            });
        }
        Some(Err(e)) => reply.refused.push(undelivered(
            RefusalCode::SyncFailureUndelivered,
            &format!("sync failure of phase {}", phase.name),
            &e,
        )),
    }
    Ok(())
}

/// Clears the alert of a phase whose sync succeeded.
pub(crate) fn sync_succeeded<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    scope: &ScopeId,
) -> Result<(), CacheError> {
    if cache.sync_alert(scope)?.is_some() {
        cache.set_sync_alert(scope, None)?;
    }
    Ok(())
}

/// The text of an outage message.
fn outage_body(outage: &Outage, phases: &[&PhaseName]) -> String {
    let names: Vec<&str> = phases.iter().map(|p| p.as_str()).collect();
    format!(
        "bd-cache: the beads backend is unreachable (since {since}).\n\
         Latest error: {error}\n\
         Affected phases: {phases}.\n\
         Cached reads continue and are marked stale, and writes stay journaled; nothing is lost \
         and no write is escalated while this lasts. This is reported once per outage.",
        since = outage.since,
        error = clip(&outage.last_error),
        phases = names.join(", "),
    )
}

/// Announces an outage to the leads of `targets` (REQ-BDC-0704, ADR-BDC-0005,
/// ADR-SYNC-0003), or resets the alerts when it has ended.
///
/// `targets` are the active phases and the phases whose root could not be
/// read, never a phase known to be closed. One message goes to each distinct
/// lead, covering that lead's phases whose alert does not yet read
/// `backend_unavailable`.
pub(crate) fn outage<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    sender: &dyn EscalationSender,
    settings: &TickSettings,
    budget: &Budget<'_>,
    targets: &[&Phase],
    all: &[Phase],
    reply: &mut TickReply,
) -> Result<(), CacheError> {
    let status = cache.status()?.store;
    let Some(outage) = status.outage else {
        // Over: reset every alert this outage set, without a message.
        let mut scopes: Vec<ScopeId> = status.scopes.iter().map(|s| s.sync.scope.clone()).collect();
        scopes.extend(all.iter().map(Phase::scope));
        for scope in scopes {
            if cache.sync_alert(&scope)?.as_deref() == Some(OUTAGE_CAUSE) {
                cache.set_sync_alert(&scope, None)?;
            }
        }
        return Ok(());
    };
    let mut by_lead: BTreeMap<&MemberName, Vec<&Phase>> = BTreeMap::new();
    for phase in targets {
        if cache.sync_alert(&phase.scope())?.as_deref() != Some(OUTAGE_CAUSE) {
            by_lead
                .entry(alert_lead(phase, settings))
                .or_default()
                .push(phase);
        }
    }
    for (lead, phases) in by_lead {
        let names: Vec<&PhaseName> = phases.iter().map(|p| &p.name).collect();
        let body = outage_body(&outage, &names);
        match deliver(sender, budget, lead, settings, &body, reply) {
            None => {}
            Some(Ok(())) => {
                for phase in &phases {
                    cache.set_sync_alert(&phase.scope(), Some(OUTAGE_CAUSE))?;
                }
                reply.sent.push(Announcement {
                    kind: AnnouncementKind::Outage {
                        phases: names.into_iter().cloned().collect(),
                    },
                    lead: lead.to_string(),
                });
            }
            Some(Err(e)) => reply.refused.push(undelivered(
                RefusalCode::OutageUndelivered,
                &format!("outage message to {lead}"),
                &e,
            )),
        }
    }
    Ok(())
}

/// The text of an escalation message: the entry, the error and the recovery.
fn escalation_body(e: &JournalEntry) -> String {
    let mut body = format!(
        "bd-cache: write #{seq} to {bead} ({op}, by {actor}) was not applied and needs you.\n",
        seq = e.seq,
        bead = e.bead,
        op = e.op.name(),
        actor = e.actor,
    );
    let _ = write!(
        body,
        "Error: {error}\n\
         Check the bead in bd and set the intended state, then run:\n  \
         bd-cache resolve {seq} --note \"<what you did>\" --actor <you>",
        error = clip(e.last_error.as_deref().unwrap_or("(no reason recorded)")),
        seq = e.seq,
    );
    body
}

/// Sends every escalated entry that has not been announced to its phase's
/// lead, then marks it (REQ-BDC-0705). An entry with no scope goes to the
/// fallback lead; an entry whose phase has no lead is refused (REQ-BDC-0706).
pub(crate) fn escalations<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    sender: &dyn EscalationSender,
    settings: &TickSettings,
    budget: &Budget<'_>,
    phases: &[Phase],
    reply: &mut TickReply,
) -> Result<(), CacheError> {
    for entry in cache.unannounced_escalations()? {
        let lead = match &entry.scope {
            None => Some(settings.fallback_lead()),
            Some(scope) => phases
                .iter()
                .find(|p| p.name == *scope.phase_name())
                .and_then(|p| p.lead.as_ref()),
        };
        let Some(lead) = lead else {
            let phase = entry.scope.as_ref().map_or("-", ScopeId::name);
            reply.refused.push(Refusal::new(
                RefusalCode::EscalationNoLead,
                format!(
                    "escalation #{}: phase {phase} has no lead to tell",
                    entry.seq
                ),
            ));
            continue;
        };
        match deliver(
            sender,
            budget,
            lead,
            settings,
            &escalation_body(&entry),
            reply,
        ) {
            None => {}
            Some(Ok(())) => {
                cache.mark_announced(entry.seq)?;
                reply.sent.push(Announcement {
                    kind: AnnouncementKind::Escalation { seq: entry.seq },
                    lead: lead.to_string(),
                });
            }
            Some(Err(e)) => reply.refused.push(undelivered(
                RefusalCode::EscalationUndelivered,
                &format!("escalation #{}", entry.seq),
                &e,
            )),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use bd_cache_model::testing::{FakeDrainLock, FakeStore};
    use bd_cache_model::{CacheConfig, TickConfig};
    use sc_beads::testing::{FakeBackend, ManualClock};
    use sc_beads::{Clock, Deadline, ProjectRef};

    use super::*;
    use crate::control::StopFlag;

    /// Records what it is asked to send.
    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl EscalationSender for Recorder {
        fn send(&self, lead: &str, _team: &str, _body: &str) -> Result<(), SendError> {
            self.0.lock().unwrap().push(lead.to_owned());
            Ok(())
        }
    }

    type TestCache = Cache<FakeStore, FakeDrainLock, FakeBackend>;

    fn cache(clock: &Arc<ManualClock>) -> TestCache {
        Cache::new(
            CacheConfig::new(ProjectRef::local(None), vec![]),
            FakeStore::new(),
            FakeDrainLock::new(),
            FakeBackend::new(Arc::clone(clock)),
            Arc::clone(clock) as Arc<dyn Clock>,
        )
        .unwrap()
    }

    fn phase() -> Phase {
        Phase {
            name: PhaseName::new("x").unwrap(),
            root: sc_beads::testing::id("rx"),
            lead: Some(MemberName::new("lead-x").unwrap()),
        }
    }

    fn settings() -> TickSettings {
        TickSettings::from_config(&TickConfig {
            team: Some("t".into()),
            fallback_lead: Some("f".into()),
            ..TickConfig::default()
        })
        .unwrap()
    }

    fn sync_failed(cause: Option<SyncCause>) -> CacheError {
        CacheError::SyncFailed {
            detail: "phase x: scripted".into(),
            scope: Some(ScopeId::phase("x").unwrap()),
            cause,
            source: None,
        }
    }

    /// An unreachable backend is the outage's to announce, never a phase's:
    /// the sync reports it as an outage, but a `SYNC_FAILED` that names
    /// `BD_UNAVAILABLE` as its cause (the baseline's rule) is held back too.
    #[test]
    fn a_sync_failure_caused_by_an_unreachable_backend_is_not_announced_per_phase() {
        let clock = Arc::new(ManualClock::default());
        let stop = StopFlag::new();
        let budget = Budget {
            clock: &*clock,
            end: Deadline::after(&*clock, Duration::from_secs(25)),
            stop: &stop,
        };
        let sender = Recorder::default();
        let mut cache = cache(&clock);
        let mut reply = TickReply::started();
        sync_failure(
            &mut cache,
            &sender,
            &settings(),
            &budget,
            &phase(),
            &sync_failed(Some(SyncCause::BdUnavailable)),
            &mut reply,
        )
        .unwrap();
        assert!(sender.0.lock().unwrap().is_empty());
        assert!(reply.sent.is_empty() && reply.refused.is_empty());
        assert_eq!(cache.sync_alert(&phase().scope()).unwrap(), None);

        // Any other cause is announced, and keyed by its code.
        sync_failure(
            &mut cache,
            &sender,
            &settings(),
            &budget,
            &phase(),
            &sync_failed(Some(SyncCause::BdContract)),
            &mut reply,
        )
        .unwrap();
        assert_eq!(*sender.0.lock().unwrap(), ["lead-x"]);
        assert_eq!(
            cache.sync_alert(&phase().scope()).unwrap().as_deref(),
            Some("BD_CACHE.BD_CONTRACT")
        );
    }

    #[test]
    fn alert_causes_have_stable_keys() {
        assert_eq!(AlertCause::BackendUnavailable.key(), "backend_unavailable");
        assert_eq!(
            AlertCause::of(&sync_failed(None)).key(),
            "BD_CACHE.SYNC_FAILED"
        );
        assert_eq!(
            AlertCause::of(&sync_failed(Some(SyncCause::BdListFailed))).key(),
            "BD_CACHE.BD_LIST_FAILED"
        );
    }

    #[test]
    fn embedded_text_is_clipped_on_a_character_boundary() {
        assert_eq!(clip("short"), "short");
        let long = "\u{e9}".repeat(MAX_TEXT_BYTES);
        let clipped = clip(&long);
        assert!(clipped.ends_with("(truncated)"));
        assert!(clipped.len() < MAX_TEXT_BYTES + 32);
    }

    /// A sender that is asked after the stop flag is set is not asked at all.
    #[test]
    fn a_stopped_tick_sends_nothing_and_counts_the_message_unsent() {
        let clock = Arc::new(ManualClock::default());
        let stop = StopFlag::new();
        stop.stop();
        let budget = Budget {
            clock: &*clock,
            end: Deadline::after(&*clock, Duration::from_secs(25)),
            stop: &stop,
        };
        let sender = Recorder::default();
        let mut reply = TickReply::started();
        let lead = MemberName::new("l").unwrap();
        let out = deliver(&sender, &budget, &lead, &settings(), "hi", &mut reply);
        assert!(out.is_none());
        assert_eq!((reply.unsent, reply.stopped), (1, true));
        assert!(sender.0.lock().unwrap().is_empty());
    }
}
