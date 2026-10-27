//! Surface (a) `data`: `/v0` bodies plus `x_cache` (contract §1 rule 2, §5.3).
//!
//! A row is bd's list row with the journal overlay applied ([`Bead`]); a
//! `/v0` member the cache cannot know is absent, never `null` (contract §1
//! rule 3). Cache facts live in `x_cache`, never inside a row.

use sc_beads::{Bead, BeadId, Edge};
use serde::Serialize;

use crate::data::x_cache::{
    ContextXCache, DepsXCache, GetXCache, ReadXCache, ReadyXCache, WriteXCache,
};

/// `issues get`: the row, plus `x_cache`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IssuesGetData {
    /// The row.
    #[serde(flatten)]
    pub issue: Bead,
    /// The cache facts.
    pub x_cache: GetXCache,
}

/// `issues list`: `{items, has_more, next_cursor?, x_cache}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IssuesListData {
    /// The page, in the requested order.
    pub items: Vec<Bead>,
    /// More rows follow this page.
    pub has_more: bool,
    /// The cursor of the next page; absent when there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// The cache facts.
    pub x_cache: ReadXCache,
}

/// `ready`: `{items, has_more, x_cache}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadyData {
    /// The ready rows, `(priority asc, created_at desc, id asc)`.
    pub items: Vec<Bead>,
    /// `limit` cut the list.
    pub has_more: bool,
    /// The cache facts.
    pub x_cache: ReadyXCache,
}

/// `dependencies list`: `{items, missing, x_cache}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DependenciesListData {
    /// The edges `{issue_id, depends_on_id, type}`, in request order of
    /// `issue_id`, then by `depends_on_id`.
    pub items: Vec<Edge>,
    /// Requested beads the cache does not hold.
    pub missing: Vec<BeadId>,
    /// The cache facts.
    pub x_cache: DepsXCache,
}

/// `issues update`: `{issue, changed, x_cache}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IssuesUpdateData {
    /// The view after the write.
    pub issue: Bead,
    /// The overlay changed the view (a prediction: bd decides at flush);
    /// absent when unknown (lead ruling S68: a duplicate of an entry with no
    /// stored value that is no longer pending), never `null` or a guess;
    /// on a duplicate, the value stored with the original entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed: Option<bool>,
    /// The journal entry.
    pub x_cache: WriteXCache,
}

/// `issues close`: `{issue, already_closed, x_cache}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IssuesCloseData {
    /// The view after the write.
    pub issue: Bead,
    /// The view before the write was closed; on a duplicate, the stored value.
    /// It is `!changed` (a close stores `changed = !already_closed`, S62),
    /// so it is absent exactly when `changed` is unknown (S68).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_closed: Option<bool>,
    /// The journal entry.
    pub x_cache: WriteXCache,
}

/// `dependencies add`: `{added, x_cache}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DependenciesAddData {
    /// The edge journaled.
    pub added: Vec<Edge>,
    /// The journal entry.
    pub x_cache: WriteXCache,
}

/// `context`: `{bd_version?, database, project_id, beads_dir?, repo_root?,
/// capabilities, x_cache}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContextData {
    /// bd's version; absent until the first fingerprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bd_version: Option<String>,
    /// The dolt database (`metadata.json`).
    pub database: String,
    /// The project id (`metadata.json`).
    pub project_id: String,
    /// The beads directory, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beads_dir: Option<String>,
    /// The repository root, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_root: Option<String>,
    /// The `/v0` capability tokens served ([`CAPABILITIES`](crate::CAPABILITIES)).
    pub capabilities: Vec<&'static str>,
    /// The cache facts.
    pub x_cache: ContextXCache,
}
