//! [`BdCli`]: the backend for the Go `bd` CLI (steveyegge/beads), and for an
//! unrecognized client, which shares every `_ =>` arm with bd today.
//!
//! `BdCli` implements btit's three traits as btit-bd does, and
//! [`CacheBackend`](sc_beads::CacheBackend) for the cache (module [`cache`](crate::cache)).
//! The cache calls need the time: a `BdCli` holds the `Arc<dyn Clock>` it is constructed
//! with ([`BdCli::new_with_clock`], [`BdCli::with_clock`]) and turns each call's
//! [`Deadline`](sc_beads::Deadline) into the time the child may run. btit's constructors
//! use the system clock.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use crate::locks::ProjectLocks;
use crate::ops;
use crate::run::resolve_working_dir;
use crate::runner::{CliInvoker, CliRunner};
use sc_beads::backend::{BeadsBackend, CliBackend, DoltOperations};
use sc_beads::error::BeadsError;
use sc_beads::{
    BackendCapabilities, BdRawIssue, CliClient, CliOutput, CliProbe, CliVersion, Clock,
    CreatePayload, DoltOpResult, ListQuery, ProjectRef, RelationType, ReleaseSource, SystemClock,
    UpdatePayload,
};

/// GitHub location of `bd` releases (`updates.rs:287,291`), used by the app's
/// `check_bd_cli_update` selection (b-8).
pub const BD_RELEASE_SOURCE: ReleaseSource = ReleaseSource {
    api_url: "https://api.github.com/repos/steveyegge/beads/releases/latest",
    releases_url: "https://github.com/steveyegge/beads/releases",
};

/// The Go `bd` CLI (also used for an unrecognized client, which shares bd's defaults
/// today), over a `Box<dyn CliInvoker>`: a [`CliRunner`] in production, a scripted
/// `RecordingInvoker` or a `FakeBd` in tests (feature `test-support`).
///
/// # Bounded and unbounded calls
///
/// The [`CacheBackend`](sc_beads::CacheBackend) methods are deadline-bounded: they wait
/// for the project lock by polling, kill the `bd` process group at the deadline, and
/// give up. btit's [`BeadsBackend`], [`CliBackend`] and [`DoltOperations`] methods
/// keep btit's contract: they block, with no timeout, until `bd` exits, holding the
/// project lock for a JSON call the whole time (PE-RSH-012). One hung btit call would
/// make every cache call on that project time out waiting for the lock, so **a
/// `BdCli` used for btit's calls must not share its [`ProjectLocks`] with the
/// `BdCli` the cache uses**: construct the cache's with its own lock set. Only the
/// `--version` probe of the btit calls is bounded
/// ([`VERSION_PROBE_TIMEOUT`](crate::run::VERSION_PROBE_TIMEOUT)).
pub struct BdCli {
    pub(crate) inv: Box<dyn CliInvoker>,
    /// The cache's clock: reads the remaining time of each cache call's deadline.
    pub(crate) clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for BdCli {
    /// `CliInvoker` is not `Debug` (it must stay object-safe across a test-support
    /// scripted implementation too), so this reports only the configured binary.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BdCli")
            .field("binary", &self.inv.binary())
            .finish_non_exhaustive()
    }
}

impl BdCli {
    /// A `BdCli` wrapping a fresh [`CliRunner`] for `binary`, which probes
    /// `--version` lazily on first use.
    #[must_use]
    pub fn new(binary: impl Into<String>, locks: Arc<ProjectLocks>) -> Self {
        Self {
            inv: Box::new(CliRunner::new(binary, locks)),
            clock: Arc::new(SystemClock),
        }
    }

    /// A `BdCli` for the cache: as [`new`](Self::new), reading time from the `Arc<dyn Clock>`
    /// the cache and its store share.
    #[must_use]
    pub fn new_with_clock(
        binary: impl Into<String>,
        locks: Arc<ProjectLocks>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self::new(binary, locks).with_clock(clock)
    }

    /// This `BdCli` reading time from `clock` instead of the system clock.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// This `BdCli` with `key` set to `value` (`Some`) or removed (`None`) in the
    /// environment of every `bd` it spawns ([`CliInvoker::set_child_var`],
    /// ADR-BDCLI-0004): a composition that resolved the beads directory from an explicit
    /// `BEADS_DIR` gives `bd` the same value. A test double that spawns nothing ignores it.
    #[must_use]
    pub fn with_child_var(mut self, key: &OsStr, value: Option<&OsStr>) -> Self {
        self.inv.set_child_var(key, value);
        self
    }

