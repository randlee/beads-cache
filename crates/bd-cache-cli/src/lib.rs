//! The `bd-cache` / `bdc` command line (contract `docs/bd-cache-cli/contract.md`).
//!
//! The two binaries `bd-cache` and its alias `bdc` (REQ-BDC-0805) implement
//! both command groups of the contract over `bd-cache-api`: surface (a), the
//! beads commands in `/v0` vocabulary (§5), and surface (b), the cache
//! commands (§6). The crate also holds the `fake-bd` test harness (feature
//! `fake-bd`, contract §8 "Testing").
//!
//! # One path for every command
//!
//! 1. argv is parsed by clap (one table of commands, module `args`). Long
//!    options are spelled out in full; any argv-shape error is
//!    `BD_CACHE.USAGE`, exit 2, with `details.did_you_mean` and `valid`.
//! 2. argv becomes the command's JSON request (or `--input FILE|-` gives it
//!    whole), which goes to `bd_cache_api::decode`: validation lives there,
//!    so every front end returns the same code for the same input.
//! 3. The configuration (flag > `BD_CACHE__*` env > `.beads/bd-cache.toml` >
//!    default; no upward search) is opened through
//!    `bd_cache_sync::compose_with_config`, and `bd_cache_sync::execute_api`
//!    runs the request (ADR-BDC-0015 Amendment 1: every front end calls it,
//!    so `cache health` is never answered without the sync rows). The phase-B verbs a front end runs (`cache schedule *`,
//!    `cache cron run`, `cache tick`, `cache sync *`) run over
//!    `bd-cache-sync` (ADR-BDC-0015). Every new command is on the `cache`
//!    management surface; none joins the `/v0`-shaped beads surface
//!    (ADR-BDC-0009).
//! 4. The result is one [`sc_command::Envelope`] on stdout, or text with
//!    `--format text`. Logs go to stderr (`-v`, `-q`, `BD_CACHE__LOG`).
//!
//! # Exit codes (contract §3)
//!
//! `0` for `ok: true` (degraded results are reported in `data`), `2` for
//! `USAGE`, `3` for any other typed error. A caught panic is
//! `BD_CACHE.INTERNAL`, exit 3.
//!
//! # Signals (RSH-001, ADR-CLI-0005)
//!
//! On Unix [`run_process`] handles `SIGTERM`, `SIGINT` and `SIGHUP`: the
//! running `bd` call's process group is killed and reaped, the project locks
//! are released as the call returns, `cache tick` and `sync use-python` stop
//! (killing a hook's or script's process tree), `cache cron run` starts no
//! new run and waits for the runs in flight up to `--shutdown-timeout-ms`
//! (REQ-BDC-0730), the command's envelope is still written, and the process
//! exits `128 + signal` (143 for `SIGTERM`) whatever the envelope says. A
//! second signal never changes the exit code; during `cache cron run`'s wait it
//! means "stop now" (Ctrl-C twice): the `bd` calls are stopped and the runs'
//! process trees killed at once, as when `--shutdown-timeout-ms` expires.
//!
//! # `--help`
//!
//! Text when stdout is a terminal and no `--format` is given; otherwise the
//! envelope, with `data: {command, synopsis, arguments}`. Exit 0. [`run`]
//! answers as for a pipe; [`run_with`] takes the terminal state, which
//! [`run_process`] reads from the real stdout.
//!
//! # Example
//!
//! ```
//! use std::collections::BTreeMap;
//! use std::sync::Arc;
//!
//! use sc_beads::SystemClock;
//!
//! let argv = vec!["--version".to_owned()];
//! let (exit, stdout) = bd_cache_cli::run(&argv, &BTreeMap::new(), None, Arc::new(SystemClock));
//! assert_eq!(exit, 0);
//! let reply: serde_json::Value = serde_json::from_str(&stdout)?;
//! assert_eq!(reply["ok"], true);
//! assert_eq!(reply["data"]["name"], "bd-cache");
//! # Ok::<(), serde_json::Error>(())
//! ```

use std::collections::BTreeMap;
use std::io::{IsTerminal as _, Write as _};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;
use std::sync::Arc;

use bd_cache_api::{ApiError, Command};
use clap::ArgMatches;
use clap::error::{ContextKind, ContextValue};
use sc_bd_cli::CliInvoker;
use sc_beads::Clock;
use sc_beads::logging::tracing;
use serde_json::{Value, json};

mod args;
mod commands;
mod config;
mod help;
mod log;
mod render;
mod request;

#[cfg(feature = "fake-bd")]
pub mod scenario;

