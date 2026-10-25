//! Invocation functions: argv assembly, the `--json` call under the project lock,
//! raw calls, and the `--version` spawn.
//!
//! [`json_argv`] is the single assembler of a JSON invocation's argv and
//! [`json_invocation`] the single step from a cached probe to that argv. Both are
//! pure, so [`CliRunner`](crate::CliRunner) spawns, and the test-support
//! `RecordingInvoker` records, exactly the same vector.
//!
//! Every spawn sets `PATH` to [`get_extended_path`]; project invocations run in the
//! project's working directory with `BEADS_PATH` set to it. The project spawns
//! ([`spawn_json`], [`run_raw`]) block until the child exits (`Command::output()`, no
//! timeout), as in btit; see [`ProjectLocks`] on why they must not share a lock set
//! with the cache. The `--version` probe ([`probe_version_output`]) is bounded by
//! [`VERSION_PROBE_TIMEOUT`] (PE-RSH-012).

use std::env;
use std::io;
use std::process::Output;
use std::sync::atomic::Ordering;
use std::sync::PoisonError;
use std::time::{Duration, Instant};

use sc_beads::error::BeadsError;
use sc_beads::gates::capabilities_for;
use sc_beads::logging::VERBOSE_LOGGING;
use sc_beads::{log_debug, log_error, log_info};
use sc_beads::{CliClient, CliOutput, CliProbe, ProjectRef};

use crate::bounded::{run_child, BoundedError, MAX_OUTPUT_BYTES};
use crate::command::{new_command, ChildEnv};
use crate::locks::{ProjectLocks, WorkingDir};
use crate::path::extended_path_os;

/// The working directory for `project`: its `cwd`, else `BEADS_PATH`, else the
/// process current directory, else `"."`.
///
/// # Errors
///
/// [`BeadsError::Unsupported`] for a project reference that is not a local directory
/// (none exists today; `ProjectRef` is `#[non_exhaustive]`), attributed to `client`.
pub fn resolve_working_dir(project: &ProjectRef, client: CliClient) -> Result<String, BeadsError> {
    match project {
        ProjectRef::Local { cwd } => Ok(cwd
            .clone()
            .or_else(|| env::var("BEADS_PATH").ok())
            .unwrap_or_else(|| {
                env::current_dir()
                    .map_or_else(|_| ".".to_string(), |p| p.to_string_lossy().to_string())
            })),
        _ => Err(BeadsError::Unsupported {
            operation: "non-local project reference",
            client,
        }),
    }
}

/// [`resolve_working_dir`] as a normalised [`WorkingDir`], the lock key of a project.
///
/// # Errors
///
/// As [`resolve_working_dir`].
pub fn resolve_project_dir(
    project: &ProjectRef,
    client: CliClient,
) -> Result<WorkingDir, BeadsError> {
    resolve_working_dir(project, client).map(|dir| WorkingDir::new(&dir))
}

/// The argv of one JSON invocation: the words of `command` (so `"comments add"` is two
/// words), then `args`, then `--no-daemon` when `no_daemon`, then `--json`.
#[must_use]
pub fn json_argv(command: &str, args: &[String], no_daemon: bool) -> Vec<String> {
    let mut full_args: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
    full_args.extend(args.iter().cloned());
    if no_daemon {
        full_args.push("--no-daemon".to_owned());
    }
    full_args.push("--json".to_owned());
    full_args
}

/// Client (for [`resolve_working_dir`]) and argv for one JSON invocation, from the
/// cached probe.
///
/// `None` yields [`CliClient::Unknown`] and no `--no-daemon`; `Some(p)` adds
/// `--no-daemon` when `capabilities_for(p.client, p.version).supports_daemon_flag`.
#[must_use]
pub fn json_invocation(
    info: Option<&CliProbe>,
    command: &str,
    args: &[String],
) -> (CliClient, Vec<String>) {
    let client = info.map_or(CliClient::Unknown, |p| p.client);
    let no_daemon =
        info.is_some_and(|p| capabilities_for(p.client, p.version).supports_daemon_flag);
    (client, json_argv(command, args, no_daemon))
}

