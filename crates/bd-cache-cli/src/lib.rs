//! The `bd-cache` / `bdc` command line (contract `docs/bd-cache-cli/contract.md`).
//!
//! This crate is the home of the two binaries `bd-cache` and its alias `bdc`
//! (REQ-BDC-0805), and of the `fake-bd` test harness (feature `fake-bd`,
//! contract §8 "Testing"). Sprint b-3a ships the skeleton: [`run`] answers
//! `--version` and answers everything else with a `BD_CACHE.USAGE` envelope.
//! Sprint b-3 adds the commands behind the same signature.
//!
//! # One envelope on stdout
//!
//! Every reply is one [`sc_command::Envelope`] followed by a newline
//! (contract §1 rule 1, §4). The exit code is `0` for `ok: true`, `2` for a
//! usage error and `3` for any other typed error (contract §3).
//!
//! # In process and as a process
//!
//! [`run`] is the in-process entry point the tests call. [`run_process`] is
//! the body of both binaries: it reads the real argv and environment, calls
//! [`run`] and prints the reply.
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
use std::io::Write as _;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Arg, ArgAction, Command};
use sc_bd_cli::CliInvoker;
use sc_beads::Clock;
use sc_command::{Envelope, ErrorKind, OpError};
use sc_observability_types::ErrorCode;
use serde_json::{Value, json};

#[cfg(feature = "fake-bd")]
pub mod scenario;

/// The name `--version` reports, for `bd-cache` and for its alias `bdc` alike.
///
/// Both binaries must print identical output for the same argv (REQ-BDC-0805),
/// so the name is the canonical one and never `argv[0]`.
pub const NAME: &str = "bd-cache";

/// The contract version this build implements (contract header, `1.0.0`).
pub const CONTRACT_VERSION: &str = "1.0.0";

/// The exit code of a `BD_CACHE.USAGE` reply (contract §3).
pub const EXIT_USAGE: i32 = 2;

/// The error code of an argv-shape error (contract §4).
const CODE_USAGE: &str = "BD_CACHE.USAGE";

/// The long flags this build understands, listed in a `USAGE` reply as `details.valid`.
const VALID_FLAGS: &[&str] = &["--version", "--repo", "--cache-path", "--bd"];

/// Runs one command and returns `(exit code, stdout)`.
///
/// `argv` is the command line **without** the program name. `env` is the
/// process environment as a map; `invoker` is the `bd` to use (`None` means
/// the real `bd` the configuration names) and `clock` the time source of the
/// command. The skeleton of sprint b-3a reads none of `env`, `invoker` and
/// `clock`; b-3 consumes them behind this signature, which is final.
///
/// The reply is always one envelope and a trailing newline. Only `--version`
/// succeeds in this build; every other argv is a `BD_CACHE.USAGE` error.
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
    _env: &BTreeMap<String, String>,
    _invoker: Option<Box<dyn CliInvoker>>,
    _clock: Arc<dyn Clock>,
) -> (i32, String) {
    match command()
        .try_get_matches_from(std::iter::once(NAME.to_owned()).chain(argv.iter().cloned()))
    {
        Ok(matches) if matches.get_flag("version") => {
            let data = json!({
                "name": NAME,
                "version": env!("CARGO_PKG_VERSION"),
                "contract_version": CONTRACT_VERSION,
            });
            (0, render(&Envelope::success(data)))
        }
        Ok(_) => usage("no command given; only --version is implemented in this build"),
        Err(e) => usage(first_line(&e.to_string())),
    }
}

/// The body of the `bd-cache` and `bdc` binaries.
///
/// Reads the real argv and environment, calls [`run`] with the real `bd`
/// (`None`) and the system clock, writes the reply to stdout and returns the
/// exit code.
#[must_use]
pub fn run_process() -> ExitCode {
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let env: BTreeMap<String, String> = std::env::vars().collect();
    let (code, stdout) = run(&argv, &env, None, Arc::new(sc_beads::SystemClock));
    // A closed stdout leaves nobody to tell; the exit code still reports the outcome.
    let _written = std::io::stdout().write_all(stdout.as_bytes());
    ExitCode::from(u8::try_from(code).unwrap_or(3))
}

/// The argv grammar of this build: `--version` and the three global path options of
/// contract §2, which are accepted so that the harness invocation
/// (`bdc --repo DIR --bd PATH ...`) has its final shape. b-3 reads them; this build
/// does not. Abbreviations are refused (clap's `infer_long_args` is off), as
/// contract §2 requires.
fn command() -> Command {
    let path = |name: &'static str, value_name: &'static str| {
        Arg::new(name)
            .long(name)
            .value_name(value_name)
            .num_args(1)
            .global(true)
    };
    Command::new(NAME)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(
            Arg::new("version")
                .long("version")
                .action(ArgAction::SetTrue),
        )
        .arg(path("repo", "DIR"))
        .arg(path("cache-path", "PATH"))
        .arg(path("bd", "PATH"))
}

/// A `BD_CACHE.USAGE` reply (contract §3, §4).
fn usage(message: &str) -> (i32, String) {
    let error = OpError::new(
        ErrorKind::InvalidInput,
        ErrorCode::new_static(CODE_USAGE),
        message,
    )
    .with_details(json!({ "valid": VALID_FLAGS, "did_you_mean": Value::Null }))
    .with_suggestion("run `bdc --version`; the commands arrive with sprint b-3");
    (EXIT_USAGE, render(&Envelope::<Value>::failure(error)))
}

/// The envelope as one line of JSON.
fn render(envelope: &Envelope<Value>) -> String {
    // An envelope of plain JSON values always serializes; the fallback is a valid
    // failure envelope should that ever stop being true.
    let mut line = serde_json::to_string(envelope).unwrap_or_else(|_| {
        r#"{"version":"1","ok":false,"data":null,"error":{"kind":"internal","code":"ENVELOPE.SERIALIZATION","message":"the reply could not be serialized","details":null,"suggested_action":null}}"#.to_owned()
    });
    line.push('\n');
    line
}

/// The first line of a clap message (the rest is a usage block for a terminal).
fn first_line(message: &str) -> &str {
    let line = message.lines().next().unwrap_or(message).trim();
    line.strip_prefix("error: ").unwrap_or(line)
}
