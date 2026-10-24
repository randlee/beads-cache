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
    assert_eq!(cfg.tick.as_ref().unwrap()["team"].as_str(), Some("atm-dev"));
    let cache = cfg.cache_config(ProjectRef::local(Some("/repo".into())));
    assert_eq!(
        cache.scopes,
        vec![ScopeId::phase("bk"), ScopeId::phase("bj")]
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
