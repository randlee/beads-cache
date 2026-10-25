//! The real-process harness of contract §8 "Testing", shared by the integration tests.
//!
//! [`TestRepo`] is a temporary repository with a `.beads/metadata.json` and a
//! scenario file. It runs `bdc --repo <repo> --bd <fake-bd>` as a real process,
//! hands `BdCli` the same `fake-bd` binary, and parses what comes back.
#![allow(
    dead_code,
    reason = "each integration test file uses a different part of the harness"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;

use bd_cache_cli::scenario::{SCENARIO_ENV, Scenario};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{ProjectRef, SystemClock};
use sc_command::Envelope;
use serde_json::Value;
use tempfile::TempDir;

/// The `bdc` binary built for this test run.
pub const BDC: &str = env!("CARGO_BIN_EXE_bdc");
/// The `bd-cache` binary built for this test run.
pub const BD_CACHE: &str = env!("CARGO_BIN_EXE_bd-cache");
/// The `fake-bd` binary built for this test run.
pub const FAKE_BD: &str = env!("CARGO_BIN_EXE_fake-bd");

/// A bead label the scenarios use for their scope.
pub const LABEL: &str = "phase-x";

/// What a process printed and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    /// The exit code.
    pub exit: i32,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

impl From<Output> for Ran {
    fn from(out: Output) -> Self {
        Self {
            exit: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }
}

/// Runs `binary` with `args` and no scenario.
pub fn run_binary(binary: &str, args: &[&str]) -> Ran {
    Command::new(binary)
        .args(args)
        .output()
        .expect("the binary starts")
        .into()
}

/// Parses stdout as exactly one sc-command envelope and a newline.
///
/// The decoder is sc-command's own, so an extra or missing key fails too.
///
/// # Errors
///
/// A description of why `stdout` is not one envelope.
pub fn parse_envelope(stdout: &str) -> Result<Envelope<Value>, String> {
    let line = stdout
        .strip_suffix('\n')
        .ok_or_else(|| format!("stdout does not end in a newline: {stdout:?}"))?;
    if line.contains('\n') {
        return Err(format!("stdout is more than one line: {stdout:?}"));
    }
    serde_json::from_str(line).map_err(|e| format!("stdout is not an envelope ({e}): {stdout:?}"))
}

/// A bead in the scenarios' scope: `id`, labelled [`LABEL`].
pub fn labelled(id: &str) -> sc_beads::Bead {
    let mut bead = sc_beads::testing::bead(id);
    bead.labels = vec![LABEL.to_owned()];
    bead
}

/// A scenario with two open beads, `b-1` and `b-2`, in the scope [`LABEL`].
pub fn two_beads() -> Scenario {
    let mut scenario = Scenario::new("2026-10-23T10:00:00Z");
    scenario.rows = vec![labelled("b-1"), labelled("b-2")];
    scenario
}

/// A temporary repository, its scenario file and the processes that use them.
pub struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    /// A repository (with `.beads/metadata.json`) whose fake bd starts as `scenario`.
    pub fn new(scenario: &Scenario) -> Self {
        let dir = TempDir::new().expect("a temporary directory");
        let beads = dir.path().join("repo").join(".beads");
        std::fs::create_dir_all(&beads).expect("the .beads directory");
        std::fs::write(beads.join("metadata.json"), r#"{"backend":"dolt"}"#)
            .expect("metadata.json");
        let repo = Self { dir };
        repo.set_scenario(scenario);
        repo
    }

    /// The repository root.
    pub fn root(&self) -> PathBuf {
        self.dir.path().join("repo")
    }

    /// The scenario file `fake-bd` reads and writes.
    pub fn scenario_path(&self) -> PathBuf {
        self.dir.path().join("scenario.json")
    }

    /// Replaces the scenario file.
    pub fn set_scenario(&self, scenario: &Scenario) {
        scenario
            .save(&self.scenario_path())
            .expect("the scenario file");
    }

    /// The scenario as `fake-bd` last wrote it.
    pub fn scenario(&self) -> Scenario {
        Scenario::load(&self.scenario_path()).expect("the scenario file")
    }

    /// Runs `bdc --repo <repo> --bd <fake-bd> <args>` as a real process.
    pub fn bdc(&self, args: &[&str]) -> Ran {
        let root = self.root();
        let mut full = vec!["--repo", path_str(&root), "--bd", FAKE_BD];
        full.extend_from_slice(args);
        Command::new(BDC)
            .args(&full)
            .env(SCENARIO_ENV, self.scenario_path())
            .output()
            .expect("bdc starts")
            .into()
    }

    /// Runs `fake-bd <args>` itself, as `bd` would be run.
    pub fn fake_bd(&self, args: &[&str]) -> Ran {
        Command::new(FAKE_BD)
            .args(args)
            .env(SCENARIO_ENV, self.scenario_path())
            .output()
            .expect("fake-bd starts")
            .into()
    }

    /// A `BdCli` that spawns the `fake-bd` process on this repository's scenario.
    pub fn process_backend(&self) -> BdCli {
        BdCli::new_with_clock(
            FAKE_BD,
            Arc::new(ProjectLocks::new()),
            Arc::new(SystemClock),
        )
        .with_child_var(
            SCENARIO_ENV.as_ref(),
            Some(self.scenario_path().as_os_str()),
        )
    }

    /// The project `BdCli` calls are made on.
    pub fn project(&self) -> ProjectRef {
        ProjectRef::local(Some(path_str(&self.root()).to_owned()))
    }
}

/// An in-process `BdCli` over `FakeBd` seeded from `scenario`: the reference the
/// real-process runs are compared with.
pub fn in_process_backend(scenario: &Scenario) -> BdCli {
    BdCli::with_invoker(Box::new(scenario.fake_bd().expect("a valid scenario")))
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 temporary path")
}
