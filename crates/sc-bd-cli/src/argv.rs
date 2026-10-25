//! The argv of every cache call: pure builders, one per `bd` command form.
//!
//! These are the command forms of the BD-CACHE baseline ("Bead shape", "Journal
//! operations") and REQ-BDC-0201; the tests at the foot of this file pin each one as
//! a literal vector. The builders hold no state and spawn nothing, so
//! [`BdCli`](crate::BdCli) and the recorded-argv tests use exactly the same code.
//!
//! Reads end in `--json`; writes do not (bd prints a human line on success and the
//! exit status carries the result). A global `--actor` is always the last flag of a
//! write. Flag values are separate argv words, so no shell quoting is involved and
//! a value is never re-split: `--assignee ""` is the two words `--assignee` and an
//! empty string.

use sc_beads::{Actor, BeadId, DepType, MetaValue, Patch, ScopeFilter, SyncCursor};

/// The seven statuses a list must name, because `bd list --id` and `bd list -l`
/// omit closed beads unless the status list is explicit (P-6, REQ-BDC-0201).
pub const ALL_STATUSES: &str = "open,in_progress,blocked,deferred,closed,pinned,hooked";

/// How many ids go into one `bd list --id a,b,...` call.
///
/// A bead id is around ten characters, so a chunk is about a kilobyte of argv: far
/// under the 32 767-character command-line limit of Windows (the tightest of the
/// supported platforms) and long enough that a drain's pre-read of a whole phase is
/// a handful of calls. Raising it risks a spawn failure on Windows; lowering it only
/// costs extra `bd` processes.
pub const IDS_PER_CALL: usize = 100;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| (*w).to_owned()).collect()
}

/// `bd version`.
#[must_use]
pub fn version() -> Vec<String> {
    words(&["version"])
}

/// `bd schema`.
#[must_use]
pub fn schema() -> Vec<String> {
    words(&["schema"])
}

/// `bd list --id a,b,... --status <all seven> -n 0 --json` for one chunk of ids.
#[must_use]
pub fn list_by_ids(ids: &[&BeadId]) -> Vec<String> {
    let joined = ids
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let mut argv = words(&["list", "--id"]);
    argv.push(joined);
    argv.extend(words(&["--status", ALL_STATUSES, "-n", "0", "--json"]));
    argv
}

/// The bd commands that take a scope filter, and so the flags each accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterCommand {
    /// `bd list`: `--all`, `-l`, `--label-any`, `-t`, `-a`.
    List,
    /// `bd blocked`: `-l` and `--label-any` only.
    Blocked,
    /// `bd ready`: `-l`, `--label-any`, `-t`, `-a`.
    Ready,
}

/// The selection flags of a scope filter (REQ-BDC-0702) that `command`
/// accepts: `--all` for an all-beads filter on `bd list` (the other two list
/// every bead without a filter), else each label as `-l`, each any-label as
/// `--label-any`, then `-t` and `-a`. Every value is its own argv word.
fn push_filter(argv: &mut Vec<String>, filter: &ScopeFilter, command: FilterCommand) {
    if filter.is_all() {
        if command == FilterCommand::List {
            argv.push("--all".to_owned());
        }
        return;
    }
    for label in filter.labels() {
        argv.push("-l".to_owned());
        argv.push(label.clone());
    }
    for label in filter.labels_any() {
        argv.push("--label-any".to_owned());
        argv.push(label.clone());
    }
    if command == FilterCommand::Blocked {
        return;
    }
    if let Some(t) = filter.issue_type() {
        argv.push("-t".to_owned());
        argv.push(t.to_owned());
    }
    if let Some(a) = filter.assignee() {
        argv.push("-a".to_owned());
        argv.push(a.to_owned());
    }
}

/// `bd list <filter> --status <all seven> -n 0 [--updated-after <cursor>] --json`.
///
/// The filter's selection flags (`--all`, or `-l`, `--label-any`, `-t`, `-a`);
/// every status is listed, so a bead whose status changed is returned and the
/// caller applies the filter's status criterion to the rows.
#[must_use]
pub fn list_scope(filter: &ScopeFilter, since: Option<&SyncCursor>) -> Vec<String> {
    let mut argv = words(&["list"]);
    push_filter(&mut argv, filter, FilterCommand::List);
    argv.extend(words(&["--status", ALL_STATUSES, "-n", "0"]));
    if let Some(cursor) = since {
        argv.push("--updated-after".to_owned());
        argv.push(cursor.as_str().to_owned());
    }
    argv.push("--json".to_owned());
    argv
}

/// `bd blocked [-l <label>]... [--label-any <label>]... --json`.
///
/// `bd blocked` takes only label filters: a filter's type and assignee are
/// not passed, so the answer is a superset of the scope's blocked rows.
#[must_use]
pub fn blocked(filter: &ScopeFilter) -> Vec<String> {
    let mut argv = words(&["blocked"]);
    push_filter(&mut argv, filter, FilterCommand::Blocked);
    argv.push("--json".to_owned());
    argv
}

