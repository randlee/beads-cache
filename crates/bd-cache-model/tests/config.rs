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
use sc_beads::{Clock, Deadline, ProjectRef};

const SAMPLE: &str = r#"
cache_path = ".cache-dir/dev.bd-cache.sqlite3"
scopes = ["bk", "phase-bj", "bk", "  "]
miss_fill_timeout_ms = 500

[[schema_packages]]
path = "schemas/beads"
enforcement = "warn"
"#;

#[test]
fn sample_config_parses_and_derives_defaults() {
    let cfg = RepoConfig::from_toml(SAMPLE).unwrap();
    assert_eq!(
        cfg.cache_path,
        Some(PathBuf::from(".cache-dir/dev.bd-cache.sqlite3"))
    );
    assert_eq!(
        cfg.schema_packages.len(),
        1,
        "schema packages are kept opaque"
    );
    let cache = cfg.cache_config(ProjectRef::local(Some("/repo".into())));
    assert_eq!(
        cache.scopes,
        vec![
            Scope::legacy(ScopeId::new("bk").unwrap()),
            Scope::legacy(ScopeId::new("bj").unwrap())
        ]
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

/// The bounds of `validate` (B7Q1-046): zero and values past the documented
/// maximum are refused; the maximum itself is accepted.
#[test]
fn timeouts_and_attempts_have_documented_maximums() {
    use bd_cache_model::{MAX_CALL_TIMEOUT_S, MAX_MAX_ATTEMPTS, MAX_MISS_FILL_TIMEOUT_MS};
    for (key, most) in [
        ("call_timeout_s", MAX_CALL_TIMEOUT_S),
        ("miss_fill_timeout_ms", MAX_MISS_FILL_TIMEOUT_MS),
        ("max_attempts", u64::from(MAX_MAX_ATTEMPTS)),
    ] {
        assert!(
            RepoConfig::from_toml(&format!("{key} = {most}\n")).is_ok(),
            "{key}"
        );
        let e = RepoConfig::from_toml(&format!("{key} = {}\n", most + 1)).unwrap_err();
        assert!(
            e.detail.contains(key) && e.detail.contains("at most"),
            "{e}"
        );
    }
}

/// Configured scopes are held to the registration limits and their count is
/// capped; a name is defined once (B7Q1-045).
#[test]
fn configured_scopes_are_held_to_the_registration_limits() {
    let long = "n".repeat(bd_cache_model::MAX_SCOPE_NAME_LEN + 1);
    let cfg = RepoConfig::from_toml(&format!("scopes = [\"{long}\"]\n")).unwrap();
    assert!(cfg.configured_scopes().is_err());
    assert!(cfg.cache_config(ProjectRef::local(None)).scopes.is_empty());
    let names: Vec<String> = (0..=bd_cache_model::MAX_CONFIGURED_SCOPES)
        .map(|i| format!("\"s{i}\""))
        .collect();
    let cfg = RepoConfig::from_toml(&format!("scopes = [{}]\n", names.join(", "))).unwrap();
    let e = cfg.configured_scopes().unwrap_err();
    assert!(e.reason.contains("more than"), "{e}");
    assert!(e.to_string().contains(&e.value), "Display names the entry");
    let fits = RepoConfig::from_toml(&format!(
        "scopes = [{}]\n",
        names[..bd_cache_model::MAX_CONFIGURED_SCOPES].join(", ")
    ))
    .unwrap();
    assert_eq!(
        fits.configured_scopes().unwrap().len(),
        bd_cache_model::MAX_CONFIGURED_SCOPES
    );
}

/// The file cannot name code to run (REQ-BDC-0724, ADR-SYNC-0008): every
/// `[tick]` table, `[tick.hooks]` and `[tick.sync]` included, is refused, so
/// the CLI reports `CONFIG_INVALID`. Planted negative: the same file without
/// the table parses.
#[test]
fn a_tick_table_cannot_name_code_to_run_and_is_refused() {
    for table in [
        "[tick]\nteam = \"dev\"\n",
        "[tick.hooks]\npost_sync = \"pkg:fn\"\n",
        "[tick.sync]\npython = \"/usr/bin/python3\"\nmodule = \"pkg.sync\"\n",
        "[tick]\n",
    ] {
        let text = format!("scopes = [\"bk\"]\n{table}");
        let err = RepoConfig::from_toml(&text).unwrap_err();
        assert!(err.detail.contains("tick"), "{table}: {err}");
    }
    assert!(RepoConfig::from_toml("scopes = [\"bk\"]\n").is_ok());
}
