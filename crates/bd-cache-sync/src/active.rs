//! Which phases are active (REQ-BDC-0702).
//!
//! A phase is active while its root bead is not closed. A root the cache
//! holds in the phase's scope is read from the cache (a sync keeps it
//! current). Every other root is read with one `bd list --id` of all of them
//! under the miss-fill timeout (`Cache::refresh`), so a root that is
//! labelled outside its own phase is still seen closing. A root that cannot
//! be read counts as inactive for this tick and is reported
//! (BD-CACHE.md, Cron module); a root bd does not know is no open root.

use bd_cache::Cache;
use bd_cache_model::{CacheError, CacheStore, DrainLock};
use sc_beads::{BeadId, CacheBackend, Deadline};

use crate::phases::Phase;

/// The result of looking at every phase's root.
#[derive(Debug, Default)]
pub(crate) struct ActiveSet {
    /// Phases with a root that is not closed, in input order.
    pub active: Vec<Phase>,
    /// Phases whose root could not be read.
    pub unreadable: Vec<Phase>,
}

/// True when the row's status is `closed`.
fn is_closed(status: Option<&str>) -> bool {
    status == Some(bd_cache::CLOSED_STATUS)
}

/// What the cache knows of a phase's root.
enum Cached {
    /// The root is in the phase's scope, which has synced, with this closed-ness.
    Known { closed: bool },
    /// Not held, not in the phase, or the scope never synced.
    Unknown,
}

/// One cached read of the root (`Cache::cached`: no bd call, no scan of the
/// scope): trusted only when the phase's own label holds it and its scope has
/// synced, since a sync is what keeps it current.
fn cached_root<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    phase: &Phase,
) -> Result<Cached, CacheError> {
    Ok(match cache.cached(&phase.root)? {
        Some(reply) if phase.scope().holds(&reply.row.bead) && reply.scope_last_ok_at.is_some() => {
            Cached::Known {
                closed: is_closed(reply.row.bead.status.as_deref()),
            }
        }
        _ => Cached::Unknown,
    })
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
    end: Deadline,
) -> Result<ActiveSet, CacheError> {
    let clock = cache.clock();
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
    let mut unreadable: Vec<&Phase> = Vec::new();
    if !uncached.is_empty() {
        let ids: Vec<BeadId> = uncached.iter().map(|p| p.root.clone()).collect();
        let timeout = cache.config().miss_fill_timeout.min(end.remaining(&*clock));
        match cache.refresh(&ids, timeout) {
            Ok(rows) => {
                for phase in &uncached {
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