/// `bd ready <filter> -n 0 --json` (`-l`, `--label-any`, `-t`, `-a`).
#[must_use]
pub fn ready(filter: &ScopeFilter) -> Vec<String> {
    let mut argv = words(&["ready"]);
    push_filter(&mut argv, filter, FilterCommand::Ready);
    argv.extend(words(&["-n", "0", "--json"]));
    argv
}

/// `bd close <id> [--reason <reason>] --actor <actor>`.
#[must_use]
pub fn close(id: &BeadId, reason: Option<&str>, actor: &Actor) -> Vec<String> {
    let mut argv = words(&["close"]);
    argv.push(id.as_str().to_owned());
    if let Some(reason) = reason {
        argv.push("--reason".to_owned());
        argv.push(reason.to_owned());
    }
    push_actor(&mut argv, actor);
    argv
}

/// `bd update <id>` with every requested patch field, the guards and the actor.
///
/// Fields come in the patch's declaration order (REQ-BDC-0301): `--status`,
/// `--assignee`, `--add-label`, `--remove-label`, `--set-metadata k=v` (keys sorted),
/// `--unset-metadata`, `--append-notes`; then `--if-status` and `--if-assignee`
/// (`""` requires unassigned); then `--actor`. Metadata values are passed as
/// [`MetaValue::to_bd_arg`] text, so a bool is `true` or `false`, never `True`.
#[must_use]
pub fn update(id: &BeadId, patch: &Patch, guards: &sc_beads::Guards, actor: &Actor) -> Vec<String> {
    let mut argv = words(&["update"]);
    argv.push(id.as_str().to_owned());
    if let Some(status) = &patch.status {
        argv.push("--status".to_owned());
        argv.push(status.clone());
    }
    if let Some(assignee) = &patch.assignee {
        argv.push("--assignee".to_owned());
        argv.push(assignee.as_wire().to_owned());
    }
    for label in &patch.add_labels {
        argv.push("--add-label".to_owned());
        argv.push(label.clone());
    }
    for label in &patch.remove_labels {
        argv.push("--remove-label".to_owned());
        argv.push(label.clone());
    }
    for (key, value) in &patch.set_metadata {
        argv.push("--set-metadata".to_owned());
        argv.push(metadata_pair(key, value));
    }
    for key in &patch.unset_metadata {
        argv.push("--unset-metadata".to_owned());
        argv.push(key.clone());
    }
    if let Some(notes) = &patch.append_notes {
        argv.push("--append-notes".to_owned());
        argv.push(notes.clone());
    }
    if let Some(status) = &guards.status {
        argv.push("--if-status".to_owned());
        argv.push(status.clone());
    }
    if let Some(assignee) = &guards.assignee {
        argv.push("--if-assignee".to_owned());
        argv.push(assignee.as_wire().to_owned());
    }
    push_actor(&mut argv, actor);
    argv
}

/// `bd dep add <id> <on> --type <kind> --actor <actor>`.
#[must_use]
pub fn dep_add(id: &BeadId, on: &BeadId, kind: DepType, actor: &Actor) -> Vec<String> {
    let mut argv = words(&["dep", "add"]);
    argv.push(id.as_str().to_owned());
    argv.push(on.as_str().to_owned());
    argv.push("--type".to_owned());
    argv.push(kind.as_str().to_owned());
    push_actor(&mut argv, actor);
    argv
}

/// `key=value` for `--set-metadata`.
fn metadata_pair(key: &str, value: &MetaValue) -> String {
    format!("{key}={}", value.to_bd_arg())
}

