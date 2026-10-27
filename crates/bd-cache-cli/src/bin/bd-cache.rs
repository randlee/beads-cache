//! The `bd-cache` binary: the same entry point as `bdc` (REQ-BDC-0805).

use std::process::ExitCode;

fn main() -> ExitCode {
    bd_cache_cli::run_process()
}