/// The name `--version` reports, for `bd-cache` and for its alias `bdc` alike.
///
/// Both binaries must print identical output for the same argv (REQ-BDC-0805),
/// so the name is the canonical one and never `argv[0]`.
pub const NAME: &str = "bd-cache";

/// The contract version this build implements (contract header, `1.0.0`).
pub const CONTRACT_VERSION: &str = bd_cache_api::CONTRACT_VERSION;

/// The exit code of a `BD_CACHE.USAGE` reply (contract §3).
pub const EXIT_USAGE: i32 = 2;

/// The exit code of every typed error other than `USAGE` (contract §3).
pub const EXIT_ERROR: i32 = 3;

/// The environment variable that makes a command panic after parsing, so the
/// tests can see a caught panic answer `BD_CACHE.INTERNAL`. Only read in a
/// build with the `fake-bd` feature, which a release build never has.
#[cfg(feature = "fake-bd")]
pub const PANIC_ENV: &str = "BDC_TEST_PANIC";

/// Whether stdout is a terminal: the one thing `--help` depends on (contract §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stdout {
    /// A terminal: `--help` without `--format` prints text.
    Terminal,
    /// A pipe or a file: `--help` prints the envelope.
    Piped,
}

/// Runs one command and returns `(exit code, stdout)`, as for a piped stdout.
///
/// `argv` is the command line **without** the program name. `env` is the
/// process environment as a map (`BD_CACHE__*`, `BEADS_DIR`); `invoker` is
/// the `bd` to use (`None` means the real `bd` the configuration names; an
/// injected one needs the `fake-bd` feature) and `clock` the time source of
/// the command.
///
/// The reply is one envelope and a trailing newline, or text with
/// `--format text`.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use std::sync::Arc;
///
/// use sc_beads::SystemClock;
///
/// let argv = vec!["frobnicate".to_owned()];
/// let (exit, stdout) = bd_cache_cli::run(&argv, &BTreeMap::new(), None, Arc::new(SystemClock));
/// assert_eq!(exit, 2);
/// assert!(stdout.contains("BD_CACHE.USAGE"));
/// ```
#[must_use]
pub fn run(
    argv: &[String],
    env: &BTreeMap<String, String>,
    invoker: Option<Box<dyn CliInvoker>>,
    clock: Arc<dyn Clock>,
) -> (i32, String) {
    run_with(argv, env, invoker, clock, Stdout::Piped)
}

/// As [`run`], with the state of stdout given (only `--help` reads it).
#[must_use]
pub fn run_with(
    argv: &[String],
    env: &BTreeMap<String, String>,
    invoker: Option<Box<dyn CliInvoker>>,
    clock: Arc<dyn Clock>,
    stdout: Stdout,
) -> (i32, String) {
    run_stoppable(
        argv,
        env,
        invoker,
        clock,
        stdout,
        &commands::signal::Signals::new(),
    )
}

/// As [`run_with`], with what the process's signals do (`cron run`, `tick`
/// and `sync use-python` stop on them).
fn run_stoppable(
    argv: &[String],
    env: &BTreeMap<String, String>,
    invoker: Option<Box<dyn CliInvoker>>,
    clock: Arc<dyn Clock>,
    stdout: Stdout,
    signals: &commands::signal::Signals,
) -> (i32, String) {
    let text = args::format_flag(argv) == Some("text");
    match catch_unwind(AssertUnwindSafe(|| {
        dispatch(argv, env, invoker, clock, stdout, signals)
    })) {
        Ok(reply) => reply,
        Err(payload) => {
            let why = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a panic with no message".to_owned());
            let e = ApiError::internal(format!("bdc panicked: {why}"))
                .with_suggested_action("report this bug, with the command that caused it");
            finish(&Err(e), text)
        }
    }
}

