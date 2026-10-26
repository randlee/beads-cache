//! The one request decoder (REQ-BDC-0807, contract §1 rules 5 and 6).
//!
//! [`decode`] reads a JSON object for one [`Command`]. Every member is one of:
//!
//! - **served**: decoded and validated into the typed [`Request`];
//! - **a `/v0` name the cache does not serve**: `UNSUPPORTED`, with
//!   `details.unsupported` (`parameter` for a query parameter, `member` for
//!   a body member) and `details.name`;
//! - **anything else**: `BAD_ARGS` with `details.member`.
//!
//! Nothing is ignored. Unknown members are reported before unsupported
//! ones, so the code does not depend on member order. A `null` member is
//! the same as an absent one. The `/v0` names per operation are those of
//! the pinned spec, beads v1.3.2 @ `9b65b9e` (contract §5.2).
//!
//! Operation-level `UNSUPPORTED` (contract §5.4) is not decided here: a
//! front end knows its own command names and maps a `/v0` operation it does
//! not offer with `ApiError::unsupported(Unsupported::Operation, name)`
//! (`tests/error_table.rs` shows the shape). The decoder takes a
//! [`Command`], which is always served.
//!
//! The writes take a required `idempotency_key`: the cache never derives
//! one from the request (REQ-BDC-0004). A patch is validated as the journal
//! will parse it ([`Op::parse_new`]), so a front end learns of a bad
//! metadata value at decode time, with the same code.

use bd_cache_model::{IdemKey, Op, ScopeId, Seq};
use sc_beads::{Actor, BeadId, DepType, Edge};
use serde_json::{Map, Value};

use crate::error::{ApiError, Unsupported};
use crate::page::{Cursor, ListSort};
use crate::request::{
    CacheEvict, CacheFlush, CacheHealth, CacheRefreshIssues, CacheRefreshScopes, CacheScopes,
    CacheStatus, Command, Context, DependenciesAdd, DependenciesList, IssuePatch, IssuesClose,
    IssuesGet, IssuesList, IssuesUpdate, JournalAnnounce, JournalGet, JournalKey, JournalList,
    JournalListState, JournalResolve, MetadataPatch, Ready, Request,
};
use crate::v0::patch_args;

