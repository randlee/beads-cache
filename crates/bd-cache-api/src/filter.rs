//! Scope selection and the row filters of `issues list` and `ready`
//! (REQ-BDC-0808, contract §5.1, §5.3).
//!
//! The core lists a scope and filters nothing else (`Cache::list(scope)`);
//! every filter here runs on the overlaid view, so a list agrees with `get`.
//!
//! - **Scope.** `scope`, by its name as written (ADR-BDC-0013, contract
//!   §5.1); a read that names none is `SCOPE_UNKNOWN`. No scope is derived
//!   from a label: the prefix rule is the 0.1 compatibility rule's alone.
//!   Every label is applied as an AND filter.
//! - **Default exclusions** of `issues list`: closed rows, templates and
//!   `gate` rows, unless `status` or `all` is set. Infra types are unknown to
//!   the cache, so they are not excluded.

use bd_cache_model::CLOSED_STATUS;
use sc_beads::Bead;

use crate::request::{IssuesList, Ready};

/// The issue type bd gives gate rows.
const GATE_TYPE: &str = "gate";

/// True for a row `issues list` leaves out unless `status` or `all` is set.
fn excluded_by_default(view: &Bead) -> bool {
    view.status.as_deref() == Some(CLOSED_STATUS)
        || view.is_template == Some(true)
        || view.issue_type.as_deref() == Some(GATE_TYPE)
}

/// The label filters: every `label`, any of `label_any`, none of `exclude_label`.
fn labels_match(view: &Bead, label: &[String], label_any: &[String], exclude: &[String]) -> bool {
    label.iter().all(|l| view.has_label(l))
        && (label_any.is_empty() || label_any.iter().any(|l| view.has_label(l)))
        && !exclude.iter().any(|l| view.has_label(l))
}

/// True when `issues list` keeps the row.
pub(crate) fn list_keeps(req: &IssuesList, view: &Bead) -> bool {
    let status_ok = if req.status.is_empty() {
        req.all || !excluded_by_default(view)
    } else {
        view.status.as_ref().is_some_and(|s| req.status.contains(s))
    };
    status_ok
        && labels_match(view, &req.label, &req.label_any, &req.exclude_label)
        && req
            .assignee
            .as_ref()
            .is_none_or(|a| view.assignee.as_ref() == Some(a))
}

/// True when `ready` keeps the row.
pub(crate) fn ready_keeps(req: &Ready, view: &Bead) -> bool {
    labels_match(view, &req.label, &req.label_any, &req.exclude_label)
        && req
            .assignee
            .as_ref()
            .is_none_or(|a| view.assignee.as_ref() == Some(a))
        && (!req.unassigned || view.assignee.as_deref().unwrap_or("").is_empty())
        && req.priority.is_none_or(|p| view.priority == p)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use crate::page::ListSort;

    fn list() -> IssuesList {
        IssuesList {
            status: Vec::new(),
            label: Vec::new(),
            label_any: Vec::new(),
            exclude_label: Vec::new(),
            assignee: None,
            all: false,
            sort: ListSort::Created,
            limit: None,
            cursor: None,
            scope: None,
        }
    }

    #[test]
    fn label_filters_and_or_and_not() {
        let mut b = sc_beads::testing::bead("b1");
        b.labels = vec!["a".into(), "b".into()];
        let mut r = list();
        r.label = vec!["a".into(), "b".into()];
        assert!(list_keeps(&r, &b));
        r.label = vec!["a".into(), "c".into()];
        assert!(!list_keeps(&r, &b));
        r.label = Vec::new();
        r.label_any = vec!["c".into(), "b".into()];
        assert!(list_keeps(&r, &b));
        r.exclude_label = vec!["a".into()];
        assert!(!list_keeps(&r, &b));
    }
}
