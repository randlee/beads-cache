//! Announcements: sync failures, outages and escalations (REQ-BDC-0704..0706).
//!
//! Every message goes through the [`EscalationSender`] port. A message that is
//! not delivered leaves its marker unset (the entry stays unannounced, the
//! alert is not recorded), so the next tick sends it again, and the tick
//! replies `refused` with the list (REQ-BDC-0706).

use std::collections::BTreeMap;

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainLock, EscalationSender, JournalEntry, Outage, PhaseName, ScopeId,
    SyncCause,
};
use sc_beads::CacheBackend;

use crate::phases::{Phase, TickSettings};
use crate::reply::{Announcement, AnnouncementKind, TickReply};

/// The alert cause key of an outage (ADR-BDC-0005): each active scope's lead
/// hears it once, and the alert is reset when the outage ends.
pub const OUTAGE_CAUSE: &str = "backend_unavailable";

/// The stable cause of a failed sync: the underlying code, else `SYNC_FAILED`.
fn cause_of(e: &CacheError) -> Option<SyncCause> {
    match e {
        CacheError::SyncFailed { cause, .. } => *cause,
        other => SyncCause::of(other),
    }
}

/// The text recorded as the sync alert for a cause.
fn cause_key(e: &CacheError) -> String {
    cause_of(e).map_or_else(|| e.code().to_owned(), |c| c.as_str().to_owned())
}

/// The lead of a phase for alerts about the backend: its own, else the fallback.
fn alert_lead<'a>(phase: &'a Phase, settings: &'a TickSettings) -> &'a str {
    phase
        .lead
        .as_deref()
        .unwrap_or_else(|| settings.fallback_lead())
}

/// Announces a failed sync of `phase` once per cause (REQ-BDC-0704).
///
/// A cause that is unreachability is the outage's to announce, not the
/// phase's.
pub(crate) fn sync_failure<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    sender: &dyn EscalationSender,
    settings: &TickSettings,
    phase: &Phase,
    error: &CacheError,
    reply: &mut TickReply,
) -> Result<(), CacheError> {
    if cause_of(error) == Some(SyncCause::BdUnavailable) {
        return Ok(());
    }
    let cause = cause_key(error);
    let scope = phase.scope();
    if cache.sync_alert(&scope)?.as_deref() == Some(cause.as_str()) {
        return Ok(());
    }
    let lead = alert_lead(phase, settings);
    let body = format!(
        "bd-cache: the sync of phase {name} failed.\n\
         Error: {error}\n\
         Cause: {cause}\n\
         This cause is reported once; you will hear again only if the cause changes or the \
         sync recovers and fails again.",
        name = phase.name,
    );
    match sender.send(lead, settings.team(), &body) {
        Ok(()) => {
            cache.set_sync_alert(&scope, Some(&cause))?;
            reply.sent.push(Announcement {
                kind: AnnouncementKind::SyncFailure {
                    phase: phase.name.clone(),
                    cause,
                },
                lead: lead.to_owned(),
            });
        }
        Err(e) => reply
            .refused
            .push(format!("sync failure of phase {}: {e}", phase.name)),
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
        error = outage.last_error,
        phases = names.join(", "),
    )
}

/// Announces an outage to the leads of the active phases (REQ-BDC-0704,
/// ADR-BDC-0005), or resets the alerts when it has ended.
///
/// `targets` are the active phases; when none is active the caller passes
/// every configured phase, so a root that cannot be read during the outage
/// does not silence it. One message goes to each distinct lead, covering that
/// lead's phases whose alert does not yet read `backend_unavailable`.
pub(crate) fn outage<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    sender: &dyn EscalationSender,
    settings: &TickSettings,
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
    let mut by_lead: BTreeMap<&str, Vec<&Phase>> = BTreeMap::new();
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
        match sender.send(lead, settings.team(), &outage_body(&outage, &names)) {
            Ok(()) => {
                for phase in &phases {
                    cache.set_sync_alert(&phase.scope(), Some(OUTAGE_CAUSE))?;
                }
                reply.sent.push(Announcement {
                    kind: AnnouncementKind::Outage {
                        phases: names.into_iter().cloned().collect(),
                    },
                    lead: lead.to_owned(),
                });
            }
            Err(e) => reply.refused.push(format!("outage message to {lead}: {e}")),
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
        error = e.last_error.as_deref().unwrap_or("(no reason recorded)"),
    )
}

/// Sends every escalated entry that has not been announced to its phase's
/// lead, then marks it (REQ-BDC-0705). An entry with no scope goes to the
/// fallback lead; an entry whose phase has no lead is refused (REQ-BDC-0706).
pub(crate) fn escalations<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    sender: &dyn EscalationSender,
    settings: &TickSettings,
    phases: &[Phase],
    reply: &mut TickReply,
) -> Result<(), CacheError> {
    for entry in cache.unannounced_escalations()? {
        let lead = match &entry.scope {
            None => Some(settings.fallback_lead()),
            Some(scope) => phases
                .iter()
                .find(|p| p.name == *scope.phase_name())
                .and_then(|p| p.lead.as_deref()),
        };
        let Some(lead) = lead else {
            let phase = entry.scope.as_ref().map_or("-", ScopeId::name);
            reply.refused.push(format!(
                "escalation #{}: phase {phase} has no lead to tell",
                entry.seq
            ));
            continue;
        };
        match sender.send(lead, settings.team(), &escalation_body(&entry)) {
            Ok(()) => {
                cache.mark_announced(entry.seq)?;
                reply.sent.push(Announcement {
                    kind: AnnouncementKind::Escalation { seq: entry.seq },
                    lead: lead.to_owned(),
                });
            }
            Err(e) => reply
                .refused
                .push(format!("escalation #{}: {e}", entry.seq)),
        }
    }
    Ok(())
}
