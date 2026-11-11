//! Whole-scenario watchdog: killing the child owns all service Rust threads.
#![expect(clippy::expect_used, reason = "test fixture assertions")]
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
const WAIT: Duration = Duration::from_secs(10);
struct FixtureChild(Child);

impl Drop for FixtureChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn run_fixture_child(name: &str, deadline: Duration) -> Result<(), String> {
    let marker = tempfile::tempdir().map_err(|_error| "fixture marker directory failed")?;
    let done = marker.path().join("completed");
    let mut child = FixtureChild(
        Command::new(std::env::current_exe().map_err(|_error| "test executable unavailable")?)
            .args(["--exact", name, "--nocapture"])
            .env("SC_SERVICE_FIXTURE", name)
            .env("SC_SERVICE_FIXTURE_DONE", &done)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_error| "fixture child could not start")?,
    );
    let until = Instant::now() + deadline;
    loop {
        match child
            .0
            .try_wait()
            .map_err(|_error| "fixture status inspection failed")?
        {
            Some(status) => {
                if !status.success() {
                    return Err("fixture child failed".into());
                }
                if std::fs::read(&done).ok().as_deref() != Some(b"completed") {
                    return Err("fixture did not execute its exact test".into());
                }
                return Ok(());
            }
            None if Instant::now() >= until => {
                child
                    .0
                    .kill()
                    .map_err(|_error| "fixture timeout: child kill failed")?;
                child
                    .0
                    .wait()
                    .map_err(|_error| "fixture timeout: child reap failed")?;
                return Err("fixture timed out; child killed and reaped".into());
            }
            None => std::thread::sleep(Duration::from_millis(10)), // allow: test polls its owned child
        }
    }
}

pub(crate) fn bounded_fixture(name: &str, scenario: fn()) {
    if std::env::var("SC_SERVICE_FIXTURE").ok().as_deref() == Some(name) {
        scenario();
        let done = std::env::var_os("SC_SERVICE_FIXTURE_DONE").expect("completion marker");
        std::fs::write(done, b"completed").expect("fixture completion");
    } else {
        run_fixture_child(name, WAIT).expect("bounded service fixture");
    }
}
