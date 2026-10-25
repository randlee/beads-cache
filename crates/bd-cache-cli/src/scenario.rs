//! The scenario file of the `fake-bd` harness (feature `fake-bd`, ADR-CLI-0001).
//!
//! A scenario is a JSON file that holds the whole state of one fake bd: its rows,
//! the beads it reports as blocked, its version and schema, a fixed clock and the
//! faults it injects. `fake-bd` loads it on every start and writes it back after a
//! call that changed a row, so a sequence of processes sees one consistent bd.
//! The format is documented in `docs/bd-cache-cli/architecture.md`.
//!
//! # Example
//!
//! ```
//! use bd_cache_cli::scenario::Scenario;
//! use sc_beads::testing::bead;
//!
//! let mut scenario = Scenario::new("2026-10-23T10:00:00Z");
//! scenario.rows.push(bead("b-1"));
//! let text = serde_json::to_string(&scenario)?;
//! let back: Scenario = serde_json::from_str(&text)?;
//! assert_eq!(back.rows.len(), 1);
//! # Ok::<(), serde_json::Error>(())
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use sc_bd_cli::CliInvoker;
use sc_bd_cli::testing::FakeBd;
use sc_beads::testing::{FakeBackend, FakeOp, Injection, ManualClock};
use sc_beads::{Bead, DepType, ProjectRef, Refusal};
use serde::{Deserialize, Serialize};

/// The environment variable that names the scenario file.
pub const SCENARIO_ENV: &str = "FAKE_BD_SCENARIO";

/// The exit code of `fake-bd` when the harness itself fails (no or bad scenario).
///
/// It is outside the codes a real bd uses (`0`, `1`, `2`, `13`), so a test can tell
/// a harness error from an answer of the fake bd.
pub const EXIT_HARNESS: i32 = 70;

/// The bd exit code of a refused guard (`--if-*` did not hold).
const EXIT_REFUSED: i32 = 13;

/// How long the fake's own deadline is. The caller's deadline is the real one: the
/// parent kills the process, so the fake never needs a short deadline of its own.
const OWN_DEADLINE: Duration = Duration::from_secs(3600);

/// The default fixed time of a scenario (the default of `ManualClock`).
const DEFAULT_NOW: &str = "2026-10-23T10:00:00Z";

/// The state of one fake bd, as the scenario file holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// The fixed UTC time (RFC 3339) every process of the scenario reads. A write
    /// stamps `updated_at` with it, so two runs of one scenario are identical.
    #[serde(default = "default_now")]
    pub now: String,
    /// The version `bd version` prints; `None` keeps the pinned release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The text `bd schema` prints; `None` keeps the pinned schema. The schema
    /// fingerprint the cache sees is the sha256 of this text, so changing it
    /// simulates a bd upgrade that changed the schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The rows of the fake bd, written back after each mutating call.
    #[serde(default)]
    pub rows: Vec<Bead>,
    /// Beads to report as blocked: each key is blocked by the beads listed. They
    /// are applied as `blocks` edges on top of `rows`, so the blocked set follows
    /// bd's own rule (a closed blocker stops blocking).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub blocked: BTreeMap<String, Vec<String>>,
    /// The faults to inject.
    #[serde(default)]
    pub faults: Faults,
}

/// The faults a scenario injects into every call.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Faults {
    /// Every call that reaches the database fails as an unreachable Dolt server
    /// (bd exit 1, "Dolt server unreachable"). `bd version` and `bd schema` still
    /// answer, as they do on a real bd.
    #[serde(default)]
    pub unavailable: bool,
    /// Every call takes this many milliseconds before it answers. The delay is
    /// real, so the caller's own timeout is what ends it.
    #[serde(default)]
    pub slow_ms: u64,
    /// Every write is refused with this exit code; the rows do not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refuse_write: Option<RefuseWrite>,
}

/// A refused write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefuseWrite {
    /// The exit code of the refused write; `13` is bd's refused guard.
    pub exit_code: i32,
}

fn default_now() -> String {
    DEFAULT_NOW.to_owned()
}

/// Why a scenario could not be read, written or applied.
#[derive(Debug)]
pub enum ScenarioError {
    /// The file could not be read or written.
    Io(io::Error),
    /// The file is not a scenario.
    Parse(serde_json::Error),
    /// The scenario names a bead that is not among its rows.
    UnknownBead(String),
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "scenario file: {e}"),
            Self::Parse(e) => write!(f, "scenario file is not valid: {e}"),
            Self::UnknownBead(id) => write!(f, "scenario blocks unknown bead {id:?}"),
        }
    }
}

impl std::error::Error for ScenarioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::UnknownBead(_) => None,
        }
    }
}

