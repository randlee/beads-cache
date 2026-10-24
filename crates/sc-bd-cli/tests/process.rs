//! Real-process tests of the deadline-bounded run (NFR-BDC-0004, REQ-BDC-0006).
//!
//! The test binary is its own fake `bd`: this target has no libtest harness
//! (`harness = false`), and when the environment variable `BEADS_PATH` (which
//! `sc-bd-cli` sets to the project's working directory for every child) names a
//! directory called `sc-bd-cli-child-<mode>`, the binary behaves as a `bd` in that
//! mode and exits. Otherwise it runs the tests below, each pointing `BdCli` at this
//! very executable with a working directory that selects the mode. This needs no
//! shell, no script file and no installed `bd`, so it runs the same on every
//! platform CI covers.
//!
//! Modes: `ok` (answers `version`, `schema`, `list`), `refuse` (exit 13), `down`
//! (exit 1, "connection refused"), `slow` (runs 30 s, logging that it is alive),
//! `flood` (writes far more than a pipe holds), `stdin` (reads standard input to the
//! end) and `echo` (prints its argv).

use std::fs;
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sc_bd_cli::bounded::{run_bounded, BoundedError};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{
    BeadsCacheError, CacheBackend, Clock, Deadline, Guards, Patch, ProjectRef, Refusal,
    ScopeFilter, SystemClock, BD_SCHEMA_SHA256, BD_VERSION,
};

const CHILD_PREFIX: &str = "sc-bd-cli-child-";
const SCHEMA: &str = include_str!("fixtures/bd-schema-1.3.1.json");

/// What `slow` and the readers get: far longer than any test waits.
const SLOW_CHILD_SECS: u64 = 30;
/// Bytes `flood` writes to stdout and stderr: several times a 64 KiB pipe buffer.
const FLOOD_STDOUT: usize = 4 * 1024 * 1024;
const FLOOD_STDERR: usize = 1024 * 1024;
/// The slowest a bounded call may take to give up in these tests; the children run
/// for 30 s, so a call that waited for one fails this bound by a wide margin.
const GIVE_UP_WITHIN: Duration = Duration::from_secs(10);

fn main() -> ExitCode {
    match child_mode() {
        Some(mode) => child(&mode),
        None => run_tests(),
    }
}

// ---- the fake bd ----------------------------------------------------------------

fn child_mode() -> Option<String> {
    let dir = std::env::var_os("BEADS_PATH")?;
    let name = Path::new(&dir).file_name()?.to_str()?;
    name.strip_prefix(CHILD_PREFIX).map(str::to_owned)
}

fn child(mode: &str) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str);
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    match (mode, first) {
        ("ok", Some("version")) => {
            let _ = writeln!(out, "bd version {BD_VERSION} (dev)");
            ExitCode::SUCCESS
        }
        ("ok", Some("schema")) => {
            let _ = write!(out, "{SCHEMA}");
            ExitCode::SUCCESS
        }
        ("ok", Some("list")) => {
            let _ = writeln!(out, "[]");
            ExitCode::SUCCESS
        }
        ("ok", _) => ExitCode::SUCCESS,
        ("refuse", _) => {
            let _ = writeln!(err, "Error updating: status mismatch");
            ExitCode::from(13)
        }
        ("down", _) => {
            let _ = writeln!(
                err,
                "Error: failed to open database: Dolt server unreachable at 127.0.0.1:1: dial tcp 127.0.0.1:1: connect: connection refused"
            );
            ExitCode::from(1)
        }
        ("slow", _) => {
            let alive = std::env::var_os("BEADS_PATH")
                .map(|d| Path::new(&d).join("alive"))
                .unwrap_or_default();
            for _ in 0..SLOW_CHILD_SECS * 10 {
                if let Ok(mut f) = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&alive)
                {
                    let _ = writeln!(f, "alive");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            ExitCode::SUCCESS
        }
        ("flood", _) => {
            let chunk = vec![b'x'; 64 * 1024];
            // stdout and stderr are written in turn: a parent that drained one pipe
            // only would stall here once the other fills.
            let mut sent = (0, 0);
            while sent.0 < FLOOD_STDOUT || sent.1 < FLOOD_STDERR {
                if sent.0 < FLOOD_STDOUT {
                    let _ = out.write_all(&chunk);
                    sent.0 += chunk.len();
                }
                if sent.1 < FLOOD_STDERR {
                    let _ = err.write_all(&chunk);
                    sent.1 += chunk.len();
                }
            }
            let _ = out.flush();
            ExitCode::SUCCESS
        }
        ("stdin", _) => {
            let mut sink = Vec::new();
            let _ = std::io::stdin().read_to_end(&mut sink);
            let _ = write!(out, "eof after {} bytes", sink.len());
            ExitCode::SUCCESS
        }
        ("echo", _) => {
            for a in &args {
                let _ = writeln!(out, "arg:{a}");
            }
            let beads = std::env::var("BEADS_PATH").unwrap_or_default();
            let _ = writeln!(out, "beads_path:{beads}");
            ExitCode::SUCCESS
        }
        _ => {
            let _ = writeln!(err, "unknown fake-bd mode {mode:?}");
            ExitCode::from(99)
        }
    }
}