    /// A `BdCli` whose probe cache is pre-seeded with `probe`, so no second
    /// `--version` is spawned (b-7's factory and `check_bd_compatibility`'s rebuild).
    #[must_use]
    pub fn with_seeded_probe(
        binary: impl Into<String>,
        locks: Arc<ProjectLocks>,
        probe: CliProbe,
    ) -> Self {
        Self {
            inv: Box::new(CliRunner::with_probe(binary, locks, probe)),
            clock: Arc::new(SystemClock),
        }
    }

    /// A `BdCli` over any [`CliInvoker`], for tests (e.g. a scripted
    /// `RecordingInvoker`); the app's tests (b-8) use this too.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn with_invoker(inv: Box<dyn CliInvoker>) -> Self {
        Self {
            inv,
            clock: Arc::new(SystemClock),
        }
    }
}

impl BeadsBackend for BdCli {
    fn project_uses_dolt(&self, project: &ProjectRef) -> bool {
        // One `client_info()` read feeds both the client and the version tuple: an
        // unparsed or missing binary is re-probed on every read, and this runs on every
        // poll tick (QA-1 RSH-001, b-7).
        let info = self.inv.client_info();
        let client = info.as_ref().map_or(CliClient::Unknown, |p| p.client);
        let tuple = info.and_then(|p| p.version.map(|v| (p.client, v.major, v.minor, v.patch)));
        match resolve_working_dir(project, client) {
            Ok(wd) => crate::dolt::project_uses_dolt_for(tuple, &Path::new(&wd).join(".beads")),
            Err(_) => false,
        }
    }

    fn list(&self, project: &ProjectRef, query: &ListQuery) -> Result<Vec<BdRawIssue>, BeadsError> {
        ops::list(self.inv.as_ref(), project, query)
    }

    fn ready(&self, project: &ProjectRef) -> Result<Vec<BdRawIssue>, BeadsError> {
        ops::ready(self.inv.as_ref(), project)
    }

    fn status(&self, project: &ProjectRef) -> Result<serde_json::Value, BeadsError> {
        ops::status(self.inv.as_ref(), project)
    }

    fn show(&self, project: &ProjectRef, id: &str) -> Result<Option<BdRawIssue>, BeadsError> {
        ops::show(self.inv.as_ref(), project, id)
    }

    fn create(
        &self,
        project: &ProjectRef,
        payload: &CreatePayload,
    ) -> Result<BdRawIssue, BeadsError> {
        ops::create(self.inv.as_ref(), project, payload)
    }

    fn update(
        &self,
        project: &ProjectRef,
        id: &str,
        updates: &UpdatePayload,
    ) -> Result<Option<BdRawIssue>, BeadsError> {
        ops::update(self.inv.as_ref(), project, id, updates)
    }

    fn close(&self, project: &ProjectRef, id: &str) -> Result<serde_json::Value, BeadsError> {
        ops::close(self.inv.as_ref(), project, id, false)
    }

    fn search(&self, project: &ProjectRef, query: &str) -> Result<Vec<BdRawIssue>, BeadsError> {
        ops::search(self.inv.as_ref(), project, query)
    }

    fn label_add(&self, project: &ProjectRef, id: &str, label: &str) -> Result<(), BeadsError> {
        ops::label_add(self.inv.as_ref(), project, id, label)
    }

    fn label_remove(&self, project: &ProjectRef, id: &str, label: &str) -> Result<(), BeadsError> {
        ops::label_remove(self.inv.as_ref(), project, id, label)
    }

    fn delete(&self, project: &ProjectRef, id: &str) -> Result<(), BeadsError> {
        ops::delete(
            self.inv.as_ref(),
            project,
            id,
            self.capabilities().supports_delete_hard_flag,
        )
    }

    fn comment_add(&self, project: &ProjectRef, id: &str, content: &str) -> Result<(), BeadsError> {
        ops::comment_add(self.inv.as_ref(), project, id, content)
    }

