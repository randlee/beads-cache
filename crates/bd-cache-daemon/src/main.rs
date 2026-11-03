//! Explicit foreground daemon process, adapted from the pinned scaffold main.

use bd_cache_daemon as daemon;
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "bd-cache-daemon",
    version,
    about = "Foreground cache scheduler with loopback readiness"
)]
struct Args {
    /// Absolute repository root containing the existing .beads configuration.
    #[arg(long)]
    repo: PathBuf,
    /// Explicit daemon JSON file (separate from .beads/bd-cache.toml).
    #[arg(long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(a) => a,
        Err(e) => return usage(&e),
    };
    if !args.repo.is_absolute() {
        return startup_exit(
            "CLI.USAGE",
            "--repo must be an absolute path",
            "run with --help",
            2,
        );
    }
    let config = match daemon::config::load(&args.config) {
        Ok(c) => c,
        Err(e) => return startup_exit(e.code(), config_cause(&e), e.recovery(), 1),
    };
    let name = match sc_transport::AppName::new(daemon::APP_NAME) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("{}", daemon::agent_error::transport_error(&e));
            return ExitCode::from(1);
        }
    };
    let log_root = match config
        .logging
        .log_root
        .clone()
        .map_or_else(|| daemon::config::instance_root(&config, &name), Ok)
    {
        Ok(root) => root,
        Err(e) => {
            eprintln!("{}", daemon::agent_error::transport_error(&e));
            return ExitCode::from(1);
        }
    };
    let observability = match daemon::observability::Observability::init(
        daemon::APP_NAME,
        &config.logging,
        log_root,
    ) {
        Ok(o) => o,
        Err(e) => {
            return startup_exit(
                "DAEMON.OBSERVABILITY",
                e,
                "fix logging configuration or sink paths and retry",
                1,
            );
        }
    };
    let primary = daemon::composition::run(&config, &args.repo, &name, &observability).await;
    // Preserve typed failures until the single final process formatting boundary.
    let result = primary.map_err(|e| e.to_string());
    match daemon::agent_error::finish(result, observability.shutdown()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn startup_exit(code: &str, cause: impl std::fmt::Display, recovery: &str, exit: u8) -> ExitCode {
    eprintln!(
        "{}",
        daemon::agent_error::format_agent_error(code, cause, recovery)
    );
    ExitCode::from(exit)
}
fn config_cause(error: &sc_config::ConfigError) -> String {
    let rendered = error.to_string();
    rendered
        .strip_prefix(&format!("{}: ", error.code()))
        .and_then(|s| s.strip_suffix(&format!("; {}", error.recovery())))
        .unwrap_or(&rendered)
        .to_owned()
}
fn usage(error: &clap::Error) -> ExitCode {
    if matches!(
        error.kind(),
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
    ) {
        print!("{error}");
        return ExitCode::SUCCESS;
    }
    let rendered = error.to_string();
    startup_exit(
        "CLI.USAGE",
        rendered
            .lines()
            .find(|l| !l.is_empty())
            .unwrap_or("invalid arguments"),
        "run with --help",
        2,
    )
}