// ---- the tests --------------------------------------------------------------------

type TestResult = Result<(), String>;
type Test = fn(&Env) -> TestResult;

struct Env {
    root: PathBuf,
    exe: String,
}

impl Env {
    /// The working directory that selects `mode`, created on first use.
    fn dir(&self, mode: &str) -> Result<String, String> {
        let dir = self.root.join(format!("{CHILD_PREFIX}{mode}"));
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        Ok(dir.to_string_lossy().into_owned())
    }

    fn project(&self, mode: &str) -> Result<ProjectRef, String> {
        Ok(ProjectRef::local(Some(self.dir(mode)?)))
    }

    fn cli(&self) -> BdCli {
        BdCli::new_with_clock(
            &self.exe,
            Arc::new(ProjectLocks::new()),
            Arc::new(SystemClock) as Arc<dyn Clock>,
        )
    }
}

fn require(cond: bool, what: impl Into<String>) -> TestResult {
    if cond {
        Ok(())
    } else {
        Err(what.into())
    }
}

fn after(ms: u64) -> Deadline {
    Deadline::after(&SystemClock, Duration::from_millis(ms))
}

fn scope() -> ScopeFilter {
    ScopeFilter::Label("x".into())
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| (*w).to_owned()).collect()
}

fn timeout_kills_a_running_child(env: &Env) -> TestResult {
    let project = env.project("slow")?;
    let alive = Path::new(&env.dir("slow")?).join("alive");
    let started = Instant::now();
    let r = env.cli().list_scope(&project, &scope(), None, after(500));
    let took = started.elapsed();
    require(
        matches!(
            r,
            Err(BeadsCacheError::Unavailable {
                timed_out: true,
                ..
            })
        ),
        format!("expected a timeout, got {r:?}"),
    )?;
    require(
        took < GIVE_UP_WITHIN,
        format!("gave up after {took:?}, the child runs {SLOW_CHILD_SECS} s"),
    )?;
    // A killed child stops writing; one left running would keep appending.
    std::thread::sleep(Duration::from_millis(300));
    let first = fs::metadata(&alive)
        .map_err(|e| format!("alive file: {e}"))?
        .len();
    std::thread::sleep(Duration::from_millis(500));
    let second = fs::metadata(&alive)
        .map_err(|e| format!("alive file: {e}"))?
        .len();
    require(
        first == second,
        format!("the child is still running ({first} -> {second} bytes)"),
    )
}

fn waiting_for_the_project_lock_is_bounded(env: &Env) -> TestResult {
    let wd = env.dir("ok")?;
    let locks = Arc::new(ProjectLocks::new());
    let held = locks.guard(&wd);
    let _held = held.lock().map_err(|e| format!("lock: {e}"))?;
    let started = Instant::now();
    // The binary does not exist: if the call got past the lock it would fail to spawn
    // instead of timing out.
    let r = run_bounded(
        "definitely-not-a-real-cli-binary-xyz",
        &locks,
        &wd,
        &words(&["list"]),
        Duration::from_millis(300),
    );
    require(
        matches!(r, Err(BoundedError::TimedOut)),
        format!("expected a timeout waiting for the lock, got {r:?}"),
    )?;
    require(
        started.elapsed() < GIVE_UP_WITHIN,
        "the wait for the lock was not bounded",
    )
}

