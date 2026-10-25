//! Typed requests: one model per command of contract §5.3 and §6.
//!
//! [`decode`](crate::decode) builds them from JSON; every value in them has
//! been validated. `cache tick` has no model here: its request and reply
//! belong to `bd-cache-sync`.

mod beads;
mod manage;

#[doc(inline)]
pub use beads::{
    Context, DependenciesAdd, DependenciesList, IssuePatch, IssuesClose, IssuesGet, IssuesList,
    IssuesUpdate, MetadataPatch, Ready,
};
#[doc(inline)]
pub use manage::{
    CacheEvict, CacheFlush, CacheHealth, CacheRefreshIssues, CacheRefreshScopes, CacheScopes,
    CacheScopesAnnounce, CacheScopesIngest, CacheStatus, JournalAnnounce, JournalGet, JournalKey,
    JournalList, JournalListState, JournalResolve,
};

/// A command of the contract, by its path (`issues get`, `cache journal list`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// `issues get`.
    IssuesGet,
    /// `issues list`.
    IssuesList,
    /// `ready`.
    Ready,
    /// `dependencies list`.
    DependenciesList,
    /// `issues update`.
    IssuesUpdate,
    /// `issues close`.
    IssuesClose,
    /// `dependencies add`.
    DependenciesAdd,
    /// `context`.
    Context,
    /// `cache status`.
    CacheStatus,
    /// `cache health`.
    CacheHealth,
    /// `cache scopes`.
    CacheScopes,
    /// `cache scopes ingest`.
    CacheScopesIngest,
    /// `cache scopes announce`.
    CacheScopesAnnounce,
    /// `cache refresh scopes`.
    CacheRefreshScopes,
    /// `cache refresh issues`.
    CacheRefreshIssues,
    /// `cache flush`.
    CacheFlush,
    /// `cache evict`.
    CacheEvict,
    /// `cache journal get`.
    JournalGet,
    /// `cache journal list`.
    JournalList,
    /// `cache journal resolve`.
    JournalResolve,
    /// `cache journal announce`.
    JournalAnnounce,
}

impl Command {
    /// Every command, surface (a) first, in contract order.
    pub const ALL: [Command; 21] = [
        Self::IssuesGet,
        Self::IssuesList,
        Self::Ready,
        Self::DependenciesList,
        Self::IssuesUpdate,
        Self::IssuesClose,
        Self::DependenciesAdd,
        Self::Context,
        Self::CacheStatus,
        Self::CacheHealth,
        Self::CacheScopes,
        Self::CacheScopesIngest,
        Self::CacheScopesAnnounce,
        Self::CacheRefreshScopes,
        Self::CacheRefreshIssues,
        Self::CacheFlush,
        Self::CacheEvict,
        Self::JournalGet,
        Self::JournalList,
        Self::JournalResolve,
        Self::JournalAnnounce,
    ];

    /// The command path, words joined by one space (e.g. `cache journal get`).
    #[must_use]
    pub fn path(self) -> &'static str {
        match self {
            Self::IssuesGet => "issues get",
            Self::IssuesList => "issues list",
            Self::Ready => "ready",
            Self::DependenciesList => "dependencies list",
            Self::IssuesUpdate => "issues update",
            Self::IssuesClose => "issues close",
            Self::DependenciesAdd => "dependencies add",
            Self::Context => "context",
            Self::CacheStatus => "cache status",
            Self::CacheHealth => "cache health",
            Self::CacheScopes => "cache scopes",
            Self::CacheScopesIngest => "cache scopes ingest",
            Self::CacheScopesAnnounce => "cache scopes announce",
            Self::CacheRefreshScopes => "cache refresh scopes",
            Self::CacheRefreshIssues => "cache refresh issues",
            Self::CacheFlush => "cache flush",
            Self::CacheEvict => "cache evict",
            Self::JournalGet => "cache journal get",
            Self::JournalList => "cache journal list",
            Self::JournalResolve => "cache journal resolve",
            Self::JournalAnnounce => "cache journal announce",
        }
    }

    /// The command with this path; `None` for any other text.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_api::Command;
    /// assert_eq!(Command::parse("cache journal get"), Some(Command::JournalGet));
    /// assert_eq!(Command::parse("cache tick"), None);
    /// ```
    #[must_use]
    pub fn parse(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.path() == path)
    }

    /// True for the journaled writes, which require an idempotency key.
    #[must_use]
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Self::IssuesUpdate | Self::IssuesClose | Self::DependenciesAdd
        )
    }
}

