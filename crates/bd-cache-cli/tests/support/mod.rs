//! The real-process harness of contract §8 "Testing", shared by the integration tests.
//!
//! [`TestRepo`] is a temporary repository with a `.beads/metadata.json`
//! (database [`DATABASE`], project [`PROJECT`]), a `.beads/bd-cache.toml`
//! naming the scope [`label`], and a scenario file. It runs
//! `bdc --repo <repo> --bd <fake-bd>` as a real process in a [`hermetic`]
//! environment, hands `BdCli` the
//! same `fake-bd` binary, and parses what comes back.
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
pub fn label() -> String {
    bd_cache_model::legacy_label(&bd_cache_model::ScopeId::new("x").expect("a scope name"))
}

/// The `dolt_database` of every test repository's `metadata.json`.
pub const DATABASE: &str = "beads";

/// The `project_id` of every test repository's `metadata.json`.
pub const PROJECT: &str = "proj-1";

/// The `metadata.json` of a test repository.
pub fn metadata(database: &str, project: &str) -> String {
    format!(r#"{{"dolt_database": "{database}", "project_id": "{project}"}}"#)
}

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

/// The `PATH` every test process gets unless the test sets its own: system
/// directories only, never the host's, so no installed `bd` and no
/// developer's `PATH` order can change what a test sees.
pub fn system_path() -> String {
    if cfg!(windows) {
        let root = std::env::var("SYSTEMROOT").unwrap_or_else(|_| r"C:\Windows".to_owned());
        format!(r"{root}\System32;{root}")
    } else {
        "/usr/bin:/bin".to_owned()
    }
}

/// A [`Command`] for `program` with a hermetic environment (QA RCR-002 item 3):
/// the host environment is cleared, so no `BEADS_DIR`, `BEADS_ACTOR`,
/// `BD_CACHE__*` or `PATH` of the machine running the tests reaches the
/// child. Only `PATH` ([`system_path`]) and what a process needs to start
/// are set (`SYSTEMROOT`, `SYSTEMDRIVE`, `WINDIR`, `TEMP` and `TMP` on
/// Windows, `TMPDIR` elsewhere); a test adds the rest explicitly.
pub fn hermetic(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    cmd.env_clear().env("PATH", system_path());
    let keep: &[&str] = if cfg!(windows) {
        &["SYSTEMROOT", "SYSTEMDRIVE", "WINDIR", "TEMP", "TMP"]
    } else {
        &["TMPDIR"]
    };
    for key in keep {
        if let Some(v) = std::env::var_os(key) {
            cmd.env(key, v);
        }
    }
    cmd
}

/// Runs `binary` with `args` and no scenario.
pub fn run_binary(binary: &str, args: &[&str]) -> Ran {
    hermetic(binary)
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

/// A bead in the scenarios' scope: `id`, labelled [`label`].
pub fn labelled(id: &str) -> sc_beads::Bead {
    let mut bead = sc_beads::testing::bead(id);
    bead.labels = vec![label()];
    bead
}

/// A scenario with two open beads, `b-1` and `b-2`, in the scope [`label`].
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
        std::fs::write(beads.join("metadata.json"), metadata(DATABASE, PROJECT))
            .expect("metadata.json");
        std::fs::write(beads.join("bd-cache.toml"), "scopes = [\"x\"]\n").expect("bd-cache.toml");
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
        hermetic(BDC)
            .args(&full)
            .env(SCENARIO_ENV, self.scenario_path())
            .output()
            .expect("bdc starts")
            .into()
    }

    /// Runs `bdc --repo <repo> --bd <fake-bd> <args>` and parses its one envelope.
    ///
    /// Returns the exit code and the envelope as JSON (`{version, ok, data, error}`).
    pub fn json(&self, args: &[&str]) -> (i32, Value) {
        let ran = self.bdc(args);
        envelope_json(&ran)
    }

    /// Runs `bdc <args>` in `cwd` in a [`hermetic`] environment with `envs`
    /// added (and `FAKE_BD_SCENARIO`), and nothing else: no `--repo` and no
    /// `--bd` unless `args` gives them.
    pub fn bdc_raw(&self, cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> Ran {
        let mut cmd = hermetic(BDC);
        cmd.args(args)
            .current_dir(cwd)
            .env(SCENARIO_ENV, self.scenario_path());
        for (k, v) in envs {
            cmd.env(k, v);
        }
        cmd.output().expect("bdc starts").into()
    }

    /// Edits the scenario file in place.
    pub fn edit(&self, f: impl FnOnce(&mut Scenario)) {
        let mut scenario = self.scenario();
        f(&mut scenario);
        self.set_scenario(&scenario);
    }

    /// The release file of a gated scenario ([`Faults::gate_file`]): absent
    /// until [`TestRepo::release_gate`] creates it.
    ///
    /// [`Faults::gate_file`]: bd_cache_cli::scenario::Faults::gate_file
    pub fn gate_file(&self) -> PathBuf {
        self.dir.path().join("bd-gate")
    }

    /// Opens the gate, so a fake-bd still blocked at it answers and exits.
    pub fn release_gate(&self) {
        std::fs::write(self.gate_file(), b"").expect("the gate file");
    }

    /// Runs `fake-bd <args>` itself, as `bd` would be run.
    pub fn fake_bd(&self, args: &[&str]) -> Ran {
        hermetic(FAKE_BD)
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

/// The exit code and the one envelope of a run, as JSON; panics (fails the test)
/// when stdout is not exactly one envelope.
pub fn envelope_json(ran: &Ran) -> (i32, Value) {
    parse_envelope(&ran.stdout).unwrap_or_else(|e| panic!("{e}; stderr: {}", ran.stderr));
    let value: Value = serde_json::from_str(ran.stdout.trim_end()).expect("JSON");
    (ran.exit, value)
}

/// A path as UTF-8 text (every temporary path of the tests is).
pub fn path_str(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 temporary path")
}

/// Writes an executable script at `path` with `contents`, through a
/// `/bin/sh` child (`printf … > f && chmod 755 f`).
///
/// This test binary runs its tests on several threads. A file it wrote
/// itself (`fs::write`, `fs::copy`) can still have its write fd open in
/// a child forked by a concurrent test, and exec'ing it then fails with
/// ETXTBSY (os error 26); a rename would not help, as it keeps the inode.
/// A child writes it instead, so this process never holds that fd
/// (sc-bd-cli's `run.rs` tests use the same remedy).
#[cfg(unix)]
pub fn write_script(path: &Path, contents: &str) {
    let status = Command::new("/bin/sh")
        .arg("-c")
        .arg("printf '%s' \"$1\" > \"$2\" && chmod 755 \"$2\"")
        .arg("sh")
        .arg(contents)
        .arg(path)
        .status()
        .expect("/bin/sh starts");
    assert!(status.success(), "cannot write {}", path.display());
}

/// Copies the executable `from` to `to`, to be exec'd from there.
///
/// On Unix through a `cp` child, for the reason [`write_script`] gives; on
/// Windows, which has no ETXTBSY, with `fs::copy`.
pub fn copy_executable(from: &Path, to: &Path) {
    #[cfg(unix)]
    {
        let status = Command::new("cp")
            .arg(from)
            .arg(to)
            .status()
            .expect("cp starts");
        assert!(
            status.success(),
            "cannot copy {} to {}",
            from.display(),
            to.display()
        );
    }
    #[cfg(not(unix))]
    {
        std::fs::copy(from, to).expect("the copy");
    }
}