fn an_expired_deadline_starts_nothing(env: &Env) -> TestResult {
    let cli = BdCli::new_with_clock(
        "definitely-not-a-real-cli-binary-xyz",
        Arc::new(ProjectLocks::new()),
        Arc::new(SystemClock) as Arc<dyn Clock>,
    );
    let r = cli.fingerprint(&env.project("ok")?, after(0));
    require(
        matches!(
            r,
            Err(BeadsCacheError::Unavailable {
                timed_out: true,
                ..
            })
        ),
        format!("an expired deadline must time out before spawning, got {r:?}"),
    )
}

fn output_larger_than_a_pipe_is_drained(env: &Env) -> TestResult {
    let locks = ProjectLocks::new();
    let out = run_bounded(
        &env.exe,
        &locks,
        &env.dir("flood")?,
        &words(&["list"]),
        Duration::from_secs(60),
    )
    .map_err(|e| format!("flood: {e}"))?;
    require(out.success, "flood child failed")?;
    require(
        out.stdout.len() == FLOOD_STDOUT,
        format!("stdout {} bytes", out.stdout.len()),
    )?;
    require(
        out.stderr.len() == FLOOD_STDERR,
        format!("stderr {} bytes", out.stderr.len()),
    )
}

fn standard_input_is_closed(env: &Env) -> TestResult {
    let out = run_bounded(
        &env.exe,
        &ProjectLocks::new(),
        &env.dir("stdin")?,
        &words(&["list"]),
        Duration::from_secs(30),
    )
    .map_err(|e| format!("stdin: {e}"))?;
    require(
        out.stdout == "eof after 0 bytes",
        format!("got {:?}", out.stdout),
    )
}

fn argv_and_working_directory_reach_the_child_verbatim(env: &Env) -> TestResult {
    let wd = env.dir("echo")?;
    let out = run_bounded(
        &env.exe,
        &ProjectLocks::new(),
        &wd,
        &words(&[
            "update",
            "b-1",
            "",
            "--append-notes",
            "two words",
            "--actor",
            "san",
        ]),
        Duration::from_secs(30),
    )
    .map_err(|e| format!("echo: {e}"))?;
    let lines: Vec<&str> = out.stdout.lines().collect();
    let want = [
        "arg:update",
        "arg:b-1",
        "arg:",
        "arg:--append-notes",
        "arg:two words",
        "arg:--actor",
        "arg:san",
    ];
    require(
        lines.get(..want.len()) == Some(&want[..]),
        format!("argv was altered: {lines:?}"),
    )?;
    require(
        lines.last() == Some(&format!("beads_path:{wd}").as_str()),
        format!("BEADS_PATH is not the working directory: {lines:?}"),
    )
}

fn exit_13_is_a_precondition_refusal(env: &Env) -> TestResult {
    let patch = Patch {
        status: Some("open".into()),
        ..Patch::default()
    };
    let guards = Guards {
        status: Some("closed".into()),
        assignee: None,
    };
    let r = env.cli().update_guarded(
        &env.project("refuse")?,
        &sc_beads::BeadId::new("b-1").map_err(|e| e.to_string())?,
        &patch,
        &guards,
        &sc_beads::Actor::new("san").map_err(|e| e.to_string())?,
        after(30_000),
    );
    require(
        matches!(r, Err(BeadsCacheError::Refused(Refusal::Precondition))),
        format!("expected a precondition refusal, got {r:?}"),
    )
}

fn an_unreachable_server_is_unavailable(env: &Env) -> TestResult {
    let r = env
        .cli()
        .list_ready(&env.project("down")?, &scope(), after(30_000));
    require(
        matches!(&r, Err(e) if e.is_unavailable() && !e.is_timeout()),
        format!("expected Unavailable (not a timeout), got {r:?}"),
    )
}

