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

use bd_cache_model::{CacheConfig, DrainBudget, RepoConfig, ScopeId};
use sc_beads::{Clock, Deadline, ProjectRef};

const SAMPLE: &str = r#"
cache_path = ".atm-bd/atm-dev.bd-cache.sqlite3"
scopes = ["bk", "phase-bj", "bk", "  "]
miss_fill_timeout_ms = 500

[[schema_packages]]
path = ".claude/skills/atm-beads/schemas"
enforcement = "warn"

[tick]
team = "atm-dev"
fallback_lead = "team-lead"
phases_dir = ".atm-bd"
"#;

#[test]
fn sample_config_parses_and_derives_defaults() {
    let cfg = RepoConfig::from_toml(SAMPLE).unwrap();
    assert_eq!(
        cfg.cache_path,
        Some(PathBuf::from(".atm-bd/atm-dev.bd-cache.sqlite3"))
    );
    assert_eq!(
        cfg.schema_packages.len(),
        1,
        "schema packages are kept opaque"
    );
    let tick = cfg.tick.as_ref().unwrap();
    assert_eq!(tick.team.as_deref(), Some("atm-dev"));
    assert_eq!(tick.fallback_lead.as_deref(), Some("team-lead"));
    assert_eq!(tick.phases_dir, Some(PathBuf::from(".atm-bd")));
    let cache = cfg.cache_config(ProjectRef::local(Some("/repo".into())));
    assert_eq!(
        cache.scopes,
        vec![ScopeId::phase("bk").unwrap(), ScopeId::phase("bj").unwrap()]
    );
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
    let e = RepoConfig::from_toml("[tick]\nteams = \"x\"").unwrap_err();
    assert!(
        e.to_string().starts_with("invalid .beads/bd-cache.toml: "),
        "{e}"
    );
}

#[test]
fn drain_budget_sits_before_the_tick_end() {
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
