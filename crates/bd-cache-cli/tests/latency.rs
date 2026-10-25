//! NFR-BDC-0002 (acceptance 12 of sprint b-3): a new-process `get` is under 50 ms.
//!
//! Measured in a release build only: the test is `#[ignore]` and runs in its
//! own CI step, `cargo test --release -p bd-cache-cli --test latency -- --ignored`.
//!
//! - **Gated:** the median and the p95 of 50 new-process `bdc issues get`
//!   cache hits, with nothing pending on the scope, are under 50 ms (contract
//!   §2). The scenario gives `fake-bd` a 60 ms delay on every call, so a `get`
//!   that called bd at all (a backend check on open, a miss) could not pass.
//! - **Reported, not gated:** the same with 10 pending entries in a 1,000-row
//!   scope, where the core adjusts the whole base while a write is pending.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::path::Path;
use std::time::{Duration, Instant};

use bd_cache_cli::scenario::{SCENARIO_ENV, Scenario};
use support::{BDC, FAKE_BD, TestRepo, hermetic, labelled, path_str};

/// The number of timed runs per case.
const RUNS: usize = 50;
/// The bound of NFR-BDC-0002.
const BOUND: Duration = Duration::from_millis(50);
/// The delay `fake-bd` adds to every call: more than the bound by itself.
const BD_DELAY_MS: u64 = 60;

fn scenario(rows: usize) -> Scenario {
    let mut s = Scenario::new("2026-10-23T10:00:00Z");
    s.rows = (0..rows).map(|i| labelled(&format!("b-{i}"))).collect();
    s
}

/// `(median, p95)` of `RUNS` new-process `issues get b-1` runs.
fn measure(repo: &TestRepo) -> (Duration, Duration) {
    let root = repo.root();
    let run = || {
        let started = Instant::now();
        let out = hermetic(BDC)
            .args([
                "--repo",
                path_str(&root),
                "--bd",
                FAKE_BD,
                "issues",
                "get",
                "b-1",
            ])
            .env(SCENARIO_ENV, repo.scenario_path())
            .output()
            .unwrap();
        let elapsed = started.elapsed();
        assert!(out.status.success(), "{out:?}");
        assert!(String::from_utf8_lossy(&out.stdout).contains(r#""ok":true"#));
        elapsed
    };
    for _ in 0..3 {
        run(); // warm the page cache; not timed
    }
    let mut times: Vec<Duration> = (0..RUNS).map(|_| run()).collect();
    times.sort();
    (times[RUNS / 2], times[(RUNS * 95).div_ceil(100) - 1])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn prepared(rows: usize, pending: usize) -> TestRepo {
    let repo = TestRepo::new(&scenario(rows));
    let ok = |args: &[&str]| {
        let (exit, v) = repo.json(args);
        assert_eq!(exit, 0, "{args:?}: {v}");
    };
    ok(&["cache", "refresh", "scopes", "x"]);
    for i in 0..pending {
        let key = format!("lat-{i}");
        let id = format!("b-{}", i + 2);
        ok(&[
            "issues",
            "update",
            &id,
            "--actor",
            "dev",
            "--idempotency-key",
            &key,
            "--status",
            "in_progress",
        ]);
    }
    repo.edit(|s| s.faults.slow_ms = BD_DELAY_MS);
    repo
}

#[test]
#[ignore = "release-profile latency gate: cargo test --release -p bd-cache-cli --test latency -- --ignored"]
#[expect(
    clippy::assertions_on_constants,
    reason = "the build profile is the precondition of the measurement"
)]
fn a_new_process_get_is_under_50_ms() {
    assert!(
        !cfg!(debug_assertions),
        "NFR-BDC-0002 is measured in a release build (cargo test --release)"
    );
    assert!(Path::new(BDC).exists());

    let gated = prepared(2, 0);
    let (median, p95) = measure(&gated);
    println!(
        "NFR-BDC-0002 gated: {RUNS} new-process `bdc issues get` cache hits, nothing pending, \
         fake-bd delay {BD_DELAY_MS} ms: median {:.1} ms, p95 {:.1} ms (bound {} ms)",
        ms(median),
        ms(p95),
        BOUND.as_millis()
    );

    let reported = prepared(1_000, 10);
    let (r_median, r_p95) = measure(&reported);
    println!(
        "NFR-BDC-0002 reported (not gated): 10 pending entries in a 1,000-row scope: \
         median {:.1} ms, p95 {:.1} ms",
        ms(r_median),
        ms(r_p95)
    );

    // Lead rulings S78/S79: gated on Unix only. On hosted Windows runners the
    // same code measured median 27 ms then 66 ms (p95 190-400 ms) on successive
    // runs: process creation and on-access scanning on a shared VM, not bdc.
    // There the numbers are reported; a Windows measurement on a real host
    // (fastpc4) is the evidence for that platform.
    if cfg!(windows) {
        println!("NFR-BDC-0002: reported, not gated, on Windows (S79)");
    } else {
        assert!(median < BOUND, "median {median:?} is not under {BOUND:?}");
        assert!(p95 < BOUND, "p95 {p95:?} is not under {BOUND:?}");
    }
}