/// Decodes the JSON request of `command` (module docs).
///
/// # Errors
///
/// `BAD_ARGS` (`details.member`) for an unknown member, a missing or
/// mistyped one, an empty patch, `status: "closed"` in a patch, a missing
/// `idempotency_key` on a write, or a cursor this crate did not mint
/// (`v0_code: invalid_cursor`); `UNSUPPORTED` (`details.unsupported`) for a
/// `/v0` name the cache does not serve.
pub fn decode(command: Command, request: &Value) -> Result<Request, ApiError> {
    let empty = Map::new();
    let obj = match request {
        Value::Object(m) => m,
        Value::Null => &empty,
        _ => return Err(ApiError::bad_args("the request must be a JSON object")),
    };
    Ok(match command {
        Command::IssuesGet => Request::IssuesGet(issues_get(obj)?),
        Command::IssuesList => Request::IssuesList(issues_list(obj)?),
        Command::Ready => Request::Ready(ready(obj)?),
        Command::DependenciesList => Request::DependenciesList(dependencies_list(obj)?),
        Command::IssuesUpdate => Request::IssuesUpdate(issues_update(obj)?),
        Command::IssuesClose => Request::IssuesClose(issues_close(obj)?),
        Command::DependenciesAdd => Request::DependenciesAdd(dependencies_add(obj)?),
        Command::Context => {
            Members::check(obj, "", &[], &[], Unsupported::Member)?;
            Request::Context(Context)
        }
        Command::CacheStatus => {
            Members::check(obj, "", &[], &[], Unsupported::Member)?;
            Request::CacheStatus(CacheStatus)
        }
        Command::CacheHealth => {
            let m = Members::check(obj, "", &["probe"], &[], Unsupported::Member)?;
            Request::CacheHealth(CacheHealth {
                probe: m.flag("probe")?,
            })
        }
        Command::CacheScopes => {
            Members::check(obj, "", &[], &[], Unsupported::Member)?;
            Request::CacheScopes(CacheScopes)
        }
        Command::CacheRefreshScopes => {
            let m = Members::check(obj, "", &["scopes"], &[], Unsupported::Member)?;
            let scopes = m
                .strings("scopes")?
                .iter()
                .map(|s| scope_of("scopes", s))
                .collect::<Result<_, _>>()?;
            Request::CacheRefreshScopes(CacheRefreshScopes { scopes })
        }
        Command::CacheRefreshIssues => {
            let m = Members::check(obj, "", &["ids", "timeout_ms"], &[], Unsupported::Member)?;
            let ids = m.ids("ids")?;
            if ids.is_empty() {
                return Err(ApiError::bad_member(
                    "ids",
                    "ids must name at least one bead",
                ));
            }
            Request::CacheRefreshIssues(CacheRefreshIssues {
                ids,
                timeout_ms: m.uint("timeout_ms")?,
            })
        }
        Command::CacheFlush => Request::CacheFlush(cache_flush(obj)?),
        Command::CacheEvict => {
            let m = Members::check(obj, "", &["scope"], &[], Unsupported::Member)?;
            let scope = m.required_str("scope")?;
            Request::CacheEvict(CacheEvict {
                scope: scope_of("scope", &scope)?,
            })
        }
        Command::JournalGet => Request::JournalGet(journal_get(obj)?),
        Command::JournalList => Request::JournalList(journal_list(obj)?),
        Command::JournalResolve => {
            let m = Members::check(obj, "", &["seq", "note", "actor"], &[], Unsupported::Member)?;
            Request::JournalResolve(JournalResolve {
                seq: m.seq("seq")?,
                note: m.required_str("note")?,
                actor: m.actor()?,
            })
        }
        Command::JournalAnnounce => {
            let m = Members::check(obj, "", &["seq"], &[], Unsupported::Member)?;
            Request::JournalAnnounce(JournalAnnounce { seq: m.seq("seq")? })
        }
    })
}

/// A request object whose member names have been checked.
struct Members<'a> {
    obj: &'a Map<String, Value>,
    /// The path of `obj` in the request, `""` at the top (`patch.` below it).
    prefix: &'static str,
}

impl<'a> Members<'a> {
    /// Checks every member name: unknown first (`BAD_ARGS`), then the `/v0`
    /// names not served (`UNSUPPORTED`).
    fn check(
        obj: &'a Map<String, Value>,
        prefix: &'static str,
        served: &[&str],
        not_served: &[&str],
        what: Unsupported,
    ) -> Result<Self, ApiError> {
        let names = || obj.keys().map(String::as_str);
        if let Some(k) = names().find(|k| !served.contains(k) && !not_served.contains(k)) {
            return Err(ApiError::unknown_member(&format!("{prefix}{k}")));
        }
        if let Some(k) = names().find(|k| not_served.contains(k)) {
            return Err(ApiError::unsupported(what, &format!("{prefix}{k}")));
        }
        Ok(Self { obj, prefix })
    }

    fn path(&self, name: &str) -> String {
        format!("{}{name}", self.prefix)
    }

