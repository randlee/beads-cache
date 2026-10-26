//! The 0.1 compatibility rule for scope names (REQ-BDC-0812 as amended,
//! ADR-BDC-0012, ADR-BDC-0013).
//!
//! Where a 0.1 interface took a phase (the `scopes` string list of
//! `.beads/bd-cache.toml` and the 0.1 Python verbs), `x` and
//! [`LEGACY_SCOPE_PREFIX`]`x` both mean the phase-a scope stored as `x`, whose
//! label is the prefix followed by `x`. Every other interface takes a scope
//! name as written. The prefix is written once, here; every use refers to
//! the constant.

use crate::scope::ScopeId;

/// The label prefix of a phase-a scope: a scope stored as `x` with no filter
/// syncs the beads labelled with this prefix followed by `x`.
pub const LEGACY_SCOPE_PREFIX: &str = "phase-";

/// The phase-a scope a 0.1 argument names: `x` or the prefix followed by `x`,
/// with surrounding whitespace trimmed; `None` when `x` is not a valid name.
///
/// Used only on the 0.1 paths. A bare `x` always means this phase-a scope,
/// never a configured scope of another kind with the same name: the caller
/// checks the registered scope is a phase-a one (REQ-BDC-0812).
///
/// # Examples
///
/// ```
/// use bd_cache_model::{LEGACY_SCOPE_PREFIX, ScopeId, legacy_scope};
/// let x = ScopeId::new("x").ok();
/// assert_eq!(legacy_scope("x"), x);
/// assert_eq!(legacy_scope(&format!("{LEGACY_SCOPE_PREFIX}x")), x);
/// assert_eq!(legacy_scope(LEGACY_SCOPE_PREFIX), None);
/// ```
#[must_use]
pub fn legacy_scope(arg: &str) -> Option<ScopeId> {
    let name = arg.trim();
    let name = name.strip_prefix(LEGACY_SCOPE_PREFIX).unwrap_or(name);
    ScopeId::new(name).ok()
}

/// The label of the phase-a scope `id`: [`LEGACY_SCOPE_PREFIX`] followed by its name.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{LEGACY_SCOPE_PREFIX, ScopeId, legacy_label};
/// let x = ScopeId::new("x").unwrap();
/// assert_eq!(legacy_label(&x), format!("{LEGACY_SCOPE_PREFIX}x"));
/// ```
#[must_use]
pub fn legacy_label(id: &ScopeId) -> String {
    let mut label = String::with_capacity(LEGACY_SCOPE_PREFIX.len() + id.name().len());
    label.push_str(LEGACY_SCOPE_PREFIX);
    label.push_str(id.name());
    label
}
