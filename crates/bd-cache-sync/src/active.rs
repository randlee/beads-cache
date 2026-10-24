//! Which phases are active (REQ-BDC-0702).
//!
//! A phase is active while its root bead is not closed. A root the cache
//! holds in the phase's scope, once that scope has synced, is read from the
//! cache (a sync keeps it current). Every other root is read with one
//! `bd list --id` of all of them under the miss-fill timeout
//! (`Cache::refresh`), so a root that is labelled outside its own phase is
//! still seen closing. A root that cannot be read, and a root bd does not
//! return, is unknown: its phase counts as inactive for this tick and is
//! reported (BD-CACHE.md, Cron module), and the stop rule keeps going on it
//! (ADR-SYNC-0004: stop only on positive knowledge).

use bd_cache::Cache;
use bd_cache_model::{CacheError, CacheStore, DrainLock, ScopeId};
use sc_beads::{BeadId, CacheBackend, Deadline};

use crate::control::StopFlag;
use crate::phases::Phase;

/// The result of looking at every phase's root.
#[derive(Debug, Default)]
pub(crate) struct ActiveSet {
    /// Phases with a root that is not closed, in input order.
    pub active: Vec<Phase>,
    /// Phases whose root is unknown: bd could not be read, or its answer did
    /// not return the root. Never known closed.
    pub unreadable: Vec<Phase>,
    /// The stop flag was set before the uncached roots were read: those roots
    /// are in neither list, and the caller must not act on the set as complete.
    pub stopped: bool,
}

/// True when the row's status is `closed`.
fn is_closed(status: Option<&str>) -> bool {
    status == Some(bd_cache::CLOSED_STATUS)
}

/// What the cache knows of a phase's root.
enum Cached {
    /// The root is in the phase's scope, which has synced, with this closed-ness.
    Known { closed: bool },
    /// Not held, not in the phase, or the phase's scope never synced.
    Unknown,
}

/// The scopes that have synced at least once, read on first use.
///
/// `Cache` has no accessor for one scope's `sync_state`, so the states come
/// from one [`Cache::status`] (store reads only, no `bd` call), taken only
/// when some root is held in its phase's scope.
#[derive(Default)]
struct SyncedScopes(Option<Vec<ScopeId>>);

impl SyncedScopes {
    /// True when `scope` has a `last_ok_at`.
    fn has_synced<S: CacheStore, L: DrainLock, B: CacheBackend>(
        &mut self,
        cache: &Cache<S, L, B>,
        scope: &ScopeId,
    ) -> Result<bool, CacheError> {
        if self.0.is_none() {
            let synced = cache
                .status()?
                .store
                .scopes
                .into_iter()
                .filter(|s| s.sync.last_ok_at.is_some())
                .map(|s| s.sync.scope)
                .collect();
            self.0 = Some(synced);
        }
        Ok(self.0.as_ref().is_some_and(|s| s.contains(scope)))
    }
}

/// One cached read of the root (`Cache::cached`: no bd call, no scan of the
/// scope): trusted only when the phase's own label holds it and the phase's
/// own scope has synced, since a sync of that scope is what keeps it current.
/// A root that carries several phase labels is reported by `Cache::cached`
/// under the first known one; that scope's sync says nothing about this phase.
fn cached_root<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    phase: &Phase,
    synced: &mut SyncedScopes,
) -> Result<Cached, CacheError> {
    let Some(reply) = cache.cached(&phase.root)? else {
        return Ok(Cached::Unknown);
    };
    let scope = phase.scope();
    if scope.holds(&reply.row.bead) && synced.has_synced(cache, &scope)? {
        return Ok(Cached::Known {
            closed: is_closed(reply.row.bead.status.as_deref()),
        });
    }
    Ok(Cached::Unknown)
}