/// The longest a `<binary> --version` probe may run before it is killed
/// (PE-RSH-012).
///
/// `bd --version` prints a constant and touches no database, so it answers in
/// milliseconds; ten seconds leaves room for a cold start on a loaded host or a
/// first-run virus scan on Windows, while a binary that hangs (a wrapper script
/// waiting on a terminal, a broken install) can no longer stall the runner's
/// first call, which holds the probe cache's lock across the probe.
pub const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Spawns `binary` with exactly `argv` in `working_dir`, holding that directory's
/// project lock, and returns stdout.
///
/// The wait for the lock and the child are unbounded (btit's contract): do not share
/// `locks` with the cache ([`ProjectLocks`], PE-RSH-012).
///
/// Logs `[bd] <binary> <argv> | cwd: <dir>` before the spawn, `[bd] OK | <n> bytes`
/// after it, and a 500-character output preview when verbose logging is on.
///
/// # Errors
///
/// - [`BeadsError::Spawn`] (`operation: None`) when the process cannot be started.
/// - [`BeadsError::Unsupported`] when the canonical path of `working_dir` is not UTF-8,
///   so it cannot key a project lock ([`ProjectLocks::try_guard_dir`]).
/// - [`BeadsError::SchemaMigration`] when it fails with `no such column: spec_id` on stderr.
/// - [`BeadsError::CommandFailed`] for any other unsuccessful exit.
pub fn spawn_json(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &str,
    argv: &[String],
) -> Result<String, BeadsError> {
    spawn_json_in(binary, locks, working_dir, argv, &ChildEnv::default())
}

