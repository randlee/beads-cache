//! The scope registry through the core (ADR-BDC-0013, REQ-BDC-0721).
//!
//! `.beads/bd-cache.toml` is the source of truth for the scopes it defines:
//! they are created or updated in the store's registry when the cache opens
//! (`Cache::new`), and changing or removing one through the registry is
//! `CONFIG_INVALID` (`details.source: "config_scope"`): the file wins. Any
//! other scope lives in the registry only, managed with `scopes add`, `set`
//! and `remove`. Removing a scope is a prune of it (refused with
//! `PRUNE_BLOCKED` while it has open writes) followed by its removal from the
//! registry: its membership rows go, not the bead rows another scope holds.
//! These operations surface as the `bdc cache scopes` verbs (b-8,
//! ADR-BDC-0009); the beads commands keep their names, flags and replies.

use bd_cache_model::{CacheError, CacheStore, DrainLock, PruneReport, Scope, ScopeId};
use sc_beads::CacheBackend;

use crate::Cache;

/// `details.source` of the `CONFIG_INVALID` for a file scope.
pub(crate) const CONFIG_SCOPE: &str = "config_scope";

/// `CONFIG_INVALID` when the configuration file defines `s`.
fn refuse_file_scope<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    s: &ScopeId,
    verb: &str,
) -> Result<(), CacheError> {
    if cache.config.scopes.iter().any(|c| c.id == *s) {
        return Err(CacheError::ConfigInvalid {
            detail: format!(
                "scopes {verb} {s}: .beads/bd-cache.toml defines this scope, and the file wins"
            ),
            origin: CONFIG_SCOPE,
        });
    }
    Ok(())
}

/// `scopes add`: a new registry scope.
pub(crate) fn add<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    scope: &Scope,
) -> Result<(), CacheError> {
    refuse_file_scope(cache, &scope.id, "add")?;
    cache.store.scope_add(scope)
}

/// `scopes set`: a registry scope's new filter.
pub(crate) fn set<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    scope: &Scope,
) -> Result<(), CacheError> {
    refuse_file_scope(cache, &scope.id, "set")?;
    cache.store.scope_set(scope)
}

/// `scopes remove`: a prune of the scope, then its removal from the registry.
pub(crate) fn remove<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    s: &ScopeId,
) -> Result<PruneReport, CacheError> {
    refuse_file_scope(cache, s, "remove")?;
    cache.store.scope_remove(s)
}

/// The phase-a scope a 0.1 argument names (REQ-BDC-0812 as amended): `x` or
/// the legacy-prefixed `x`, read as the scope stored as `x`. A registered
/// scope of that name with a filter is never what a 0.1 name means:
/// `SCOPE_UNKNOWN`. An unregistered name is returned as is; a read of it is
/// then `SCOPE_UNKNOWN` and registers nothing.
pub(crate) fn legacy<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &Cache<S, L, B>,
    arg: &str,
) -> Result<ScopeId, CacheError> {
    let Some(id) = bd_cache_model::legacy_scope(arg) else {
        return Err(CacheError::ScopeUnknown {
            detail: format!("{arg:?} is not a scope name"),
            known: cache.store.scopes()?.into_iter().map(|k| k.id).collect(),
        });
    };
    match cache.store.scope(&id)? {
        Some(scope) if !scope.is_legacy() => Err(crate::sync::not_legacy(cache, &id)?),
        _ => Ok(id),
    }
}