/// The body of the `bd-cache` and `bdc` binaries.
///
/// Reads the real argv and environment, calls [`run_with`] with the real `bd`
/// (`None`), the system clock and whether stdout is a terminal, writes the
/// reply to stdout and returns the exit code.
///
/// On Unix `SIGTERM`, `SIGINT` and `SIGHUP` stop the command instead of
/// killing the process outright (RSH-001; module docs, "Signals"): the reply
/// is still written, and the exit code is then `128 + signal`.
#[must_use]
pub fn run_process() -> ExitCode {
    let signals = commands::signal::Signals::new();
    // Before anything can start a `bd`: from here a signal stops the calls.
    #[cfg(unix)]
    if let Err(e) = commands::signal::install(&signals) {
        // No log is set up yet; stderr is the log's destination anyway.
        let _written = writeln!(
            std::io::stderr(),
            "bdc: cannot handle SIGTERM/SIGINT/SIGHUP ({e}); a signal will kill bdc but not its bd"
        );
    }
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let env: BTreeMap<String, String> = std::env::vars_os()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect();
    // The libraries' gated `log_*!` calls reach the stderr log of each command.
    sc_beads::logging::LOGGING_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    let stdout = if std::io::stdout().is_terminal() {
        Stdout::Terminal
    } else {
        Stdout::Piped
    };
    let (code, reply) = run_stoppable(
        &argv,
        &env,
        None,
        Arc::new(sc_beads::SystemClock),
        stdout,
        &signals,
    );
    // A closed stdout leaves nobody to tell; the exit code still reports the outcome.
    let _written = std::io::stdout().write_all(reply.as_bytes());
    let _flushed = std::io::stdout().flush();
    match signals.first() {
        Some(signal) => {
            let _written = writeln!(std::io::stderr(), "bdc: stopped by signal {signal}");
            ExitCode::from(commands::signal::exit_code(signal))
        }
        None => ExitCode::from(u8::try_from(code).unwrap_or(3)),
    }
}

/// The exit code and stdout of a result.
fn finish(result: &Result<Value, ApiError>, text: bool) -> (i32, String) {
    let exit = match result {
        Ok(_) => 0,
        Err(e) => e.exit_code().unwrap_or(EXIT_ERROR),
    };
    // USAGE is always JSON (contract §8).
    let usage = result
        .as_ref()
        .is_err_and(|e| e.exit_code() == Some(EXIT_USAGE));
    let out = if text && !usage {
        render::text::render(result)
    } else {
        render::json::envelope(result)
    };
    (exit, out)
}

fn dispatch(
    argv: &[String],
    env: &BTreeMap<String, String>,
    invoker: Option<Box<dyn CliInvoker>>,
    clock: Arc<dyn Clock>,
    stdout: Stdout,
    signals: &commands::signal::Signals,
) -> (i32, String) {
    let format = args::format_flag(argv);
    if args::has_flag(argv, "--help") {
        return help(argv, format, stdout);
    }
    if args::has_flag(argv, "--version") {
        if format == Some("text") {
            let text = format!(
                "{NAME} {} (contract {CONTRACT_VERSION})\n",
                env!("CARGO_PKG_VERSION")
            );
            return (0, text);
        }
        let data = json!({
            "name": NAME,
            "version": env!("CARGO_PKG_VERSION"),
            "contract_version": CONTRACT_VERSION,
        });
        return finish(&Ok(data), false);
    }
    if let Some(e) = commands::beads::unserved(&args::words(argv)) {
        return finish(&Err(e), format == Some("text"));
    }
    let relaxed = args::has_flag(argv, "--input");
    let matches = match args::clap_tree(relaxed)
        .try_get_matches_from(std::iter::once(NAME.to_owned()).chain(argv.iter().cloned()))
    {
        Ok(m) => m,
        Err(e) => return finish(&Err(usage_from_clap(&e, argv)), false),
    };
    let (path, leaf) = leaf(&matches);
    let Some(spec) = args::command(&path) else {
        let valid = args::children(&path);
        let e =
            ApiError::usage("no command given", &valid).with_detail("did_you_mean", Value::Null);
        return finish(&Err(e), false);
    };
    let text = leaf.get_one::<String>("format").map(String::as_str) == Some("text");
    let from_input = leaf.get_one::<String>("input").is_some();
    let request = match leaf.get_one::<String>("input") {
        Some(_) if request::has_request_args(spec, leaf) => {
            let e = ApiError::usage(
                "--input gives the whole request: it cannot be combined with request flags",
                &["--input"],
            )
            .with_detail("did_you_mean", Value::Null);
            return finish(&Err(e), false);
        }
        Some(source) => request::from_input(source),
        None => request::from_matches(spec, leaf),
    };
    let request = match request {
        Ok(r) => r,
        Err(e) => return finish(&Err(e), text),
    };
    let flags = config::Flags {
        repo: leaf.get_one::<String>("repo").cloned(),
        cache_path: leaf.get_one::<String>("cache-path").cloned(),
        bd: leaf.get_one::<String>("bd").cloned(),
    };
    let level = log::level(
        leaf.get_count("verbose"),
        leaf.get_flag("quiet"),
        config::env_value(env, "LOG"),
        log::default_for(spec.command),
    );
    let ctx = commands::Context {
        flags,
        env,
        invoker,
        clock,
        signals,
        // `scopes ingest` streams its snapshot from stdin (REQ-BDC-0726).
        ingest_stdin: spec.command == Command::ScopesIngest && !from_input,
    };
    if spec.command == Command::CronRun {
        // sc-cron's events come from runtime and signal threads: the stderr
        // log is the process default for `cron run` (REQ-BDC-0730). A second
        // default in one process (in-process tests) keeps the first.
        let _first = tracing::subscriber::set_global_default(log::StderrLog::new(level));
    }
    let result = tracing::subscriber::with_default(log::StderrLog::new(level), || {
        tracing::info!(command = spec.path, "running");
        for key in config::unknown_env_keys(env) {
            tracing::warn!(
                key = %format!("{}{key}", config::ENV_PREFIX),
                "this environment variable is not a bdc setting and is ignored"
            );
        }
        test_panic(env);
        commands::run(spec.command, &request, ctx)
    });
    finish(&result, text)
}