/// [`spawn_json`] with `env` applied to the child (a [`CliRunner`](crate::CliRunner)'s
/// child environment, ADR-BDCLI-0004).
pub(crate) fn spawn_json_in(
    binary: &str,
    locks: &ProjectLocks,
    working_dir: &str,
    argv: &[String],
    env: &ChildEnv,
) -> Result<String, BeadsError> {
    log_info!("[bd] {} {} | cwd: {}", binary, argv.join(" "), working_dir);

    // Acquire per-project lock to prevent concurrent Dolt access (causes SIGSEGV).
    let project_lock = locks
        .try_guard_dir(&WorkingDir::new(working_dir))
        .map_err(|e| {
            log_error!("[bd] refusing to lock {}: {}", working_dir, e);
            BeadsError::Unsupported {
                operation: "a project directory whose canonical path is not UTF-8",
                client: CliClient::Unknown,
            }
        })?;
    let _guard = project_lock.lock().unwrap_or_else(PoisonError::into_inner);

    let output = env
        .command(binary)
        .args(argv)
        .current_dir(working_dir)
        .env("PATH", extended_path_os())
        .env("BEADS_PATH", working_dir)
        .output()
        .map_err(|e| {
            log_error!("[bd] Failed to execute {}: {}", binary, e);
            BeadsError::Spawn {
                binary: binary.to_string(),
                operation: None,
                source: e,
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log_error!(
            "[bd] Command failed | status: {} | stderr: {}",
            output.status,
            stderr
        );

        // Detect schema migration failure (bd 0.49.4 migration bug)
        if stderr.contains("no such column: spec_id") {
            log_error!("[bd] Schema migration failure detected - database needs repair");
            return Err(BeadsError::SchemaMigration {
                binary: binary.to_string(),
            });
        }

        return Err(BeadsError::CommandFailed {
            binary: binary.to_string(),
            status: output.status.code(),
            status_display: output.status.to_string(),
            stderr: stderr.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    log_info!("[bd] OK | {} bytes", stdout.len());

    // Log output preview only if verbose mode is enabled
    if VERBOSE_LOGGING.load(Ordering::Relaxed) {
        let preview: String = stdout.chars().take(500).collect();
        log_debug!("[bd] Output: {}", preview);
    }

    Ok(stdout)
}

/// `<binary> <command…> <args…> [--no-daemon] --json` in `working_dir` under its
/// project lock: [`spawn_json`] over [`json_argv`].
///
/// # Errors
///
/// As [`spawn_json`].
pub fn run_json(
    binary: &str,
    no_daemon: bool,
    locks: &ProjectLocks,
    working_dir: &str,
    command: &str,
    args: &[String],
) -> Result<String, BeadsError> {
    spawn_json(
        binary,
        locks,
        working_dir,
        &json_argv(command, args, no_daemon),
    )
}

/// `<binary> <args…>` in `working_dir` with `PATH`/`BEADS_PATH`, no `--json`, no lock.
///
/// Blocks until the child exits, with no timeout (btit's contract, PE-RSH-012).
///
/// Any exit status is returned as `Ok`; the caller decides what a failure means.
///
/// # Errors
///
/// [`BeadsError::Spawn`] (`operation` = the first argument) when the process cannot be started.
pub fn run_raw(binary: &str, working_dir: &str, args: &[&str]) -> Result<CliOutput, BeadsError> {
    run_raw_in(binary, working_dir, args, &ChildEnv::default())
}

/// [`run_raw`] with `env` applied to the child (ADR-BDCLI-0004).
pub(crate) fn run_raw_in(
    binary: &str,
    working_dir: &str,
    args: &[&str],
    env: &ChildEnv,
) -> Result<CliOutput, BeadsError> {
    env.command(binary)
        .args(args)
        .current_dir(working_dir)
        .env("PATH", extended_path_os())
        .env("BEADS_PATH", working_dir)
        .output()
        .map(|output| cli_output(&output))
        .map_err(|source| BeadsError::Spawn {
            binary: binary.to_string(),
            operation: args.first().map(ToString::to_string),
            source,
        })
}

/// `<binary> --version`, run from the system temp directory (so bd never auto-migrates a
/// project as a side effect of the probe) with the extended `PATH`, stopped at
/// [`VERSION_PROBE_TIMEOUT`].
///
/// Standard input is closed. The probe leads its own process group, so a timeout
/// kills everything it started (PE-RSH-012).
///
/// # Errors
///
/// [`BeadsError::Spawn`] (`operation: "--version"`) when the process cannot be
/// started, its output cannot be read, or it runs past [`VERSION_PROBE_TIMEOUT`] (the
/// source is then an [`io::ErrorKind::TimedOut`] error).
pub fn probe_version_output(binary: &str) -> Result<CliOutput, BeadsError> {
    probe_version_output_within(binary, VERSION_PROBE_TIMEOUT)
}

/// [`probe_version_output`] stopped at `timeout` instead of [`VERSION_PROBE_TIMEOUT`].
pub(crate) fn probe_version_output_within(
    binary: &str,
    timeout: Duration,
) -> Result<CliOutput, BeadsError> {
    const OPERATION: &str = "--version";
    let mut command = new_command(binary);
    command
        .arg(OPERATION)
        .current_dir(env::temp_dir())
        .env("PATH", extended_path_os());
    let start = Instant::now();
    let limit = start.checked_add(timeout).unwrap_or(start);
    let spawn = |source: io::Error| BeadsError::Spawn {
        binary: binary.to_string(),
        operation: Some(OPERATION.to_string()),
        source,
    };
    match run_child(command, binary, Some(OPERATION), limit, MAX_OUTPUT_BYTES) {
        Ok(output) => Ok(output),
        Err(BoundedError::Failed(e)) => Err(e),
        Err(BoundedError::TimedOut { phase }) => Err(spawn(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("timed out {phase} after {timeout:?}"),
        ))),
        // btit's `BeadsError` has no wait or transport variant, so the probe keeps
        // reporting these as a failed `--version` run; the message says which it was.
        Err(BoundedError::Output(e)) => Err(spawn(e)),
        Err(BoundedError::Wait(e)) => Err(spawn(io::Error::new(
            e.kind(),
            format!("waiting for the child failed: {e}"),
        ))),
        Err(e @ BoundedError::OutputTooLarge { .. }) => Err(spawn(io::Error::new(
            io::ErrorKind::FileTooLarge,
            e.to_string(),
        ))),
        Err(e @ BoundedError::Interrupted { .. }) => Err(spawn(io::Error::new(
            io::ErrorKind::Interrupted,
            e.to_string(),
        ))),
    }
}

/// Lossy UTF-8 view of a finished process.
fn cli_output(output: &Output) -> CliOutput {
    CliOutput {
        status: output.status.code(),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_beads::CliVersion;

    /// Writes an executable test script through a `sh` child, so this
    /// multi-threaded test process never holds a write fd on it that a fork in a
    /// concurrent test could inherit and turn the exec into ETXTBSY (os error 26);
    /// a rename would not help, because it keeps the same inode.
    #[cfg(unix)]
    fn write_script(path: &std::path::Path, contents: &str) {
        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("printf '%s' \"$1\" > \"$2\" && chmod 755 \"$2\"")
            .arg("sh")
            .arg(contents)
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success(), "cannot write {}", path.display());
    }

    fn probe(client: CliClient, version: (u32, u32, u32)) -> CliProbe {
        CliProbe {
            client,
            version: Some(CliVersion::from(version)),
            raw: String::new(),
        }
    }

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn json_invocation_literal_outputs() {
        let bd = probe(CliClient::Bd, (0, 49, 6));
        let br = probe(CliClient::Br, (0, 1, 33));
        assert_eq!(
            json_invocation(Some(&bd), "ready", &[]),
            (CliClient::Bd, strings(&["ready", "--no-daemon", "--json"]))
        );
        assert_eq!(
            json_invocation(Some(&br), "ready", &[]),
            (CliClient::Br, strings(&["ready", "--json"]))
        );
        assert_eq!(
            json_invocation(None, "ready", &[]),
            (CliClient::Unknown, strings(&["ready", "--json"]))
        );
    }

    #[test]
    fn json_argv_splits_subcommand_and_appends_flags_last() {
        let args = strings(&["bd-1", "text with spaces"]);
        assert_eq!(
            json_argv("comments add", &args, false),
            strings(&["comments", "add", "bd-1", "text with spaces", "--json"])
        );
        assert_eq!(
            json_argv("comments add", &args, true),
            strings(&[
                "comments",
                "add",
                "bd-1",
                "text with spaces",
                "--no-daemon",
                "--json"
            ])
        );
    }

    #[test]
    fn run_raw_maps_spawn_failure_to_operation() {
        let dir = env::temp_dir();
        let wd = dir.to_string_lossy();
        let err = run_raw(
            "definitely-not-a-real-cli-binary-xyz",
            &wd,
            &["sync", "--no-daemon"],
        )
        .unwrap_err();
        assert!(matches!(
            &err,
            BeadsError::Spawn { binary, operation: Some(op), .. }
                if binary == "definitely-not-a-real-cli-binary-xyz" && op == "sync"
        ));
        assert!(err
            .to_string()
            .starts_with("Failed to run definitely-not-a-real-cli-binary-xyz sync: "));
    }

    #[test]
    fn spawn_json_maps_spawn_failure_like_execute_bd() {
        let dir = env::temp_dir();
        let wd = dir.to_string_lossy();
        let locks = ProjectLocks::new();
        let err = run_json(
            "definitely-not-a-real-cli-binary-xyz",
            false,
            &locks,
            &wd,
            "ready",
            &[],
        )
        .unwrap_err();
        assert!(matches!(
            &err,
            BeadsError::Spawn {
                operation: None,
                ..
            }
        ));
        assert!(err
            .to_string()
            .starts_with("Failed to execute definitely-not-a-real-cli-binary-xyz: "));
    }

    /// A `--version` that hangs is stopped at its bound, and what it started dies
    /// with it (PE-RSH-012): the script starts a `sleep` and waits for it.
    #[cfg(unix)]
    #[test]
    fn a_hanging_version_probe_is_stopped_at_its_bound() {
        let dir = env::temp_dir().join(format!("sc-bd-cli-probe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pid_file = dir.join("grandchild");
        let script = dir.join("hanging-bd");
        // Written by a child so no inherited write fd can make the exec ETXTBSY.
        write_script(
            &script,
            &format!(
                "#!/bin/sh\nsleep 30 &\necho $! > '{}'\nwait\n",
                pid_file.display()
            ),
        );

        let started = Instant::now();
        let err = probe_version_output_within(script.to_str().unwrap(), Duration::from_secs(2))
            .unwrap_err();
        let took = started.elapsed();
        assert!(
            took < Duration::from_secs(8),
            "the probe was not bounded: {took:?}"
        );
        let BeadsError::Spawn {
            operation, source, ..
        } = &err
        else {
            panic!("expected a spawn error, got {err:?}");
        };
        assert_eq!(operation.as_deref(), Some("--version"));
        assert_eq!(source.kind(), io::ErrorKind::TimedOut, "{source}");

        let grandchild = std::fs::read_to_string(&pid_file).unwrap();
        let stat = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", grandchild.trim()])
            .output()
            .unwrap();
        let stat = String::from_utf8_lossy(&stat.stdout);
        assert!(
            stat.trim().is_empty() || stat.trim_start().starts_with('Z'),
            "the probe's child {} is still running: {stat:?}",
            grandchild.trim()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_version_output_maps_spawn_failure() {
        let err = probe_version_output("definitely-not-a-real-cli-binary-xyz").unwrap_err();
        assert!(matches!(&err, BeadsError::Spawn { operation: Some(op), .. } if op == "--version"));
    }
}
