//! `fake-python`: a stand-in interpreter for the post-sync hook and the
//! Python sync row in real-process tests, on every OS (Windows included,
//! where the shell-script stubs of the Unix tests cannot run).
//!
//! It answers the argv the runners give an interpreter:
//!
//! - `-I -c ...` (the import check): exits 0;
//! - `-I -m bd_cache_rs.run post_sync <module:function>` (the hook): reads
//!   its request, then acts on `$FAKE_PY_DIR/hook.mode`: absent, an ok
//!   answer; `grandchild`, it starts a grandchild of itself, writes the
//!   grandchild's pid to `gc.pid` and its own to `child.pid`, and waits;
//! - `-I -m <module>` (the sync row, sc-cron's hook protocol): answers
//!   `cron.stop` with `continue`; on `cron.tick` acts on
//!   `$FAKE_PY_DIR/tick.mode` as above (absent: exits 0);
//! - `grandchild <pid file>`: writes its pid and sleeps.
//!
//! `FAKE_PY_DIR` reaches it through the runner's `--env FAKE_PY_DIR`. On
//! Windows it ignores Ctrl-C and Ctrl-Break, so only a job-object kill (or
//! the end of its sleep) ends it: a test of the kill then shows the kill.
//!
//! The binary builds only with the `fake-bd` feature and is never part of a
//! release build (ADR-CLI-0001).

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

/// How long a hanging child sleeps: far longer than any test waits.
const HANG: Duration = Duration::from_secs(300);

fn dir() -> PathBuf {
    std::env::var_os("FAKE_PY_DIR").map_or_else(std::env::temp_dir, PathBuf::from)
}

fn mode(name: &str) -> String {
    std::fs::read_to_string(dir().join(name)).unwrap_or_default()
}

/// Writes `pid` to `path` whole (a temp file renamed into place).
fn write_pid(path: &Path, pid: u32) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, format!("{pid}\n")).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Starts a grandchild, records both pids, and waits.
fn hang_with_a_grandchild(own: &str) -> ExitCode {
    let d = dir();
    let gc = d.join("gc.pid");
    let Ok(exe) = std::env::current_exe() else {
        return ExitCode::from(2);
    };
    let spawned = std::process::Command::new(exe)
        .arg("grandchild")
        .arg(&gc)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if spawned.is_err() {
        return ExitCode::from(2);
    }
    while !gc.exists() {
        std::thread::sleep(Duration::from_millis(10));
    }
    write_pid(&d.join(own), std::process::id());
    std::thread::sleep(HANG);
    ExitCode::SUCCESS
}

#[cfg(windows)]
mod console {
    /// The console control handler type of `SetConsoleCtrlHandler`.
    type Handler = unsafe extern "system" fn(u32) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<Handler>, add: i32) -> i32;
    }

    /// Handles (ignores) every console control event.
    unsafe extern "system" fn ignore(_event: u32) -> i32 {
        1
    }

    /// Ignores Ctrl-C and Ctrl-Break for the rest of the process.
    pub(crate) fn ignore_ctrl() {
        // SAFETY: `ignore` is a valid `extern "system"` handler with the
        // signature the API requires, it lives for the whole process, and
        // the call has no other precondition.
        let _ = unsafe { SetConsoleCtrlHandler(Some(ignore), 1) };
    }
}

fn main() -> ExitCode {
    #[cfg(windows)]
    console::ignore_ctrl();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["grandchild", pid_file] => {
            write_pid(Path::new(pid_file), std::process::id());
            std::thread::sleep(HANG);
            ExitCode::SUCCESS
        }
        [_, "-c", ..] => ExitCode::SUCCESS,
        [_, "-m", "bd_cache_rs.run", "post_sync", _] => {
            let _ = std::io::stdin().read_to_end(&mut Vec::new());
            if mode("hook.mode").trim() == "grandchild" {
                return hang_with_a_grandchild("child.pid");
            }
            let _ = writeln!(
                std::io::stdout(),
                r#"{{"protocol":"bd-cache.run.v1","ok":true,"result":null}}"#
            );
            ExitCode::SUCCESS
        }
        [_, "-m", _] => {
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            if input.contains("\"cron.stop\"") {
                let _ = writeln!(std::io::stdout(), r#"{{"action": "continue"}}"#);
                return ExitCode::SUCCESS;
            }
            if mode("tick.mode").trim() == "grandchild" {
                return hang_with_a_grandchild("child.pid");
            }
            ExitCode::SUCCESS
        }
        _ => {
            let _ = writeln!(std::io::stderr(), "fake-python: unknown argv {args:?}");
            ExitCode::from(2)
        }
    }
}
