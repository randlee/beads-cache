//! The `bdc` binary, the alias of `bd-cache` (REQ-BDC-0805).

use std::process::ExitCode;

fn main() -> ExitCode {
    bd_cache_cli::run_process()
}