/// A decoded request.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// `issues get`.
    IssuesGet(IssuesGet),
    /// `issues list`.
    IssuesList(IssuesList),
    /// `ready`.
    Ready(Ready),
    /// `dependencies list`.
    DependenciesList(DependenciesList),
    /// `issues update`.
    IssuesUpdate(IssuesUpdate),
    /// `issues close`.
    IssuesClose(IssuesClose),
    /// `dependencies add`.
    DependenciesAdd(DependenciesAdd),
    /// `context`.
    Context(Context),
    /// `cache status`.
    CacheStatus(CacheStatus),
    /// `cache health`.
    CacheHealth(CacheHealth),
    /// `cache scopes`.
    CacheScopes(CacheScopes),
    /// `cache scopes ingest`.
    CacheScopesIngest(CacheScopesIngest),
    /// `cache scopes announce`.
    CacheScopesAnnounce(CacheScopesAnnounce),
    /// `cache refresh scopes`.
    CacheRefreshScopes(CacheRefreshScopes),
    /// `cache refresh issues`.
    CacheRefreshIssues(CacheRefreshIssues),
    /// `cache flush`.
    CacheFlush(CacheFlush),
    /// `cache evict`.
    CacheEvict(CacheEvict),
    /// `cache journal get`.
    JournalGet(JournalGet),
    /// `cache journal list`.
    JournalList(JournalList),
    /// `cache journal resolve`.
    JournalResolve(JournalResolve),
    /// `cache journal announce`.
    JournalAnnounce(JournalAnnounce),
}

impl Request {
    /// The command this request is for.
    #[must_use]
    pub fn command(&self) -> Command {
        match self {
            Self::IssuesGet(_) => Command::IssuesGet,
            Self::IssuesList(_) => Command::IssuesList,
            Self::Ready(_) => Command::Ready,
            Self::DependenciesList(_) => Command::DependenciesList,
            Self::IssuesUpdate(_) => Command::IssuesUpdate,
            Self::IssuesClose(_) => Command::IssuesClose,
            Self::DependenciesAdd(_) => Command::DependenciesAdd,
            Self::Context(_) => Command::Context,
            Self::CacheStatus(_) => Command::CacheStatus,
            Self::CacheHealth(_) => Command::CacheHealth,
            Self::CacheScopes(_) => Command::CacheScopes,
            Self::CacheScopesIngest(_) => Command::CacheScopesIngest,
            Self::CacheScopesAnnounce(_) => Command::CacheScopesAnnounce,
            Self::CacheRefreshScopes(_) => Command::CacheRefreshScopes,
            Self::CacheRefreshIssues(_) => Command::CacheRefreshIssues,
            Self::CacheFlush(_) => Command::CacheFlush,
            Self::CacheEvict(_) => Command::CacheEvict,
            Self::JournalGet(_) => Command::JournalGet,
            Self::JournalList(_) => Command::JournalList,
            Self::JournalResolve(_) => Command::JournalResolve,
            Self::JournalAnnounce(_) => Command::JournalAnnounce,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, reason = "test code")]

    use super::*;

    /// The request models carry schemars schemas (for the MCP server, §12a).
    #[test]
    fn a_write_schema_requires_its_idempotency_key() {
        let schema = serde_json::to_value(schemars::schema_for!(IssuesUpdate)).unwrap_or_default();
        let required = schema["required"].as_array().cloned().unwrap_or_default();
        assert!(required.contains(&"idempotency_key".into()), "{schema}");
        assert!(schema["properties"]["patch"].is_object(), "{schema}");
    }

    #[test]
    fn every_command_path_parses_back() {
        for c in Command::ALL {
            assert_eq!(Command::parse(c.path()), Some(c));
        }
        assert!(Command::IssuesClose.is_write() && !Command::Ready.is_write());
    }
}
