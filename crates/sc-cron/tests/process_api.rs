//! The exported child-process supervisor (REQ-CRON-0018) through the public
//! API: a grandchild killed at expiry and on drop, and the environment
//! allowlist. The children are shell scripts, so these run on Unix; the
//! Windows job-object path is checked by compiling it.

#![cfg(unix)]

mod common;

use std::path::Path;

use common::{Stub, read_pipe, wait_pipe};
use sc_cron::process::{ChildEnv, ChildSpec, TempCwd, supervise};

/// A script that starts a grandchild holding the `life` pipe open, says
/// `ready` and waits: the `life` reader sees EOF only when both are dead.
const TREE: &str = r#"cd "$1"; exec 3> life; ( echo up > ready; exec sleep 3600 ) & wait"#;

fn tree_spec(dir: &Path) -> ChildSpec<'_> {
    ChildSpec {
        program: Path::new("/bin/sh"),
        args: vec!["-c".into(), TREE.into(), "sh".into(), dir.into()],
        cwd: dir,
        input: b"",
        env: ChildEnv::Inherit,
    }
}

#[tokio::test]
async fn expiry_kills_the_child_and_its_grandchild() {
    let stub = Stub::new();
    let (ready, life) = (stub.fifo("ready"), stub.fifo("life"));
    let (ready, life) = (read_pipe(ready), read_pipe(life));
    let (expire, expired) = tokio::sync::oneshot::channel::<()>();
    let spec = tree_spec(stub.dir());
    let run = supervise(&spec, 1024, async {
        let _ = expired.await;
    });
    let drive = async {
        wait_pipe(ready, "the tree to start").await;
        expire.send(()).expect("supervisor waits");
        wait_pipe(life, "the whole tree to die").await;
    };
    let (captured, ()) = tokio::join!(run, drive);
    assert!(captured.expect("supervise").status.is_none(), "killed: no status");
}

#[tokio::test]
async fn dropping_the_supervisor_kills_the_child_and_its_grandchild() {
    let stub = Stub::new();
    let (ready, life) = (stub.fifo("ready"), stub.fifo("life"));
    let (ready, life) = (read_pipe(ready), read_pipe(life));
    let spec = tree_spec(stub.dir());
    let run = supervise(&spec, 1024, std::future::pending());
    let started = tokio::select! {
        _ = run => false,
        _ = wait_pipe(ready, "the tree to start") => true,
    };
    assert!(started, "the tree never ends by itself");
    wait_pipe(life, "the whole tree to die").await;
}

/// `ChildEnv::Only` keeps the named variables and sets the given ones;
/// nothing else of this process's environment reaches the child.
#[tokio::test]
async fn only_the_allowlisted_environment_reaches_the_child() {
    let cwd = TempCwd::new().expect("cwd");
    // PATH is kept (this process has it); HOME is not named, so it is gone.
    let spec = ChildSpec {
        program: Path::new("/usr/bin/env"),
        args: vec![],
        cwd: cwd.path(),
        input: b"",
        env: ChildEnv::Only {
            keep: vec!["PATH".into(), "SC_CRON_NOT_SET_ANYWHERE".into()],
            set: vec![("SC_CRON_GIVEN".into(), "1".into())],
        },
    };
    let captured = supervise(&spec, 64 * 1024, std::future::pending())
        .await
        .expect("supervise");
    let out = String::from_utf8(captured.stdout.bytes).expect("utf-8");
    let mut names: Vec<&str> = out.lines().filter_map(|l| l.split('=').next()).collect();
    names.sort_unstable();
    assert_eq!(names, ["PATH", "SC_CRON_GIVEN"], "{out}");
    // Planted negative: inheriting passes HOME (set for every test process).
    let inherit = ChildSpec {
        env: ChildEnv::Inherit,
        ..spec
    };
    let captured = supervise(&inherit, 64 * 1024, std::future::pending())
        .await
        .expect("supervise");
    let out = String::from_utf8(captured.stdout.bytes).expect("utf-8");
    assert!(out.lines().any(|l| l.starts_with("HOME=")), "{out}");
}

/// The working directory is private and removed on drop.
#[test]
fn a_temp_cwd_is_private_and_removed_on_drop() {
    use std::os::unix::fs::PermissionsExt;
    let cwd = TempCwd::new().expect("cwd");
    let path = cwd.path().to_path_buf();
    let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
    assert_eq!(mode & 0o077, 0, "private: {mode:o}");
    drop(cwd);
    assert!(!path.exists());
}