    fn get(&self, name: &str) -> Option<&'a Value> {
        self.obj.get(name).filter(|v| !v.is_null())
    }

    fn wrong(&self, name: &str, expected: &str) -> ApiError {
        let path = self.path(name);
        ApiError::bad_member(&path, format!("{path} must be {expected}"))
    }

    fn missing(&self, name: &str) -> ApiError {
        let path = self.path(name);
        ApiError::bad_member(&path, format!("{path} is required"))
    }

    fn str(&self, name: &str) -> Result<Option<String>, ApiError> {
        self.get(name)
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| self.wrong(name, "a string"))
            })
            .transpose()
    }

    fn required_str(&self, name: &str) -> Result<String, ApiError> {
        self.str(name)?.ok_or_else(|| self.missing(name))
    }

    fn strings(&self, name: &str) -> Result<Vec<String>, ApiError> {
        let Some(v) = self.get(name) else {
            return Ok(Vec::new());
        };
        let items = v
            .as_array()
            .ok_or_else(|| self.wrong(name, "an array of strings"))?;
        items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| self.wrong(name, "an array of strings"))
            })
            .collect()
    }

    fn ids(&self, name: &str) -> Result<Vec<BeadId>, ApiError> {
        self.strings(name)?
            .into_iter()
            .map(|s| BeadId::new(s).map_err(|e| self.wrong(name, &format!("bead ids ({e})"))))
            .collect()
    }

    fn id(&self, name: &str) -> Result<BeadId, ApiError> {
        let s = self.required_str(name)?;
        BeadId::new(s).map_err(|e| self.wrong(name, &format!("a bead id ({e})")))
    }

    fn flag(&self, name: &str) -> Result<bool, ApiError> {
        self.get(name).map_or(Ok(false), |v| {
            v.as_bool().ok_or_else(|| self.wrong(name, "a boolean"))
        })
    }

    fn uint(&self, name: &str) -> Result<Option<u64>, ApiError> {
        self.get(name)
            .map(|v| {
                v.as_u64()
                    .ok_or_else(|| self.wrong(name, "a non-negative integer"))
            })
            .transpose()
    }

    fn int(&self, name: &str) -> Result<Option<i64>, ApiError> {
        self.get(name)
            .map(|v| v.as_i64().ok_or_else(|| self.wrong(name, "an integer")))
            .transpose()
    }

    fn seq(&self, name: &str) -> Result<Seq, ApiError> {
        match self.int(name)? {
            Some(n) if n > 0 => Ok(Seq::new(n)),
            Some(_) => Err(self.wrong(name, "a positive integer")),
            None => Err(self.missing(name)),
        }
    }

    fn actor(&self) -> Result<Actor, ApiError> {
        let s = self.required_str("actor")?;
        Actor::new(s).map_err(|e| self.wrong("actor", &format!("a nonempty actor ({e})")))
    }

    /// The required idempotency key (REQ-BDC-0004): never derived.
    fn idempotency_key(&self) -> Result<IdemKey, ApiError> {
        let Some(s) = self.str("idempotency_key")? else {
            return Err(ApiError::bad_member(
                "idempotency_key",
                "idempotency_key is required on every write; the cache never derives one from \
                 the request (REQ-BDC-0004)",
            )
            .with_suggested_action(
                "pass a key that names this one write, and reuse it only to retry it",
            ));
        };
        IdemKey::new(s).map_err(|e| self.wrong("idempotency_key", &format!("a nonempty key ({e})")))
    }

    fn object(&self, name: &str) -> Result<Option<&'a Map<String, Value>>, ApiError> {
        self.get(name)
            .map(|v| v.as_object().ok_or_else(|| self.wrong(name, "an object")))
            .transpose()
    }
}

/// A scope name, taken as written (ADR-BDC-0013): no prefix is added or
/// stripped, so a name that is not a registered scope is `SCOPE_UNKNOWN` at
/// the operation, not here.
fn scope_of(member: &str, s: &str) -> Result<ScopeId, ApiError> {
    ScopeId::new(s).map_err(|e| {
        ApiError::bad_member(member, format!("{member} {s:?} is not a scope name: {e}"))
    })
}

fn optional_scope(m: &Members<'_>) -> Result<Option<ScopeId>, ApiError> {
    m.str("scope")?.map(|s| scope_of("scope", &s)).transpose()
}