/// The test hook of [`PANIC_ENV`].
#[cfg(feature = "fake-bd")]
/// It exists to prove that a caught panic is `BD_CACHE.INTERNAL`.
fn test_panic(env: &BTreeMap<String, String>) {
    assert!(!env.contains_key(PANIC_ENV), "{PANIC_ENV} is set");
}

#[cfg(not(feature = "fake-bd"))]
fn test_panic(_env: &BTreeMap<String, String>) {}

/// The deepest subcommand matched: its path and its matches.
fn leaf(matches: &ArgMatches) -> (String, &ArgMatches) {
    let mut path: Vec<&str> = Vec::new();
    let mut node = matches;
    while let Some((name, sub)) = node.subcommand() {
        path.push(name);
        node = sub;
    }
    (path.join(" "), node)
}

/// The longest prefix of the command words that names a command or group.
fn known_path(words: &[&str]) -> String {
    let mut path = String::new();
    for w in words {
        let next = if path.is_empty() {
            (*w).to_owned()
        } else {
            format!("{path} {w}")
        };
        if help::of(&next).is_none() {
            break;
        }
        let is_command = args::command(&next).is_some();
        path = next;
        if is_command {
            break;
        }
    }
    path
}

/// `--help`: text on a terminal without `--format`, else the envelope.
fn help(argv: &[String], format: Option<&str>, stdout: Stdout) -> (i32, String) {
    let words = args::words(argv);
    let path = known_path(&words);
    let depth = path.split_whitespace().count();
    if args::command(&path).is_none() && words.len() > depth {
        // A word that names nothing: the parser's own USAGE, with its suggestion.
        let without: Vec<String> = argv.iter().filter(|t| *t != "--help").cloned().collect();
        let e = match args::clap_tree(true)
            .try_get_matches_from(std::iter::once(NAME.to_owned()).chain(without.iter().cloned()))
        {
            Err(e) => usage_from_clap(&e, &without),
            Ok(_) => ApiError::usage("unknown command", &args::children(&path)),
        };
        return finish(&Err(e), false);
    }
    let Some(h) = help::of(&path) else {
        let e = ApiError::usage("unknown command", &args::children(""));
        return finish(&Err(e), false);
    };
    let as_text = match format {
        Some("text") => true,
        Some(_) => false,
        None => stdout == Stdout::Terminal,
    };
    if as_text {
        (0, h.text())
    } else {
        finish(&Ok(h.data()), false)
    }
}

/// The words a `USAGE` reply lists as `valid` at the deepest known path of argv.
fn valid_words(argv: &[String]) -> Vec<String> {
    let path = known_path(&args::words(argv));
    match args::command(&path) {
        Some(spec) => spec
            .args
            .iter()
            .map(args::ArgSpec::spelled)
            .chain(args::GLOBALS.iter().map(|g| g.flag.to_owned()))
            .collect(),
        None => args::children(&path)
            .into_iter()
            .map(str::to_owned)
            .collect(),
    }
}

/// A clap error as `BD_CACHE.USAGE` (contract §3).
fn usage_from_clap(e: &clap::Error, argv: &[String]) -> ApiError {
    let rendered = e.to_string();
    let first = rendered.lines().next().unwrap_or(&rendered).trim();
    let message = first.strip_prefix("error: ").unwrap_or(first);
    let suggestion = [ContextKind::SuggestedArg, ContextKind::SuggestedSubcommand]
        .into_iter()
        .find_map(|k| match e.get(k) {
            Some(ContextValue::String(s)) => Some(s.clone()),
            Some(ContextValue::Strings(v)) => v.first().cloned(),
            _ => None,
        });
    let valid = valid_words(argv);
    let valid: Vec<&str> = valid.iter().map(String::as_str).collect();
    ApiError::usage(message, &valid).with_detail("did_you_mean", json!(suggestion))
}
