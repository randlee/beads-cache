//! Announcements: sync failures, outages and escalations (REQ-BDC-0704..0706).
//!
//! Every message goes through the [`EscalationSender`] port, inside the
//! tick's budget: before each send the tick checks its stop flag and the time
//! left before `T`, passes the sender the time left, and defers the message
//! when there is too little (it stays unmarked and the next tick sends it).
//! A message that is not delivered leaves its marker unset (the entry stays
//! unannounced, the alert is not recorded), so the next tick sends it again,
//! and the tick replies `refused` with the list (REQ-BDC-0706).

use std::collections::{BTreeMap, BTreeSet};

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainLock, EscalationSender, JournalEntry, Outage, PhaseName, ScopeId,
    SendError, Seq, SyncCause,
};
use sc_beads::CacheBackend;

use crate::control::Budget;
use crate::phases::{MemberName, Phase, TickSettings};
use crate::reply::{Announcement, AnnouncementKind, Refusal, RefusalCode, TickReply};
use crate::text::clip;

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

/// The sync cause after `prev` in declaration order (`None` starts the list),
/// `None` after the last.
///
/// The inverse of `SyncCause::as_str` is kept here, in this crate, because the
/// model's `error.rs` belongs to another sprint (lead ruling S29). What guards
/// it, exactly:
///
/// - `SyncCause` is not `#[non_exhaustive]`, so this match is exhaustive: a new
///   variant does not compile until it has an arm here, and each arm names the
///   variant that follows it.
/// - The const assertion below walks the chain at compile time and checks that
///   it lists the variants in declaration order (discriminants 0, 1, 2, ...),
///   with no gap and no repeat; the round-trip test checks every listed cause
///   maps back from its key. No count is kept by hand.
///
/// What no check can catch: a variant appended after `BdListFailed` whose own
/// arm answers `None` while `BdListFailed`'s arm is left answering `None` too.
/// It compiles and is not listed. Adding a variant means changing the arm of
/// the one before it.
const fn next_sync_cause(prev: Option<SyncCause>) -> Option<SyncCause> {
    match prev {
        None => Some(SyncCause::BdUnavailable),
        Some(SyncCause::BdUnavailable) => Some(SyncCause::BdVersionFailed),
        Some(SyncCause::BdVersionFailed) => Some(SyncCause::BdSchemaFailed),
        Some(SyncCause::BdSchemaFailed) => Some(SyncCause::BdContract),
        Some(SyncCause::BdContract) => Some(SyncCause::BdListFailed),
        Some(SyncCause::BdListFailed) => None,
    }
}

// The chain lists the variants in declaration order, without gaps or repeats.
const _: () = {
    let mut prev = None;
    let mut index = 0;
    while let Some(cause) = next_sync_cause(prev) {
        assert!(
            cause as usize == index,
            "next_sync_cause must follow the declaration order of SyncCause"
        );
        index += 1;
        prev = Some(cause);
    }
    assert!(index > 0);
};

/// Every underlying code a sync failure can name, in declaration order.
fn sync_causes() -> impl Iterator<Item = SyncCause> {
    std::iter::successors(next_sync_cause(None), |c| next_sync_cause(Some(*c)))
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

    /// The cause a stored key names; `None` for a key this crate never writes.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        if key == OUTAGE_CAUSE {
            return Some(Self::BackendUnavailable);
        }
        if key == SYNC_FAILED_KEY {
            return Some(Self::SyncFailed);
        }
        sync_causes().find(|c| c.as_str() == key).map(Self::Sync)
    }

    /// The cause a stored alert value names, in either writer's format: this
    /// crate stores the bare key; `bd_cache.py` stores `"<code>: <detail>"`
    /// (atm-core `05831df`, `bd_cache_cron.py`: `f"{error.code}: {error.detail}"`),
    /// so the key is what precedes the first `": "`. `None` for text that does not
    /// start with a known key.
    #[must_use]
    pub fn from_stored(value: &str) -> Option<Self> {
        let key = value.split_once(": ").map_or(value, |(key, _)| key);
        Self::from_key(key.trim())
    }

    /// Whether an alert that stored `self` already announces `new`, so `new`
    /// need not be sent again: the same cause, or `self` is the generic
    /// `BD_CACHE.SYNC_FAILED` that `bd_cache.py` records for every sync failure
    /// (it keeps the underlying code in a separate field it does not store in the alert).
    #[must_use]
    pub fn covers(self, new: Self) -> bool {
        self == new || (self == Self::SyncFailed && matches!(new, Self::Sync(_)))
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

impl serde::Serialize for AlertCause {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.key())
    }
}

/// Whether a stored alert is the outage's.
fn is_outage_alert(stored: Option<&str>) -> bool {
    stored.and_then(AlertCause::from_stored) == Some(AlertCause::BackendUnavailable)
}

