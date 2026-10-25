//! `RepoConfig` parses `.beads/bd-cache.toml` and derives `CacheConfig` with defaults.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::path::PathBuf;
use std::time::Duration;

use bd_cache_model::{CacheConfig, DrainBudget, RepoConfig, Scope, ScopeId};
use sc_beads::{Clock, Deadline, FilterCriteria, ProjectRef, ScopeFilter};

const SAMPLE: &str = r#"
cache_path = ".bd-cache/dev.bd-cache.sqlite3"
scopes = [
  "release",
  { name = "open-bugs", status = ["open", "in_progress"], type = "bug" },
  { name = "core", labels = ["area", "team"], labels_any = ["p0", "p1"], assignee = "dev-1" },
  { name = "everything", all = true },
]
miss_fill_timeout_ms = 500

[[schema_packages]]
path = "schemas"
enforcement = "warn"
"#;

fn id(name: &str) -> ScopeId {
    ScopeId::new(name).unwrap()
}

/// Scopes are bd filters (REQ-BDC-0702, ANC-009): a label string, criteria
/// tables, and an all-beads table, in the configured order.
#[test]
fn sample_config_parses_and_derives_defaults() {
    let cfg = RepoConfig::from_toml(SAMPLE).unwrap();
    assert_eq!(
        cfg.cache_path,
        Some(PathBuf::from(".bd-cache/dev.bd-cache.sqlite3"))
    );
    assert_eq!(
        cfg.schema_packages.len(),
        1,
        "schema packages are kept opaque"
    );
    let cache = cfg.cache_config(ProjectRef::local(Some("/repo".into())));
    let names: Vec<&str> = cache.scopes.iter().map(|s| s.id().name()).collect();
    assert_eq!(names, ["release", "open-bugs", "core", "everything"]);
    assert_eq!(cache.scopes[0], Scope::label(id("release")));
    assert_eq!(
        cache.scopes[1].filter(),
        &ScopeFilter::Match(FilterCriteria {
            status: vec!["open".into(), "in_progress".into()],
            issue_type: Some("bug".into()),
            ..FilterCriteria::default()
        })
    );
    assert_eq!(
        cache.scopes[2].filter(),
        &ScopeFilter::Match(FilterCriteria {
            labels: vec!["area".into(), "team".into()],
            labels_any: vec!["p0".into(), "p1".into()],
            assignee: Some("dev-1".into()),
            ..FilterCriteria::default()
        })
    );
    assert_eq!(cache.scopes[3].filter(), &ScopeFilter::All);
    assert_eq!(cache.call_timeout, Duration::from_secs(30));
    assert_eq!(cache.miss_fill_timeout, Duration::from_millis(500));
    assert_eq!(cache.max_attempts, 5);
    assert_eq!(cache.project, ProjectRef::local(Some("/repo".into())));
}

#[test]
fn empty_config_is_all_defaults_and_unknown_keys_are_errors() {
    let cfg = RepoConfig::from_toml("").unwrap();
    assert_eq!(cfg, RepoConfig::default());
    let cache = cfg.cache_config(ProjectRef::local(None));
    assert_eq!(cache, CacheConfig::new(ProjectRef::local(None), vec![]));
    assert_eq!(cache.miss_fill_timeout, Duration::from_secs(2));
    assert!(RepoConfig::from_toml("surprise = true").is_err());
    assert!(RepoConfig::from_toml("scopes = \"bk\"").is_err());
    let e = RepoConfig::from_toml("scopes = [{ name = \"x\", all = true, surprise = 1 }]")
        .unwrap_err();
    assert!(
        e.to_string().starts_with("invalid .beads/bd-cache.toml: "),
        "{e}"
    );
}

/// The removed keys of a scheduler table are refused, not ignored
/// (ADR-BDC-0010): the schedule is a row of the cron table, never a key.
#[test]
fn the_removed_tick_table_is_refused() {
    for text in [
        "[tick]\n",
        "[tick]\nphases = [\"x\"]\n",
        "[tick]\nphases_dir = \"d\"\n",
    ] {
        assert!(RepoConfig::from_toml(text).is_err(), "{text:?}");
    }
}

/// A scope entry that bd would misread, or one named twice, is refused
/// with the entry's name (REQ-BDC-0702).
#[test]
fn bad_scope_entries_are_refused_naming_the_entry() {
    for (text, name) in [
        ("scopes = [\"b k\"]", "b k"),
        ("scopes = [\"bk\", \"bk\"]", "bk"),
        ("scopes = [{ name = \"x\" }]", "x"),
        ("scopes = [{ name = \"x\", all = true, labels = [\"a\"] }]", "x"),
        ("scopes = [{ name = \"x\", labels = [\"a,b\"] }]", "x"),
        ("scopes = [{ name = \"x\", assignee = \"-a\" }]", "x"),
    ] {
        let e = RepoConfig::from_toml(text).unwrap_err();
        assert!(e.detail.contains(&format!("{name:?}")), "{text}: {e}");
    }
}

#[test]
fn drain_budget_sits_before_the_run_end() {
    let clock = sc_beads::testing::ManualClock::default();
    let end = Deadline::after(&clock, Duration::from_secs(25));
    let b = DrainBudget::before_tick_end(end);
    assert_eq!(b.start_gate.remaining(&clock), Duration::from_secs(20));
    assert_eq!(b.hard.remaining(&clock), Duration::from_secs(23));
    assert!(clock.now_mono() < b.start_gate.instant());
}

/// PE-RSH-006: each zero value is refused with a message of its own; 1 is the floor.
#[test]
fn zero_call_timeout_miss_fill_timeout_and_max_attempts_are_refused() {
    for (key, unit) in [
        ("call_timeout_s", "1 second"),
        ("miss_fill_timeout_ms", "1 millisecond"),
        ("max_attempts", "1 attempt"),
    ] {
        let e = RepoConfig::from_toml(&format!("{key} = 0\n")).unwrap_err();
        assert!(
            e.detail.starts_with(&format!(
                "{key} = 0 is out of range: it must be at least {unit} ("
            )),
            "{key}: {}",
            e.detail
        );
        let ok = RepoConfig::from_toml(&format!("{key} = 1\n")).unwrap();
        assert!(ok.validate().is_ok(), "{key} = 1");
    }
    let cache =
        RepoConfig::from_toml("call_timeout_s = 1\nmiss_fill_timeout_ms = 1\nmax_attempts = 1\n")
            .unwrap()
            .cache_config(ProjectRef::local(None));
    assert_eq!(
        (
            cache.call_timeout,
            cache.miss_fill_timeout,
            cache.max_attempts
        ),
        (Duration::from_secs(1), Duration::from_millis(1), 1)
    );
}