fn issues_get(obj: &Map<String, Value>) -> Result<IssuesGet, ApiError> {
    let m = Members::check(
        obj,
        "",
        &["id", "miss_timeout_ms"],
        &["include_comments", "include_dependents", "brief_deps"],
        Unsupported::Parameter,
    )?;
    Ok(IssuesGet {
        id: m.id("id")?,
        miss_timeout_ms: m.uint("miss_timeout_ms")?,
    })
}

fn issues_list(obj: &Map<String, Value>) -> Result<IssuesList, ApiError> {
    let m = Members::check(
        obj,
        "",
        &[
            "status",
            "label",
            "label_any",
            "exclude_label",
            "assignee",
            "all",
            "sort",
            "limit",
            "cursor",
            "scope",
        ],
        &[
            "type",
            "parent",
            "include_templates",
            "include_gates",
            "include_infra",
            "include_ephemeral",
            "created_before",
            "created_after",
            "metadata_field",
            "has_metadata_key",
            "brief",
        ],
        Unsupported::Parameter,
    )?;
    let sort = match m.str("sort")? {
        None => ListSort::default(),
        Some(s) => ListSort::parse(&s)
            .ok_or_else(|| ApiError::bad_member("sort", "sort must be created or priority"))?,
    };
    let cursor = m
        .str("cursor")?
        .map(|c| Cursor::decode(&c, sort))
        .transpose()?;
    Ok(IssuesList {
        status: m.strings("status")?,
        label: m.strings("label")?,
        label_any: m.strings("label_any")?,
        exclude_label: m.strings("exclude_label")?,
        assignee: m.str("assignee")?,
        all: m.flag("all")?,
        sort,
        limit: m.uint("limit")?,
        cursor,
        scope: optional_scope(&m)?,
    })
}

fn ready(obj: &Map<String, Value>) -> Result<Ready, ApiError> {
    let m = Members::check(
        obj,
        "",
        &[
            "label",
            "label_any",
            "exclude_label",
            "assignee",
            "unassigned",
            "priority",
            "limit",
            "sort",
            "scope",
            "fresh_within_ms",
        ],
        &[
            "type",
            "exclude_type",
            "label_pattern",
            "label_regex",
            "parent",
            "metadata_field",
            "has_metadata_key",
            "include_ephemeral",
            "include_deferred",
            "brief",
        ],
        Unsupported::Parameter,
    )?;
    match m.str("sort")?.as_deref() {
        None | Some("priority") => {}
        Some(v @ ("hybrid" | "oldest")) => {
            return Err(
                ApiError::unsupported(Unsupported::Parameter, "sort").with_detail("value", v)
            );
        }
        Some(_) => {
            return Err(ApiError::bad_member(
                "sort",
                "sort must be priority (the only order ready serves)",
            ));
        }
    }
    Ok(Ready {
        label: m.strings("label")?,
        label_any: m.strings("label_any")?,
        exclude_label: m.strings("exclude_label")?,
        assignee: m.str("assignee")?,
        unassigned: m.flag("unassigned")?,
        priority: m.int("priority")?,
        limit: m.uint("limit")?,
        scope: optional_scope(&m)?,
        fresh_within_ms: m.uint("fresh_within_ms")?,
    })
}

fn dep_type(member: &str, s: &str) -> Result<DepType, ApiError> {
    DepType::parse(s)
        .ok_or_else(|| ApiError::bad_member(member, format!("{s:?} is not a bd dependency type")))
}

fn dependencies_list(obj: &Map<String, Value>) -> Result<DependenciesList, ApiError> {
    let m = Members::check(obj, "", &["issue_id", "type"], &[], Unsupported::Parameter)?;
    let issue_id = m.ids("issue_id")?;
    if issue_id.is_empty() {
        return Err(ApiError::bad_member(
            "issue_id",
            "issue_id must name at least one bead",
        ));
    }
    let types = m
        .strings("type")?
        .iter()
        .map(|t| dep_type("type", t))
        .collect::<Result<_, _>>()?;
    Ok(DependenciesList { issue_id, types })
}