/// The lead of a phase for alerts about the backend: its own, else the fallback.
fn alert_lead<'a>(phase: &'a Phase, settings: &'a TickSettings) -> &'a MemberName {
    phase
        .lead
        .as_ref()
        .unwrap_or_else(|| settings.fallback_lead())
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
    if cache
        .sync_alert(&scope)?
        .as_deref()
        .and_then(AlertCause::from_stored)
        .is_some_and(|stored| stored.covers(cause))
    {
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
        error = clip(&error.to_string(), MAX_TEXT_BYTES),
        key = cause.key(),
    );
    match deliver(sender, budget, lead, settings, &body, reply) {
        None => {}
        Some(Ok(())) => {
            cache
                .set_sync_alert_within(&scope, Some(cause.key()), budget.write_wait())
                .inspect_err(|e| {
                    unmarked_after_send(Unmarked::SyncFailure(&scope), lead, e);
                })?;
            reply.sent.push(Announcement {
                kind: AnnouncementKind::SyncFailure {
                    phase: phase.name.clone(),
                    cause,
                },
                lead: lead.clone(),
            });
        }
        Some(Err(e)) => reply.refused.push(Refusal::undelivered(
            RefusalCode::SyncFailureUndelivered,
            &format!("sync failure of phase {}", phase.name),
            &e,
        )),
    }
    Ok(())
}

/// The announcement a marker write after a send failed to mark, with the
/// entry or scope it marks (RBP-F045): the kind and the marked item cannot
/// disagree.
#[derive(Debug, Clone, Copy)]
enum Unmarked<'a> {
    /// A sync failure of this scope (its `set_sync_alert`).
    SyncFailure(&'a ScopeId),
    /// An outage message covering this scope (its `set_sync_alert`).
    Outage(&'a ScopeId),
    /// The escalation of this journal entry (its `mark_announced`).
    Escalation(Seq),
}

impl Unmarked<'_> {
    /// The stable name of the announcement, the `kind` field of the warning.
    const fn kind(self) -> &'static str {
        match self {
            Self::SyncFailure(_) => "sync_failure",
            Self::Outage(_) => "outage",
            Self::Escalation(_) => "escalation",
        }
    }
}

/// Logs a marker write that failed after its message was delivered (lead
/// ruling S35, RSH-031): which announcement, to whom, for which entry or
/// scope, and the error's code. The caller then fails the tick, so the marker
/// stays unset and the next tick sends the message again (at-least-once).
fn unmarked_after_send(what: Unmarked<'_>, lead: &MemberName, e: &CacheError) {
    let (seq, scope) = match what {
        Unmarked::Escalation(seq) => (Some(seq.get()), None),
        Unmarked::SyncFailure(scope) | Unmarked::Outage(scope) => (None, Some(scope.name())),
    };
    tracing::warn!(
        kind = what.kind(),
        lead = lead.as_str(),
        seq,
        scope,
        code = e.code(),
        error = %e,
        "a delivered message could not be marked; the next tick sends it again"
    );
}

/// Why an alert is cleared without a message (RBP-F045).
#[derive(Debug, Clone, Copy)]
enum ClearReason {
    /// The phase's sync succeeded.
    SyncSucceeded,
    /// The outage that set the alert is over.
    OutageOver,
}

impl ClearReason {
    /// The stable name of the reason, the `why` field of the warning.
    const fn as_str(self) -> &'static str {
        match self {
            Self::SyncSucceeded => "sync_succeeded",
            Self::OutageOver => "outage_over",
        }
    }
}

/// Clears an alert without a message, waiting for the write lock at most
/// [`Budget::write_wait`] (lead ruling S35, RSH-030). A clear that fails is
/// logged and does not fail the tick: the alert stays set, which at worst
/// suppresses one repeat of the same cause, and the next tick clears it.
fn clear_alert<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    budget: &Budget<'_>,
    scope: &ScopeId,
    why: ClearReason,
) {
    if let Err(e) = cache.set_sync_alert_within(scope, None, budget.write_wait()) {
        tracing::warn!(
            scope = scope.name(),
            why = why.as_str(),
            code = e.code(),
            error = %e,
            "cannot clear the sync alert; the next tick tries again"
        );
    }
}

