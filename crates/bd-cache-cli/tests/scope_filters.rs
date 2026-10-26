//! Scope filters through the CLI against the `fake-bd` binary (ADR-BDC-0013; AC3).
//!
//! `.beads/bd-cache.toml` defines a label-criteria scope and an `All` scope with
//! `[[scope]]` tables; `bdc cache refresh scopes` syncs them through `BdCli` and the
//! real `fake-bd` process, which answers each variant's argv; the reads then serve
//! each scope's members. The argv of a status criterion is pinned here too, where the
//! model's filter and `sc-bd-cli`'s builders meet: bd is never sent the criterion
//! (planted negative: pass `--status <criterion>`, and the argv test fails).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use bd_cache_cli::scenario::Scenario;
use bd_cache_model::{FilterParts, ScopeFilter};
use sc_bd_cli::argv::{self, ALL_STATUSES};
use serde_json::Value;
use support::TestRepo;

fn bead(id: &str, labels: &[&str], issue_type: &str) -> sc_beads::Bead {
    let mut b = sc_beads::testing::bead(id);
    b.labels = labels.iter().map(|l| (*l).to_owned()).collect();
    b.issue_type = Some(issue_type.to_owned());
    b
}

fn repo() -> TestRepo {
    let mut scenario = Scenario::new("2026-10-23T10:00:00Z");
    scenario.rows = vec![
        bead("a1", &["lane"], "task"),
        bead("a2", &["lane", "red"], "bug"),
        bead("o1", &["other"], "bug"),
    ];
    let repo = TestRepo::new(&scenario);
    std::fs::write(
        repo.root().join(".beads").join("bd-cache.toml"),
        "[[scope]]\nname = \"lane\"\nlabels_all = [\"lane\"]\n\n\
         [[scope]]\nname = \"everything\"\nall = true\n",
    )
    .unwrap();
    repo
}

fn ok(repo: &TestRepo, args: &[&str]) -> Value {
    let (exit, v) = repo.json(args);
    assert_eq!(exit, 0, "{args:?}: {v}");
    assert_eq!(v["ok"], true, "{args:?}: {v}");
    v["data"].clone()
}

fn ids(data: &Value) -> Vec<String> {
    data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn sync_label_filter_scope_through_the_cli() {
    let repo = repo();
    let refreshed = ok(&repo, &["cache", "refresh", "scopes", "lane"]);
    assert_eq!(refreshed["failed"], 0, "{refreshed}");
    let listed = ok(&repo, &["issues", "list", "--scope", "lane"]);
    assert_eq!(ids(&listed), ["a1", "a2"]);
    assert_eq!(listed["x_cache"]["scope"], "lane");
    let ready = ok(&repo, &["ready", "--scope", "lane"]);
    assert_eq!(ids(&ready), ["a1", "a2"]);
}

#[test]
fn sync_all_scope_through_the_cli() {
    let repo = repo();
    let refreshed = ok(&repo, &["cache", "refresh", "scopes", "everything"]);
    assert_eq!(refreshed["failed"], 0, "{refreshed}");
    let listed = ok(&repo, &["issues", "list", "--scope", "everything"]);
    assert_eq!(ids(&listed), ["a1", "a2", "o1"]);
    let scopes = ok(&repo, &["cache", "scopes"]);
    let names: Vec<&str> = scopes["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["scope"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["everything", "lane"],
        "both file scopes are registered"
    );
}

/// A status criterion never reaches bd: the list names all seven statuses and
/// `bd ready` and `bd blocked` take no status flag (ADR-BDC-0013, AC3).
#[test]
fn a_status_criterion_is_never_passed_to_bd() {
    let open_lane = ScopeFilter::from_parts(FilterParts {
        labels_all: vec!["lane".into()],
        status: vec!["open".into()],
        ..FilterParts::default()
    })
    .unwrap();
    let backend = open_lane.backend();
    let list = argv::list_scope(&backend, None).unwrap();
    let at = list.iter().position(|w| w == "--status").unwrap();
    assert_eq!(list[at + 1], ALL_STATUSES, "{list:?}");
    assert_eq!(list.iter().filter(|w| *w == "--status").count(), 1);
    for argv in [
        argv::blocked(&backend).unwrap(),
        argv::ready(&backend).unwrap(),
    ] {
        assert!(
            !argv
                .iter()
                .any(|w| w == "--status" || w == "-s" || w == "open"),
            "{argv:?}"
        );
    }
    // A status-only scope lists every bead, all statuses.
    let status_only = ScopeFilter::from_parts(FilterParts {
        status: vec!["open".into()],
        ..FilterParts::default()
    })
    .unwrap();
    assert_eq!(
        argv::list_scope(&status_only.backend(), None).unwrap(),
        [
            "list",
            "--all",
            "--status",
            ALL_STATUSES,
            "-n",
            "0",
            "--json"
        ]
    );
}