/// The `/v0` `IssuePatchBody` members the cache does not serve.
const PATCH_NOT_SERVED: &[&str] = &[
    "title",
    "description",
    "design",
    "acceptance_criteria",
    "notes",
    "priority",
    "issue_type",
    "parent_id",
    "labels",
    "estimated_minutes",
    "external_ref",
    "due_at",
    "defer_until",
];

fn patch(obj: &Map<String, Value>) -> Result<IssuePatch, ApiError> {
    let m = Members::check(
        obj,
        "patch.",
        &[
            "status",
            "assignee",
            "add_labels",
            "remove_labels",
            "append_notes",
            "metadata",
        ],
        PATCH_NOT_SERVED,
        Unsupported::Member,
    )?;
    let status = m.str("status")?;
    if status.as_deref() == Some(bd_cache_model::CLOSED_STATUS) {
        return Err(ApiError::bad_member(
            "patch.status",
            "status \"closed\" cannot be patched: closing goes through bd's close checks",
        )
        .with_suggested_action("use issues close"));
    }
    let metadata = match m.object("metadata")? {
        None => None,
        Some(meta) => {
            let mm = Members::check(
                meta,
                "patch.metadata.",
                &["set", "unset"],
                &["replace", "merge"],
                Unsupported::Member,
            )?;
            let set = mm.object("set")?.cloned().unwrap_or_default();
            let unset = mm.strings("unset")?;
            (!set.is_empty() || !unset.is_empty()).then_some(MetadataPatch { set, unset })
        }
    };
    let p = IssuePatch {
        status,
        assignee: m.str("assignee")?,
        add_labels: m.strings("add_labels")?,
        remove_labels: m.strings("remove_labels")?,
        // An empty note appends nothing, as the journal reads it.
        append_notes: m.str("append_notes")?.filter(|n| !n.is_empty()),
        metadata,
    };
    if p == IssuePatch::default() {
        return Err(ApiError::bad_member(
            "patch",
            "the patch requests no change",
        ));
    }
    // Validated as the journal will parse it (metadata values, keys).
    Op::parse_new("update", &patch_args(&p).to_string()).map_err(ApiError::from)?;
    Ok(p)
}

fn issues_update(obj: &Map<String, Value>) -> Result<IssuesUpdate, ApiError> {
    let m = Members::check(
        obj,
        "",
        &[
            "id",
            "actor",
            "idempotency_key",
            "patch",
            "expected_status",
            "expected_assignee",
        ],
        &[
            "expected_version",
            "force_close_policy",
            "force_assignee_transfer",
            "claim",
        ],
        Unsupported::Member,
    )?;
    let id = m.id("id")?;
    let actor = m.actor()?;
    let idempotency_key = m.idempotency_key()?;
    let patch = patch(m.object("patch")?.ok_or_else(|| m.missing("patch"))?)?;
    Ok(IssuesUpdate {
        id,
        actor,
        idempotency_key,
        patch,
        expected_status: m.str("expected_status")?,
        expected_assignee: m.str("expected_assignee")?,
    })
}

fn issues_close(obj: &Map<String, Value>) -> Result<IssuesClose, ApiError> {
    let m = Members::check(
        obj,
        "",
        &[
            "id",
            "actor",
            "idempotency_key",
            "reason",
            "expected_status",
            "expected_assignee",
        ],
        &["session", "force", "expected_version"],
        Unsupported::Member,
    )?;
    Ok(IssuesClose {
        id: m.id("id")?,
        actor: m.actor()?,
        idempotency_key: m.idempotency_key()?,
        reason: m.str("reason")?,
        expected_status: m.str("expected_status")?,
        expected_assignee: m.str("expected_assignee")?,
    })
}

