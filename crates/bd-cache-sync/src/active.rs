//! Which phases are active (REQ-BDC-0702).
//!
//! A phase is active while its root bead is not closed. A root the cache
//! holds in the phase's scope is read from the cache (a sync keeps it
//! current). Every other root is read with one `bd list --id` of all of them
//! under the miss-fill timeout (`Cache::refresh`), so a root that is
//! labelled outside its own phase is still seen closing. A root that cannot
//! be read counts as inactive for this tick and is reported
//! (BD-CACHE.md, Cron module).

use bd_cache::Cache;
use bd_cache_model::{CacheError, CacheStore, DrainLock, PhaseName};
use sc_beads::{BeadId, CacheBackend, Clock, Deadline};

use crate::phases::Phase;

/// The result of looking at every phase's root.
#[derive(Debug, Default)]
pub(crate) struct ActiveSet {
    /// Phases with a root that is not closed, in input order.
    pub active: Vec<Phase>,
    /// Phases whose root could not be read.
    pub unreadable: Vec<PhaseName>,
}

/// True when the row's status is `closed`.
fn is_closed(status: Option<&str>) -> bool {
    status == Some("closed")
}

/// What the cache knows of a phase's root.
enum Cached {
    /// The root is in the scope with this closed-ness.
    Known { closed: bool },
    /// Not in the scope's rows (never synced, or not labelled with the phase).
    Unknown,
}

fn cached_root<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    phase: &Phase,
) -> Result<Cached, CacheError> {
    match cache.list(&phase.scope(), None, None) {
        Ok(reply) => {
            Ok(reply
                .rows
                .iter()
                .find(|r| r.bead.id == phase.root)
                .map_or(Cached::Unknown, |r| Cached::Known {
                    closed: is_closed(r.bead.status.as_deref()),
                }))
        }
        Err(CacheError::ScopeUnknown { .. }) => Ok(Cached::Unknown),
        Err(e) => Err(e),
    }
}

/// Finds the active phases among `phases`.
///
/// # Errors
///
/// A store error; a backend failure only makes the roots it concerns
/// unreadable.
pub(crate) fn active_phases<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    phases: &[Phase],
    clock: &dyn Clock,
    end: Deadline,
) -> Result<ActiveSet, CacheError> {
    let mut set = ActiveSet::default();
    // Phases whose root the cache cannot answer for.
    let mut uncached: Vec<&Phase> = Vec::new();
    let mut open: Vec<&Phase> = Vec::new();
    for phase in phases {
        match cached_root(cache, phase)? {
            Cached::Known { closed: true } => {}
            Cached::Known { closed: false } => open.push(phase),
            Cached::Unknown => uncached.push(phase),
        }
    }
    if !uncached.is_empty() {
        let ids: Vec<BeadId> = uncached.iter().map(|p| p.root.clone()).collect();
        let timeout = cache.config().miss_fill_timeout.min(end.remaining(clock));
        match cache.refresh(&ids, timeout) {
            Ok(rows) => {
                for phase in &uncached {
                    // A root bd does not know is no open root.
                    if rows
                        .iter()
                        .find(|r| r.bead.id == phase.root)
                        .is_some_and(|r| !is_closed(r.bead.status.as_deref()))
                    {
                        open.push(phase);
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
                set.unreadable
                    .extend(uncached.iter().map(|p| p.name.clone()));
            }
        }
    }
    // Keep the input order.
    set.active = phases
        .iter()
        .filter(|p| open.iter().any(|o| o.name == p.name))
        .cloned()
        .collect();
    Ok(set)
}
