//! The tick and the stop rule (REQ-BDC-0703, 0707).
//!
//! [`tick`] is a library entry point: `sc-cron` drives it through
//! [`SyncJob`](crate::SyncJob), and an external scheduler (the ATM cron
//! module, through the later `bdc tick`) can call it directly. Only one
//! ticker may run per cache file.

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainBudget, DrainLock, EscalationSender, HARD_MARGIN, TickConfig,
};
use sc_beads::{CacheBackend, Clock, Deadline};

use crate::active::active_phases;
use crate::announce;
use crate::phases::{Phase, TickSettings, load_phases};
use crate::reply::TickReply;

/// The budget of one tick (BD-CACHE.md, Cron module): the schedule's run
/// timeout, and so the distance from a run's start to its end `T`.
pub const TICK_BUDGET: std::time::Duration = std::time::Duration::from_secs(25);

/// Runs one tick that must be finished by `end` (`T`).
///
/// In order:
///
/// 1. the `[tick]` table is validated; a missing team or fallback lead, or a
///    malformed phase list, is refused before any work (REQ-BDC-0701, 0702);
/// 2. the journal is drained with `DrainBudget { start_gate: T - 5 s, hard:
///    T - 2 s }` (REQ-BDC-0703);
/// 3. the phase files are read and each phase's root decides whether it is
///    active (REQ-BDC-0702);
/// 4. each active phase is synced with deadline `T`; a phase not started by
///    `T - 2 s` is deferred (REQ-BDC-0703);
/// 5. a sync failure whose cause differs from the phase's alert is sent to
///    its lead, a success clears the alert (REQ-BDC-0704);
/// 6. an outage is announced through the per-scope alert
///    `backend_unavailable`, and the alerts are reset when it ends
///    (REQ-BDC-0704, ADR-BDC-0005);
/// 7. every escalated entry not yet announced is sent to its phase's lead
///    with the `resolve` command, then marked (REQ-BDC-0705).
///
/// The reply is `refused` when anything was refused or not delivered, and
/// the next tick retries it (REQ-BDC-0706). `clock` must be the clock the
/// cache and its backend share, so the deadlines and the budget margins are
/// measured on one timeline.
///
/// # Errors
///
/// A store error, or an error of the cache that is not a backend failure.
/// Backend failures are recorded and reported in the reply.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use bd_cache::Cache;
/// use bd_cache_model::testing::{FakeDrainLock, FakeStore};
/// use bd_cache_model::{CacheConfig, SendError, TickConfig};
/// use bd_cache_sync::{TICK_BUDGET, TickOutcome, tick};
/// use sc_beads::testing::{FakeBackend, ManualClock};
/// use sc_beads::{Clock, Deadline, ProjectRef};
///
/// let clock = Arc::new(ManualClock::default());
/// let mut cache = Cache::new(
///     CacheConfig::new(ProjectRef::local(None), vec![]),
///     FakeStore::new(),
///     FakeDrainLock::new(),
///     FakeBackend::new(Arc::clone(&clock)),
///     Arc::clone(&clock) as Arc<dyn Clock>,
/// )
/// .unwrap();
/// struct Mute;
/// impl bd_cache_model::EscalationSender for Mute {
///     fn send(&self, _: &str, _: &str, _: &str) -> Result<(), SendError> { Ok(()) }
/// }
/// // Without a team the tick is refused and does nothing.
/// let end = Deadline::after(&*clock, TICK_BUDGET);
/// let reply = tick(&mut cache, &TickConfig::default(), &Mute, &*clock, end).unwrap();
/// assert_eq!(reply.outcome, TickOutcome::Refused);
/// ```
pub fn tick<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    config: &TickConfig,
    sender: &dyn EscalationSender,
    clock: &dyn Clock,
    end: Deadline,
) -> Result<TickReply, CacheError> {
    let settings = match TickSettings::from_config(config) {
        Ok(settings) => settings,
        Err(refusal) => return Ok(TickReply::refused_with(refusal.to_string())),
    };
    let mut reply = TickReply::started();

    reply.drain = Some(cache.drain(&DrainBudget::before_tick_end(end))?);

    let phases = read_phases(&settings, &mut reply);
    let active = active_phases(cache, &phases, clock, end)?;
    reply.active = active.active.iter().map(|p| p.name.clone()).collect();
    reply.unreadable_roots = active.unreadable;

    let gate = Deadline::at(
        end.instant()
            .checked_sub(HARD_MARGIN)
            .unwrap_or_else(|| end.instant()),
    );
    for phase in &active.active {
        if gate.expired(clock) {
            reply.deferred.push(phase.name.clone());
            continue;
        }
        match cache.sync(&phase.scope(), end) {
            Ok(report) if report.deferred => reply.deferred.push(phase.name.clone()),
            Ok(report) => {
                if report.outage.is_none() {
                    announce::sync_succeeded(cache, &phase.scope())?;
                }
                reply.synced.push(report);
            }
            Err(error @ CacheError::SyncFailed { .. }) => {
                announce::sync_failure(cache, sender, &settings, phase, &error, &mut reply)?;
            }
            Err(other) => return Err(other),
        }
    }

    let targets: Vec<&Phase> = if active.active.is_empty() {
        phases.iter().collect()
    } else {
        active.active.iter().collect()
    };
    announce::outage(cache, sender, &settings, &targets, &phases, &mut reply)?;
    announce::escalations(cache, sender, &settings, &phases, &mut reply)?;
    Ok(reply.finish())
}