/// Finds the active phases among `phases`.
///
/// `stop` is checked before the one `bd list --id` of the uncached roots: a
/// stop that is already set does not start it, and the set says
/// [`stopped`](ActiveSet::stopped).
///
/// # Errors
///
/// A store error; a backend failure only makes the roots it concerns
/// unreadable.
pub(crate) fn active_phases<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    phases: &[Phase],
    end: Deadline,
    stop: &StopFlag,
) -> Result<ActiveSet, CacheError> {
    let clock = cache.clock();
    let mut set = ActiveSet::default();
    let mut synced = SyncedScopes::default();
    // Phases whose root the cache cannot answer for.
    let mut uncached: Vec<&Phase> = Vec::new();
    let mut open: Vec<&Phase> = Vec::new();
    for phase in phases {
        match cached_root(cache, phase, &mut synced)? {
            Cached::Known { closed: true } => {}
            Cached::Known { closed: false } => open.push(phase),
            Cached::Unknown => uncached.push(phase),
        }
    }
    let mut unreadable: Vec<&Phase> = Vec::new();
    if !uncached.is_empty() && stop.is_stopped() {
        set.stopped = true;
    } else if !uncached.is_empty() {
        let ids: Vec<BeadId> = uncached.iter().map(|p| p.root.clone()).collect();
        let timeout = cache.config().miss_fill_timeout.min(end.remaining(&*clock));
        match cache.refresh(&ids, timeout) {
            Ok(rows) => {
                for phase in &uncached {
                    match rows.iter().find(|r| r.bead.id == phase.root) {
                        Some(r) if is_closed(r.bead.status.as_deref()) => {}
                        Some(_) => open.push(phase),
                        // Not returned is not closed: unknown (S23, ADR-SYNC-0004).
                        None => {
                            tracing::warn!(
                                phase = phase.name.as_str(),
                                root = phase.root.as_str(),
                                "bd does not return the phase's root; the phase counts as \
                                 inactive this tick and keeps the schedule going"
                            );
                            unreadable.push(phase);
                        }
                    }
                }
            }
            Err(e @ CacheError::StoreFailed { .. }) => return Err(e),
            Err(e) => {
                tracing::warn!(
                    code = e.code(),
                    detail = e.detail(),
                    "phase roots could not be read; those phases count as inactive this tick"
                );
                unreadable = uncached;
            }
        }
    }
    // Keep the input order.
    for phase in phases {
        if open.iter().any(|o| o.name == phase.name) {
            set.active.push(phase.clone());
        } else if unreadable.iter().any(|o| o.name == phase.name) {
            set.unreadable.push(phase.clone());
        }
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use bd_cache_model::CacheConfig;
    use bd_cache_model::testing::{FakeDrainLock, FakeStore};
    use sc_beads::testing::{FakeBackend, FakeOp, ManualClock, bead, id};
    use sc_beads::{Clock, ProjectRef};

    use super::*;
    use crate::phases::MemberName;
    use bd_cache_model::PhaseName;

    /// A stop that is already set does not start the `bd list --id` of the
    /// uncached roots (QA4-006); without it the root is read.
    #[test]
    fn a_set_stop_flag_skips_the_root_refresh() {
        let clock = Arc::new(ManualClock::default());
        let bd = FakeBackend::new(Arc::clone(&clock));
        bd.insert(bead("rx"));
        let mut cache = Cache::new(
            CacheConfig::new(ProjectRef::local(None), vec![]),
            FakeStore::new(),
            FakeDrainLock::new(),
            bd.clone(),
            Arc::clone(&clock) as Arc<dyn Clock>,
        )
        .unwrap();
        let phase = Phase {
            name: PhaseName::new("x").unwrap(),
            root: id("rx"),
            lead: Some(MemberName::new("lead-x").unwrap()),
        };
        let end = Deadline::after(&*clock, Duration::from_secs(25));
        let refreshes = || {
            bd.calls()
                .iter()
                .filter(|c| c.op() == FakeOp::ListByIds)
                .count()
        };

        let stop = StopFlag::new();
        stop.stop();
        let set = active_phases(&mut cache, std::slice::from_ref(&phase), end, &stop).unwrap();
        assert!(set.stopped, "{set:?}");
        assert!(
            set.active.is_empty() && set.unreadable.is_empty(),
            "{set:?}"
        );
        assert_eq!(refreshes(), 0, "no bd call after the stop");

        let set = active_phases(
            &mut cache,
            std::slice::from_ref(&phase),
            end,
            &StopFlag::new(),
        )
        .unwrap();
        assert!(!set.stopped);
        assert_eq!(set.active, [phase]);
        assert_eq!(refreshes(), 1);
    }
}