impl Scenario {
    /// An empty scenario whose clock is fixed at `now` (RFC 3339).
    #[must_use]
    pub fn new(now: &str) -> Self {
        Self {
            now: now.to_owned(),
            version: None,
            schema: None,
            rows: Vec::new(),
            blocked: BTreeMap::new(),
            faults: Faults::default(),
        }
    }

    /// Reads a scenario file.
    ///
    /// # Errors
    ///
    /// [`ScenarioError::Io`] when the file cannot be read and
    /// [`ScenarioError::Parse`] when it is not a scenario (an unknown key included).
    pub fn load(path: &Path) -> Result<Self, ScenarioError> {
        let text = fs::read_to_string(path).map_err(ScenarioError::Io)?;
        serde_json::from_str(&text).map_err(ScenarioError::Parse)
    }

    /// Writes the scenario to `path`, replacing the file whole (a temporary file in
    /// the same directory, then a rename).
    ///
    /// # Errors
    ///
    /// [`ScenarioError::Io`] when the file cannot be written, or
    /// [`ScenarioError::Parse`] when the scenario cannot be serialized.
    pub fn save(&self, path: &Path) -> Result<(), ScenarioError> {
        let text = serde_json::to_string_pretty(self).map_err(ScenarioError::Parse)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text).map_err(ScenarioError::Io)?;
        fs::rename(&tmp, path).map_err(ScenarioError::Io)
    }

    /// The fake bd this scenario describes, with its faults injected.
    ///
    /// # Errors
    ///
    /// [`ScenarioError::UnknownBead`] when `blocked` names a bead that is not a row.
    ///
    /// # Panics
    ///
    /// When `now` is not an RFC 3339 instant (a scenario bug, as for `ManualClock`).
    pub fn fake_bd(&self) -> Result<FakeBd, ScenarioError> {
        let backend = FakeBackend::new(Arc::new(ManualClock::new(&self.now)));
        backend.insert_all(self.rows.iter().cloned());
        for (id, blockers) in &self.blocked {
            for blocker in blockers {
                if !backend.add_edge(id, blocker, DepType::Blocks) {
                    return Err(ScenarioError::UnknownBead(id.clone()));
                }
            }
        }
        if self.faults.unavailable {
            backend.inject_where(
                |call| call.op() != FakeOp::Fingerprint,
                Injection::Unavailable { timed_out: false },
            );
        }
        if let Some(refuse) = self.faults.refuse_write {
            let outcome = if refuse.exit_code == EXIT_REFUSED {
                Injection::Refused(Refusal::Precondition)
            } else {
                Injection::Failed(refuse.exit_code)
            };
            backend.inject_where(|call| call.op().is_write(), outcome);
        }
        let fake = FakeBd::over(backend);
        if let Some(version) = &self.version {
            fake.set_bd_version(version.clone());
        }
        if let Some(schema) = &self.schema {
            fake.set_schema(schema.clone());
        }
        Ok(fake)
    }
}

/// What one `fake-bd` process prints and exits with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    /// The process exit code.
    pub exit: i32,
    /// What goes to stdout.
    pub stdout: String,
    /// What goes to stderr.
    pub stderr: String,
}

/// Serves one `bd` invocation (`argv` without the program name) from the scenario
/// file at `path`: sleeps for the slow fault, runs the call on the fake bd and
/// writes the scenario back when the call changed a row.
///
/// `--version` is answered as `version`. A scenario that cannot be read or applied
/// is [`EXIT_HARNESS`] with the reason on stderr.
#[must_use]
pub fn serve(argv: &[String], path: &Path) -> Served {
    match serve_checked(argv, path) {
        Ok(served) => served,
        Err(e) => Served {
            exit: EXIT_HARNESS,
            stdout: String::new(),
            stderr: format!("fake-bd: {e}\n"),
        },
    }
}

fn serve_checked(argv: &[String], path: &Path) -> Result<Served, ScenarioError> {
    let mut scenario = Scenario::load(path)?;
    let fake = scenario.fake_bd()?;
    let before = fake.backend().rows();
    if scenario.faults.slow_ms > 0 {
        std::thread::sleep(Duration::from_millis(scenario.faults.slow_ms));
    }
    let mut argv = argv.to_vec();
    if let Some(first) = argv.first_mut().filter(|w| w.as_str() == "--version") {
        "version".clone_into(first);
    }
    let served = match fake.run_bounded(&ProjectRef::local(None), &argv, OWN_DEADLINE) {
        Ok(out) => Served {
            exit: out.status.unwrap_or(1),
            stdout: out.stdout,
            stderr: out.stderr,
        },
        Err(e) => Served {
            exit: 1,
            stdout: String::new(),
            stderr: format!("Error: {e}\n"),
        },
    };
    let after = fake.backend().rows();
    if after != before {
        scenario.rows = after;
        scenario.save(path)?;
    }
    Ok(served)
}