fn a_bd_that_cannot_start_is_unavailable_with_its_cause(env: &Env) -> TestResult {
    let cli = BdCli::new_with_clock(
        "definitely-not-a-real-cli-binary-xyz",
        Arc::new(ProjectLocks::new()),
        Arc::new(SystemClock) as Arc<dyn Clock>,
    );
    let r = cli.list_ready(&env.project("ok")?, &scope(), after(30_000));
    let Err(e) = r else {
        return Err("expected an error".into());
    };
    require(e.is_unavailable() && !e.is_timeout(), format!("{e:?}"))?;
    let beads = std::error::Error::source(&e).ok_or("no source on the cache error")?;
    let io = std::error::Error::source(beads).ok_or("no io::Error behind the spawn error")?;
    require(!io.to_string().is_empty(), "empty io::Error")
}

fn the_backend_check_runs_version_and_schema(env: &Env) -> TestResult {
    let fp = env
        .cli()
        .fingerprint(&env.project("ok")?, after(30_000))
        .map_err(|e| e.to_string())?;
    require(
        fp.bd_version == BD_VERSION,
        format!("version {}", fp.bd_version),
    )?;
    require(fp.schema_sha256.as_str() == BD_SCHEMA_SHA256, "schema sha")?;
    require(
        fp.schema_version == "1",
        format!("schema_version {}", fp.schema_version),
    )
}

fn an_empty_list_is_no_rows(env: &Env) -> TestResult {
    let rows = env
        .cli()
        .list_scope(&env.project("ok")?, &scope(), None, after(30_000))
        .map_err(|e| e.to_string())?;
    require(rows.is_empty(), "expected no rows")
}

fn run_tests() -> ExitCode {
    let tests: &[(&str, Test)] = &[
        (
            "timeout_kills_a_running_child",
            timeout_kills_a_running_child,
        ),
        (
            "waiting_for_the_project_lock_is_bounded",
            waiting_for_the_project_lock_is_bounded,
        ),
        (
            "an_expired_deadline_starts_nothing",
            an_expired_deadline_starts_nothing,
        ),
        (
            "output_larger_than_a_pipe_is_drained",
            output_larger_than_a_pipe_is_drained,
        ),
        ("standard_input_is_closed", standard_input_is_closed),
        (
            "argv_and_working_directory_reach_the_child_verbatim",
            argv_and_working_directory_reach_the_child_verbatim,
        ),
        (
            "exit_13_is_a_precondition_refusal",
            exit_13_is_a_precondition_refusal,
        ),
        (
            "an_unreachable_server_is_unavailable",
            an_unreachable_server_is_unavailable,
        ),
        (
            "a_bd_that_cannot_start_is_unavailable_with_its_cause",
            a_bd_that_cannot_start_is_unavailable_with_its_cause,
        ),
        (
            "the_backend_check_runs_version_and_schema",
            the_backend_check_runs_version_and_schema,
        ),
        ("an_empty_list_is_no_rows", an_empty_list_is_no_rows),
    ];
    let filter = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    let selected: Vec<_> = tests
        .iter()
        .filter(|(name, _)| filter.as_ref().is_none_or(|f| name.contains(f.as_str())))
        .collect();

    let exe = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(e) => {
            eprintln!("cannot find the test executable: {e}");
            return ExitCode::FAILURE;
        }
    };
    let root = std::env::temp_dir().join(format!("sc-bd-cli-process-{}", std::process::id()));
    let env = Env { root, exe };

    println!("running {} tests", selected.len());
    let mut failed = 0;
    for (name, test) in selected {
        let outcome = catch_unwind(AssertUnwindSafe(|| test(&env)))
            .unwrap_or_else(|_| Err("panicked".to_owned()));
        match outcome {
            Ok(()) => println!("test {name} ... ok"),
            Err(why) => {
                failed += 1;
                println!("test {name} ... FAILED\n    {why}");
            }
        }
    }
    let _ = fs::remove_dir_all(&env.root);
    if failed == 0 {
        println!("\ntest result: ok.");
        ExitCode::SUCCESS
    } else {
        println!("\ntest result: FAILED. {failed} failed");
        ExitCode::FAILURE
    }
}
