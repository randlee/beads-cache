//! `fake-bd`: a `bd` executable over sc-bd-cli's stateful `FakeBd`.
//!
//! It is the real-process half of the test harness of contract §8 "Testing".
//! Point `bdc --bd` (or `BdCli`) at this binary and name a scenario file in
//! `$FAKE_BD_SCENARIO`; every invocation loads the scenario, answers the `bd`
//! argv it was given and writes the rows back after a call that changed one, so
//! a sequence of processes sees one consistent bd. The scenario format is in
//! `docs/bd-cache-cli/architecture.md`; faults (unavailable, slow, refused
//! write) and the fixed clock are scenario fields.
//!
//! The binary builds only with the `fake-bd` feature and is never part of a
//! release build (ADR-CLI-0001).

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use bd_cache_cli::scenario::{EXIT_HARNESS, SCENARIO_ENV, serve};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let Some(path) = std::env::var_os(SCENARIO_ENV) else {
        eprintln!("fake-bd: {SCENARIO_ENV} is not set");
        return ExitCode::from(u8::try_from(EXIT_HARNESS).unwrap_or(1));
    };
    let served = serve(&argv, &PathBuf::from(path));
    // A closed pipe leaves nobody to tell; the exit code still reports the outcome.
    let _out = std::io::stdout().write_all(served.stdout.as_bytes());
    let _err = std::io::stderr().write_all(served.stderr.as_bytes());
    ExitCode::from(u8::try_from(served.exit).unwrap_or(1))
}