/// Clears the alert of a phase whose sync succeeded, within the tick's
/// marker-write bound ([`clear_alert`]).
pub(crate) fn sync_succeeded<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    budget: &Budget<'_>,
    scope: &ScopeId,
) -> Result<(), CacheError> {
    if cache.sync_alert(scope)?.is_some() {
        clear_alert(cache, budget, scope, ClearReason::SyncSucceeded);
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
        error = clip(&outage.last_error, MAX_TEXT_BYTES),
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
///
/// When the outage is over, the alerts are cleared one scope at a time until
/// `T` has passed ([`Budget::write_wait`] is zero) or the tick is asked to
/// stop, which also sets [`TickReply::stopped`]; the rest stay set for the
/// next tick (lead ruling S45).
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
        // Over: reset every alert this outage set, without a message, each
        // within the tick's marker-write bound ([`clear_alert`]). Each scope
        // once: a phase is both a stored scope and a phase file, and a clear
        // that failed is the next tick's to retry, not this one's (QA8-001).
        // No clear starts once `T` has passed or the tick was asked to stop;
        // the alerts left set are the next tick's to clear (lead ruling S45,
        // RSH-037).
        let mut scopes: BTreeSet<ScopeId> =
            status.scopes.iter().map(|s| s.sync.scope.clone()).collect();
        scopes.extend(all.iter().map(Phase::scope));
        for scope in scopes {
            if budget.stop.is_stopped() {
                reply.stopped = true;
                break;
            }
            if budget.write_wait().is_zero() {
                break;
            }
            if is_outage_alert(cache.sync_alert(&scope)?.as_deref()) {
                clear_alert(cache, budget, &scope, ClearReason::OutageOver);
            }
        }
        return Ok(());
    };
    let mut by_lead: BTreeMap<&MemberName, Vec<&Phase>> = BTreeMap::new();
    for phase in targets {
        // Either writer's form: the bare key, or `bd_cache.py`'s `"<code>: <detail>"`.
        if !is_outage_alert(cache.sync_alert(&phase.scope())?.as_deref()) {
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
                    let scope = phase.scope();
                    cache
                        .set_sync_alert_within(
                            &scope,
                            Some(AlertCause::BackendUnavailable.key()),
                            budget.write_wait(),
                        )
                        .inspect_err(|e| {
                            unmarked_after_send(Unmarked::Outage(&scope), lead, e);
                        })?;
                }
                reply.sent.push(Announcement {
                    kind: AnnouncementKind::Outage {
                        phases: names.into_iter().cloned().collect(),
                    },
                    lead: lead.clone(),
                });
            }
            Some(Err(e)) => reply.refused.push(Refusal::undelivered(
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
    format!(
        "bd-cache: write #{seq} to {bead} ({op}, by {actor}) was not applied and needs you.\n\
         Error: {error}\n\
         Check the bead in bd and set the intended state, then run:\n  \
         bd-cache resolve {seq} --note \"<what you did>\" --actor <you>",
        seq = e.seq,
        bead = e.bead,
        op = e.op.name(),
        actor = e.actor,
        error = clip(
            e.last_error.as_deref().unwrap_or("(no reason recorded)"),
            MAX_TEXT_BYTES
        ),
    )
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
    let entries = cache.unannounced_escalations()?;
    let total = entries.len();
    for (done, entry) in entries.into_iter().enumerate() {
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
            // The budget is spent (or the tick was asked to stop): this entry
            // and every one after it wait for the next tick, unmarked.
            None => {
                let rest = total.saturating_sub(done + 1);
                reply.unsent = reply
                    .unsent
                    .saturating_add(u32::try_from(rest).unwrap_or(u32::MAX));
                break;
            }
            Some(Ok(())) => {
                cache
                    .mark_announced_within(entry.seq, budget.write_wait())
                    .inspect_err(|e| {
                        unmarked_after_send(Unmarked::Escalation(entry.seq), lead, e);
                    })?;
                reply.sent.push(Announcement {
                    kind: AnnouncementKind::Escalation { seq: entry.seq },
                    lead: lead.clone(),
                });
            }
            Some(Err(e)) => reply.refused.push(Refusal::undelivered(
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

    /// When the outage is over, a stopped tick clears no alert and says it
    /// stopped; the next tick, not stopped, clears it (lead ruling S45,
    /// RSH-037). `T` passed is the integration test's
    /// (`tests/marker_wait.rs`).
    #[test]
    fn a_stopped_tick_clears_no_outage_alert_and_the_next_tick_does() {
        let clock = Arc::new(ManualClock::default());
        let mut cache = cache(&clock);
        let scope = phase().scope();
        cache
            .set_sync_alert(&scope, Some(AlertCause::BackendUnavailable.key()))
            .unwrap();
        let sender = Recorder::default();
        let run = |cache: &mut TestCache, stop: &StopFlag| {
            let budget = Budget {
                clock: &*clock,
                end: Deadline::after(&*clock, Duration::from_secs(25)),
                stop,
            };
            let mut reply = TickReply::started();
            outage(
                cache,
                &sender,
                &settings(),
                &budget,
                &[],
                &[phase()],
                &mut reply,
            )
            .unwrap();
            reply
        };

        let stopped = StopFlag::new();
        stopped.stop();
        let reply = run(&mut cache, &stopped);
        assert!(reply.stopped, "{reply:?}");
        assert_eq!(
            cache.sync_alert(&scope).unwrap().as_deref(),
            Some(OUTAGE_CAUSE),
            "not cleared by a stopped tick"
        );

        let reply = run(&mut cache, &StopFlag::new());
        assert!(!reply.stopped, "{reply:?}");
        assert_eq!(cache.sync_alert(&scope).unwrap(), None, "cleared");
        assert!(sender.0.lock().unwrap().is_empty(), "recovery is silent");
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

    /// An alert `bd_cache.py` stored (`"<code>: <detail>"`) is understood, and a
    /// failure it already announced is not announced again (S21).
    #[test]
    fn an_alert_in_bd_cache_pys_format_is_read_and_covers_the_failure_it_announced() {
        assert_eq!(
            AlertCause::from_stored("BD_CACHE.SYNC_FAILED: phase x: BD_CACHE.BD_LIST_FAILED: boom"),
            Some(AlertCause::SyncFailed)
        );
        assert_eq!(
            AlertCause::from_stored("BD_CACHE.BD_CONTRACT: field status"),
            Some(AlertCause::Sync(SyncCause::BdContract))
        );
        assert_eq!(
            AlertCause::from_stored("backend_unavailable"),
            Some(AlertCause::BackendUnavailable)
        );
        assert_eq!(AlertCause::from_stored("unknown: x"), None);
        let generic = AlertCause::SyncFailed;
        assert!(generic.covers(AlertCause::Sync(SyncCause::BdListFailed)));
        assert!(generic.covers(AlertCause::SyncFailed));
        assert!(
            !AlertCause::Sync(SyncCause::BdContract)
                .covers(AlertCause::Sync(SyncCause::BdListFailed))
        );
        assert!(!AlertCause::BackendUnavailable.covers(AlertCause::Sync(SyncCause::BdListFailed)));

        // End to end: the store holds Python's alert; the failure is not re-sent.
        let clock = Arc::new(ManualClock::default());
        let stop = StopFlag::new();
        let budget = Budget {
            clock: &*clock,
            end: Deadline::after(&*clock, Duration::from_secs(25)),
            stop: &stop,
        };
        let sender = Recorder::default();
        let mut cache = cache(&clock);
        cache
            .set_sync_alert(
                &phase().scope(),
                Some("BD_CACHE.SYNC_FAILED: phase x: BD_CACHE.BD_CONTRACT: a field"),
            )
            .unwrap();
        let mut reply = TickReply::started();
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
        assert!(
            sender.0.lock().unwrap().is_empty(),
            "Python already told the lead"
        );
        assert!(reply.sent.is_empty());
    }

    /// Every listed `SyncCause` round-trips through its key, and the list is
    /// in declaration order with distinct keys (S29). The completeness the
    /// match gives, and its limit, are stated on `next_sync_cause`.
    #[test]
    fn every_sync_cause_round_trips_through_its_key() {
        let listed: Vec<SyncCause> = sync_causes().collect();
        assert!(!listed.is_empty());
        for (i, cause) in listed.iter().copied().enumerate() {
            assert_eq!(cause as usize, i, "declaration order: {cause:?}");
            assert_eq!(
                AlertCause::from_key(cause.as_str()),
                Some(AlertCause::Sync(cause))
            );
            assert_eq!(
                AlertCause::from_stored(&format!("{}: detail", cause.as_str())),
                Some(AlertCause::Sync(cause))
            );
        }
        let keys: std::collections::BTreeSet<&str> = listed.iter().map(|c| c.as_str()).collect();
        assert_eq!(keys.len(), listed.len(), "distinct keys: {keys:?}");
    }

    #[test]
    fn alert_causes_round_trip_through_their_keys() {
        for cause in [
            AlertCause::BackendUnavailable,
            AlertCause::SyncFailed,
            AlertCause::Sync(SyncCause::BdContract),
            AlertCause::Sync(SyncCause::BdListFailed),
        ] {
            assert_eq!(AlertCause::from_key(cause.key()), Some(cause));
        }
        assert_eq!(AlertCause::from_key("something else"), None);
    }

    #[test]
    fn embedded_error_text_is_clipped() {
        let long = "x".repeat(MAX_TEXT_BYTES * 2);
        let body = outage_body(
            &Outage {
                since: sc_beads::Timestamp::parse("2026-10-23T10:00:00Z").unwrap(),
                last_error: long,
            },
            &[],
        );
        assert!(body.contains("(truncated)") && body.len() < MAX_TEXT_BYTES + 512);
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