fn push_actor(argv: &mut Vec<String>, actor: &Actor) {
    argv.push("--actor".to_owned());
    argv.push(actor.as_str().to_owned());
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sc_beads::{Assignee, Guards};

    use super::*;

    fn id(s: &str) -> BeadId {
        BeadId::new(s).unwrap()
    }

    fn actor() -> Actor {
        Actor::new("san").unwrap()
    }

    fn strings(list: &[&str]) -> Vec<String> {
        words(list)
    }

    #[test]
    fn reads_are_the_baseline_forms() {
        assert_eq!(version(), strings(&["version"]));
        assert_eq!(schema(), strings(&["schema"]));
        let (a, b) = (id("a-1"), id("b-2"));
        assert_eq!(
            list_by_ids(&[&a, &b]),
            strings(&[
                "list",
                "--id",
                "a-1,b-2",
                "--status",
                "open,in_progress,blocked,deferred,closed,pinned,hooked",
                "-n",
                "0",
                "--json"
            ])
        );
        let scope = ScopeFilter::Label("phase-x".into());
        assert_eq!(
            list_scope(&scope, None),
            strings(&[
                "list",
                "-l",
                "phase-x",
                "--status",
                "open,in_progress,blocked,deferred,closed,pinned,hooked",
                "-n",
                "0",
                "--json"
            ])
        );
        assert_eq!(
            list_scope(&scope, Some(&SyncCursor::new("2026-10-23T09:05:00Z"))),
            strings(&[
                "list",
                "-l",
                "phase-x",
                "--status",
                "open,in_progress,blocked,deferred,closed,pinned,hooked",
                "-n",
                "0",
                "--updated-after",
                "2026-10-23T09:05:00Z",
                "--json"
            ])
        );
        assert_eq!(
            blocked(&scope),
            strings(&["blocked", "-l", "phase-x", "--json"])
        );
        assert_eq!(
            ready(&scope),
            strings(&["ready", "-l", "phase-x", "-n", "0", "--json"])
        );
    }

    /// A scope is a bd filter (REQ-BDC-0702, ANC-009): an all-beads scope
    /// lists with `--all`, and a criteria scope passes each criterion as its
    /// own flag; `bd blocked` gets only the label flags it accepts.
    #[test]
    fn filter_scopes_pass_each_criterion_to_the_command_that_takes_it() {
        const ALL: &str = "open,in_progress,blocked,deferred,closed,pinned,hooked";
        assert_eq!(
            list_scope(&ScopeFilter::All, None),
            strings(&["list", "--all", "--status", ALL, "-n", "0", "--json"])
        );
        assert_eq!(blocked(&ScopeFilter::All), strings(&["blocked", "--json"]));
        assert_eq!(
            ready(&ScopeFilter::All),
            strings(&["ready", "-n", "0", "--json"])
        );
        let criteria = ScopeFilter::Match(sc_beads::FilterCriteria {
            labels: vec!["area:core".into(), "team".into()],
            labels_any: vec!["p0".into(), "p1".into()],
            status: vec!["open".into()],
            issue_type: Some("bug".into()),
            assignee: Some("dev-1".into()),
        });
        assert_eq!(
            list_scope(&criteria, None),
            strings(&[
                "list",
                "-l",
                "area:core",
                "-l",
                "team",
                "--label-any",
                "p0",
                "--label-any",
                "p1",
                "-t",
                "bug",
                "-a",
                "dev-1",
                "--status",
                ALL,
                "-n",
                "0",
                "--json"
            ])
        );
        assert_eq!(
            blocked(&criteria),
            strings(&[
                "blocked",
                "-l",
                "area:core",
                "-l",
                "team",
                "--label-any",
                "p0",
                "--label-any",
                "p1",
                "--json"
            ])
        );
        assert_eq!(
            ready(&criteria),
            strings(&[
                "ready",
                "-l",
                "area:core",
                "-l",
                "team",
                "--label-any",
                "p0",
                "--label-any",
                "p1",
                "-t",
                "bug",
                "-a",
                "dev-1",
                "-n",
                "0",
                "--json"
            ])
        );
    }

    #[test]
    fn writes_are_the_baseline_forms() {
        assert_eq!(
            close(&id("b-1"), None, &actor()),
            strings(&["close", "b-1", "--actor", "san"])
        );
        assert_eq!(
            close(&id("b-1"), Some("done: ok"), &actor()),
            strings(&["close", "b-1", "--reason", "done: ok", "--actor", "san"])
        );
        assert_eq!(
            dep_add(&id("b-1"), &id("b-2"), DepType::ParentChild, &actor()),
            strings(&[
                "dep",
                "add",
                "b-1",
                "b-2",
                "--type",
                "parent-child",
                "--actor",
                "san"
            ])
        );
    }

    #[test]
    fn update_lists_every_field_in_order_then_guards_then_actor() {
        let patch = Patch {
            status: Some("in_progress".into()),
            assignee: Some(Assignee::Named("dev".into())),
            add_labels: vec!["new".into(), "newer".into()],
            remove_labels: vec!["old".into()],
            set_metadata: BTreeMap::from([
                ("n".to_owned(), MetaValue::Int(-5)),
                ("ok".to_owned(), MetaValue::Bool(true)),
                ("off".to_owned(), MetaValue::Bool(false)),
                ("sha".to_owned(), MetaValue::Str("abc123".into())),
            ]),
            unset_metadata: vec!["gone".into()],
            append_notes: Some("second".into()),
        };
        let guards = Guards {
            status: Some("open".into()),
            assignee: Some(Assignee::Unassigned),
        };
        assert_eq!(
            update(&id("b-1"), &patch, &guards, &actor()),
            strings(&[
                "update",
                "b-1",
                "--status",
                "in_progress",
                "--assignee",
                "dev",
                "--add-label",
                "new",
                "--add-label",
                "newer",
                "--remove-label",
                "old",
                "--set-metadata",
                "n=-5",
                "--set-metadata",
                "off=false",
                "--set-metadata",
                "ok=true",
                "--set-metadata",
                "sha=abc123",
                "--unset-metadata",
                "gone",
                "--append-notes",
                "second",
                "--if-status",
                "open",
                "--if-assignee",
                "",
                "--actor",
                "san"
            ])
        );
    }

    #[test]
    fn unassign_is_an_empty_assignee_word() {
        let patch = Patch {
            assignee: Some(Assignee::Unassigned),
            ..Patch::default()
        };
        assert_eq!(
            update(&id("b-1"), &patch, &Guards::default(), &actor()),
            strings(&["update", "b-1", "--assignee", "", "--actor", "san"])
        );
    }
}
