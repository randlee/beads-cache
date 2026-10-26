//! The real composition's `bd`, switchable: [`Switched`], the invoker in
//! front of sc-bd-cli's `FakeBd` that every [`Repo`](super::repo::Repo) is
//! composed over (`compose_with_config_and_backend`: `SqliteStore` and its
//! `CacheScheduleStore`, `FlockDrainLock`, `BdCli`). Only the `bd` process
//! is replaced: `FakeBd` parses the argv the real `BdCli` built, applies it
//! to a fake bd and records it. The switch makes every bounded call answer
//! as an unreachable Dolt server, for outages of several runs.

use std::ffi::OsStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sc_bd_cli::CliInvoker;
use sc_bd_cli::bounded::BoundedError;
use sc_bd_cli::testing::FakeBd;
use sc_beads::error::BeadsError;
use sc_beads::{CliOutput, CliProbe, ProjectRef};

/// An invoker in front of `FakeBd` that can be switched to "the Dolt server
/// is unreachable" for every bounded call.
///
/// It forwards every method it does not change, `set_child_var` included
/// (the forwarding rule of `CliInvoker`, ADR-BDCLI-0004).
#[derive(Clone)]
pub struct Switched {
    inner: FakeBd,
    down: Arc<AtomicBool>,
}

impl Switched {
    /// `inner`, up; the switch is the returned handle.
    pub fn new(inner: FakeBd) -> (Self, Arc<AtomicBool>) {
        let down = Arc::new(AtomicBool::new(false));
        (
            Self {
                inner,
                down: Arc::clone(&down),
            },
            down,
        )
    }
}

impl CliInvoker for Switched {
    fn binary(&self) -> String {
        self.inner.binary()
    }
    fn client_info(&self) -> Option<CliProbe> {
        self.inner.client_info()
    }
    fn probe(&self) -> Option<CliProbe> {
        self.inner.probe()
    }
    fn run_json(
        &self,
        project: &ProjectRef,
        command: &str,
        args: &[String],
    ) -> Result<String, BeadsError> {
        self.inner.run_json(project, command, args)
    }
    fn run_raw(&self, project: &ProjectRef, args: &[&str]) -> Result<CliOutput, BeadsError> {
        self.inner.run_raw(project, args)
    }
    fn run_bounded(
        &self,
        project: &ProjectRef,
        argv: &[String],
        timeout: Duration,
    ) -> Result<CliOutput, BoundedError> {
        if self.down.load(Ordering::SeqCst) {
            return Ok(CliOutput {
                status: Some(1),
                success: false,
                stdout: String::new(),
                stderr: "Error: failed to open database: Dolt server unreachable at 127.0.0.1:1: \
                         dial tcp 127.0.0.1:1: connect: connection refused"
                    .into(),
            });
        }
        self.inner.run_bounded(project, argv, timeout)
    }
    fn set_child_var(&mut self, key: &OsStr, value: Option<&OsStr>) {
        self.inner.set_child_var(key, value);
    }
}