    fn dep_add(
        &self,
        project: &ProjectRef,
        issue_id: &str,
        depends_on_id: &str,
        relation_type: Option<&str>,
    ) -> Result<(), BeadsError> {
        ops::dep_add(
            self.inv.as_ref(),
            project,
            issue_id,
            depends_on_id,
            relation_type,
        )
    }

    fn dep_remove(
        &self,
        project: &ProjectRef,
        issue_id: &str,
        depends_on_id: &str,
    ) -> Result<(), BeadsError> {
        ops::dep_remove(self.inv.as_ref(), project, issue_id, depends_on_id)
    }

    fn relation_types(&self) -> Vec<RelationType> {
        ops::relation_types(self.client())
    }

    fn sync(&self, project: &ProjectRef) -> Result<(), BeadsError> {
        ops::sync(
            self.inv.as_ref(),
            project,
            self.capabilities().supports_daemon_flag,
        )
    }

    fn cli(&self) -> Option<&dyn CliBackend> {
        Some(self)
    }

    fn dolt(&self) -> Option<&dyn DoltOperations> {
        Some(self)
    }
}

impl CliBackend for BdCli {
    fn binary(&self) -> String {
        self.inv.binary()
    }

    fn probe(&self) -> Option<CliProbe> {
        self.inv.probe()
    }

    fn client(&self) -> CliClient {
        self.inv.client()
    }

    fn version(&self) -> Option<CliVersion> {
        self.inv.version()
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.inv.capabilities()
    }

    fn run_raw(&self, project: &ProjectRef, args: &[&str]) -> Result<CliOutput, BeadsError> {
        self.inv.run_raw(project, args)
    }

    fn release_source(&self) -> ReleaseSource {
        BD_RELEASE_SOURCE
    }
}

impl DoltOperations for BdCli {
    fn doctor_fix(&self, project: &ProjectRef) -> Result<DoltOpResult, BeadsError> {
        self.inv
            .run_raw(project, &crate::dolt::doctor_fix_args())
            .map(crate::dolt::to_dolt_result)
    }

    fn migrate_to_dolt(&self, project: &ProjectRef) -> Result<DoltOpResult, BeadsError> {
        self.inv
            .run_raw(project, &crate::dolt::migrate_to_dolt_args())
            .map(crate::dolt::to_dolt_result)
    }

    fn init(&self, project: &ProjectRef, prefix: &str) -> Result<DoltOpResult, BeadsError> {
        self.inv
            .run_raw(project, &crate::dolt::init_args(prefix))
            .map(crate::dolt::to_dolt_result)
    }

    fn import_jsonl(&self, project: &ProjectRef, file: &Path) -> Result<DoltOpResult, BeadsError> {
        let file = file.to_string_lossy();
        self.inv
            .run_raw(project, &crate::dolt::import_args(&file))
            .map(crate::dolt::to_dolt_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::RecordingInvoker;

    // The wrapper test moved from btit-app spawned `bd --version` through a fresh
    // `CliRunner`; b-10 (B10) replaced it with
    // `dolt::tests::project_uses_dolt_for_is_false_for_dir_without_beads_layout`,
    // which calls the pure core. This test keeps the wrapper covered over a scripted
    // probe, so it spawns nothing either.

    #[test]
    fn project_uses_dolt_wrapper_derives_beads_dir_from_scripted_probe() {
        let project = std::env::temp_dir().join(format!(
            "beads_wrapper_dolt_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let beads = project.join(".beads");
        std::fs::create_dir_all(&beads).unwrap_or_else(|e| panic!("create_dir_all failed: {e}"));
        let project_ref = ProjectRef::local(Some(project.to_string_lossy().to_string()));
        let bd_1 = CliProbe {
            client: CliClient::Bd,
            version: Some((1, 0, 4).into()),
            raw: "bd version 1.0.4".to_string(),
        };
        let cli = BdCli::with_invoker(Box::new(RecordingInvoker::new(Some(bd_1))));

        assert!(!cli.project_uses_dolt(&project_ref), "no metadata.json");
        std::fs::write(beads.join("metadata.json"), r#"{"backend":"dolt"}"#)
            .unwrap_or_else(|e| panic!("write failed: {e}"));
        assert!(cli.project_uses_dolt(&project_ref), "bd 1.x metadata rule");

        let _ = std::fs::remove_dir_all(&project);
    }
}