fn dependencies_add(obj: &Map<String, Value>) -> Result<DependenciesAdd, ApiError> {
    let m = Members::check(
        obj,
        "",
        &["actor", "idempotency_key", "edges"],
        &[],
        Unsupported::Member,
    )?;
    let actor = m.actor()?;
    let idempotency_key = m.idempotency_key()?;
    let edges = m
        .get("edges")
        .ok_or_else(|| m.missing("edges"))?
        .as_array()
        .ok_or_else(|| m.wrong("edges", "an array of edges"))?;
    let edge = match edges.as_slice() {
        [] => {
            return Err(ApiError::bad_member(
                "edges",
                "edges must hold exactly one edge",
            ));
        }
        [one] => one,
        _ => {
            return Err(ApiError::unsupported(Unsupported::Member, "edges")
                .with_detail("count", edges.len()));
        }
    };
    let e = edge
        .as_object()
        .ok_or_else(|| m.wrong("edges", "an array of edge objects"))?;
    let em = Members::check(
        e,
        "edges[0].",
        &["issue_id", "depends_on_id", "type"],
        &[],
        Unsupported::Member,
    )?;
    let kind = dep_type("edges[0].type", &em.required_str("type")?)?;
    Ok(DependenciesAdd {
        actor,
        idempotency_key,
        edge: Edge::new(em.id("issue_id")?, em.id("depends_on_id")?, kind),
    })
}

fn cache_flush(obj: &Map<String, Value>) -> Result<CacheFlush, ApiError> {
    let m = Members::check(
        obj,
        "",
        &["start_gate_ms", "deadline_ms"],
        &[],
        Unsupported::Member,
    )?;
    let start_gate_ms = m
        .uint("start_gate_ms")?
        .unwrap_or(CacheFlush::DEFAULT_START_GATE_MS);
    let deadline_ms = m
        .uint("deadline_ms")?
        .unwrap_or(CacheFlush::DEFAULT_DEADLINE_MS);
    if deadline_ms <= start_gate_ms {
        return Err(ApiError::bad_member(
            "deadline_ms",
            format!(
                "deadline_ms ({deadline_ms}) must be greater than start_gate_ms ({start_gate_ms})"
            ),
        ));
    }
    Ok(CacheFlush {
        start_gate_ms,
        deadline_ms,
    })
}

fn journal_get(obj: &Map<String, Value>) -> Result<JournalGet, ApiError> {
    let m = Members::check(
        obj,
        "",
        &["seq", "idempotency_key"],
        &[],
        Unsupported::Member,
    )?;
    let key = match (m.get("seq").is_some(), m.get("idempotency_key").is_some()) {
        (true, false) => JournalKey::Seq(m.seq("seq")?),
        (false, true) => JournalKey::IdempotencyKey(m.idempotency_key()?),
        _ => {
            return Err(ApiError::bad_member(
                "seq",
                "name the entry with exactly one of seq and idempotency_key",
            ));
        }
    };
    Ok(JournalGet { key })
}

fn journal_list(obj: &Map<String, Value>) -> Result<JournalList, ApiError> {
    let m = Members::check(obj, "", &["state", "unannounced"], &[], Unsupported::Member)?;
    let state = match m.str("state")?.as_deref() {
        None | Some("escalated") => JournalListState::Escalated,
        Some("pending") => JournalListState::Pending,
        Some(v @ ("applied" | "resolved")) => {
            return Err(
                ApiError::unsupported(Unsupported::Parameter, "state").with_detail("value", v)
            );
        }
        Some(_) => {
            return Err(ApiError::bad_member(
                "state",
                "state must be pending or escalated",
            ));
        }
    };
    let unannounced = m.flag("unannounced")?;
    if unannounced && state == JournalListState::Pending {
        return Err(ApiError::bad_member(
            "unannounced",
            "unannounced lists escalations; a pending entry is never announced",
        ));
    }
    Ok(JournalList { state, unannounced })
}
