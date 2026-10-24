//! The tick and the stop rule (REQ-BDC-0703, 0707).
//!
//! [`tick`] is a library entry point: `sc-cron` drives it through
//! [`SyncJob`](crate::SyncJob), and an external scheduler (the ATM cron
//! module, through the later `bdc tick`) can call it. Only one ticker may run
//! per cache file. Time is read from the cache's own clock
//! ([`Cache::clock`]), so the deadlines and the budget margins are on one
//! timeline with the backend's calls.

use bd_cache::Cache;
use bd_cache_model::{
    CacheError, CacheStore, DrainBudget, DrainLock, EscalationSender, HARD_MARGIN, TickConfig,
};
use sc_beads::{CacheBackend, Deadline};

use crate::active::active_phases;
use crate::announce;
use crate::control::{Budget, StopFlag};
use crate::phases::{Phase, TickSettings, load_phases};
use crate::reply::{Refusal, TickReply};

/// The budget of one tick (BD-CACHE.md, Cron module): the schedule's run
/// timeout, and so the distance from a run's start to its end `T`.
pub const TICK_BUDGET: std::time::Duration = std::time::Duration::from_secs(25);

/// Runs one tick that must be finished by `end` (`T`).
///
/// In order:
///
/// 1. the `[tick]` table is validated; a missing team or fallback lead, or a
///    malformed or empty phase list, is refused before any work
///    (REQ-BDC-0701, 0702);
/// 2. the journal is drained with `DrainBudget { start_gate: T - 5 s, hard:
///    T - 2 s }` (REQ-BDC-0703);
/// 3. the phase files are read and each phase's root decides whether it is
///    active (REQ-BDC-0702);
/// 4. each active phase is synced with deadline `T`; a phase not started by
///    `T - 2 s` is deferred (REQ-BDC-0703);
/// 5. a sync failure whose cause differs from the phase's alert is sent to
///    its lead, a success clears the alert (REQ-BDC-0704);
/// 6. an outage is announced through the per-scope alert
///    `backend_unavailable` to the leads of the active phases and of the
///    phases whose root could not be read, and the alerts are reset when it
///    ends (REQ-BDC-0704, ADR-BDC-0005);
/// 7. every escalated entry not yet announced is sent to its phase's lead
///    with the `resolve` command, then marked (REQ-BDC-0705).
///
/// Steps 5 to 7 send within the time left before `T`: a message with less
/// than half a second left is not started and is counted in
/// [`TickReply::unsent`]; it stays unmarked and the next tick sends it. The
/// reply is `refused` when anything was refused or not delivered, and the next
/// tick retries it (REQ-BDC-0706).
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
/// let reply = tick(&mut cache, &TickConfig::default(), &Mute, end).unwrap();
/// assert_eq!(reply.outcome, TickOutcome::Refused);
/// ```
pub fn tick<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    config: &TickConfig,
    sender: &dyn EscalationSender,
    end: Deadline,
) -> Result<TickReply, CacheError> {
    match TickSettings::from_config(config) {
        Ok(settings) => tick_with(cache, &settings, sender, end, &StopFlag::new()),
        Err(refusal) => Ok(TickReply::refused_with(refusal.into())),
    }
}

/// [`tick`] over settings that are already validated, and with a stop flag.
///
/// The flag is checked after the drain, before the one `bd list --id` of the
/// uncached phase roots, between phases and before every message; a tick that
/// finds it set ends early with [`TickReply::stopped`] and leaves what it
/// did not send unmarked.
///
/// # Errors
///
/// As [`tick`].
pub fn tick_with<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    settings: &TickSettings,
    sender: &dyn EscalationSender,
    end: Deadline,
    stop: &StopFlag,
) -> Result<TickReply, CacheError> {
    let clock = cache.clock();
    let budget = Budget {
        clock: &*clock,
        end,
        stop,
    };
    let mut reply = TickReply::started();
    if stop.is_stopped() {
        reply.stopped = true;
        return Ok(reply.finish());
    }

    reply.drain = Some(cache.drain(&DrainBudget::before_tick_end(end))?);
    if stop.is_stopped() {
        // The drain is the long part; do not start reading phases after it.
        reply.stopped = true;
        return Ok(reply.finish());
    }

    let phases = read_phases(settings, &mut reply);
    let active = active_phases(cache, &phases, end, stop)?;
    reply.active = active.active.iter().map(|p| p.name.clone()).collect();
    reply.unreadable_roots = active.unreadable.iter().map(|p| p.name.clone()).collect();
    if active.stopped {
        // Stopped before the uncached roots were read: no sync, no message.
        reply.stopped = true;
        return Ok(reply.finish());
    }

    let gate = Deadline::at(
        end.instant()
            .checked_sub(HARD_MARGIN)
            .unwrap_or_else(|| end.instant()),
    );
    for phase in &active.active {
        if stop.is_stopped() {
            reply.stopped = true;
            break;
        }
        if gate.expired(&*clock) {
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
                announce::sync_failure(
                    cache, sender, settings, &budget, phase, &error, &mut reply,
                )?;
            }
            Err(other) => return Err(other),
        }
    }

    // The leads of the phases that are active or whose root could not be
    // read; never a phase known to be closed (ADR-SYNC-0003).
    let targets: Vec<&Phase> = active
        .active
        .iter()
        .chain(active.unreadable.iter())
        .collect();
    announce::outage(
        cache, sender, settings, &budget, &targets, &phases, &mut reply,
    )?;
    announce::escalations(cache, sender, settings, &budget, &phases, &mut reply)?;
    Ok(reply.finish())
}

/// Reads the phase files; an unreadable directory or an unusable file is
/// refused in the reply and the phases found are used.
fn read_phases(settings: &TickSettings, reply: &mut TickReply) -> Vec<Phase> {
    match load_phases(settings.phases_dir(), settings.only()) {
        Ok(set) => {
            reply
                .refused
                .extend(set.problems.into_iter().map(Refusal::from));
            set.phases
        }
        Err(e) => {
            reply.refused.push(e.refusal());
            Vec::new()
        }
    }
}

/// The stop rule (REQ-BDC-0707): `Some(reason)` when the schedule should end.
///
/// The schedule stops when no phase is active and no journal entry is pending
/// or escalated-and-unannounced. It stops only on positive knowledge: a
/// configuration that would be refused, an unreadable phases directory or
/// phase file, a root that cannot be read or that bd does not return, and a
/// store that cannot be asked all answer `None` (keep going), so an outage or a typo cannot end the
/// schedule (ADR-SYNC-0004). `end` bounds the one `bd list --id` of uncached
/// roots.
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
/// assert_eq!(stop_reason(&mut cache, &TickConfig::default(), end).unwrap(), None);
/// ```
pub fn stop_reason<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    config: &TickConfig,
    end: Deadline,
) -> Result<Option<String>, CacheError> {
    let Ok(settings) = TickSettings::from_config(config) else {
        return Ok(None);
    };
    stop_reason_with(cache, &settings, end)
}

/// [`stop_reason`] over settings that are already validated.
///
/// # Errors
///
/// As [`stop_reason`].
pub fn stop_reason_with<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    settings: &TickSettings,
    end: Deadline,
) -> Result<Option<String>, CacheError> {
    let Ok(set) = load_phases(settings.phases_dir(), settings.only()) else {
        return Ok(None);
    };
    if !set.problems.is_empty() {
        return Ok(None);
    }
    // No stop flag here: the stop check has its own small budget (`end`).
    let active = active_phases(cache, &set.phases, end, &StopFlag::new())?;
    if active.stopped || !active.active.is_empty() || !active.unreadable.is_empty() {
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