/// Reads the phase files; an unreadable directory or an unusable file is
/// refused in the reply and the phases found are used.
fn read_phases(settings: &TickSettings, reply: &mut TickReply) -> Vec<Phase> {
    match load_phases(settings.phases_dir(), settings.only()) {
        Ok(set) => {
            reply.refused.extend(set.problems);
            set.phases
        }
        Err(e) => {
            reply.refused.push(format!(
                "{e}: {}",
                std::error::Error::source(&e).map_or_else(String::new, ToString::to_string)
            ));
            Vec::new()
        }
    }
}

/// The stop rule (REQ-BDC-0707): `Some(reason)` when the schedule should end.
///
/// The schedule stops when no phase is active and no journal entry is pending
/// or escalated-and-unannounced. It stops only on positive knowledge: a
/// configuration that would be refused, an unreadable phases directory or
/// phase file, a root that cannot be read, and a store that cannot be asked
/// all answer `None` (keep going), so an outage or a typo cannot end the
/// schedule (ADR-SYNC-0004).
///
/// # Errors
///
/// A store error; the caller treats it as "keep going".
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use bd_cache::Cache;
/// use bd_cache_model::testing::{FakeDrainLock, FakeStore};
/// use bd_cache_model::{CacheConfig, TickConfig};
/// use bd_cache_sync::{TICK_BUDGET, stop_reason};
/// use sc_beads::testing::{FakeBackend, ManualClock};
/// use sc_beads::{Clock, Deadline, ProjectRef};
///
/// let clock = Arc::new(ManualClock::default());
/// let mut cache = Cache::new(
///     CacheConfig::new(ProjectRef::local(None), vec![]),
///     FakeStore::new(),
///     FakeDrainLock::new(),
///     FakeBackend::new(Arc::clone(&clock)),
///     Arc::clone(&clock) as Arc<dyn Clock>,
/// )
/// .unwrap();
/// // A configuration the tick would refuse never stops the schedule.
/// let end = Deadline::after(&*clock, TICK_BUDGET);
/// assert_eq!(stop_reason(&mut cache, &TickConfig::default(), &*clock, end).unwrap(), None);
/// ```
pub fn stop_reason<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    config: &TickConfig,
    clock: &dyn Clock,
    end: Deadline,
) -> Result<Option<String>, CacheError> {
    let Ok(settings) = TickSettings::from_config(config) else {
        return Ok(None);
    };
    let Ok(set) = load_phases(settings.phases_dir(), settings.only()) else {
        return Ok(None);
    };
    if !set.problems.is_empty() {
        return Ok(None);
    }
    let active = active_phases(cache, &set.phases, clock, end)?;
    if !active.active.is_empty() || !active.unreadable.is_empty() {
        return Ok(None);
    }
    let work = cache.open_work()?;
    if work.any() {
        return Ok(None);
    }
    Ok(Some(
        "no phase is active and no journal entry is pending or escalated and unannounced"
            .to_owned(),
    ))
}
