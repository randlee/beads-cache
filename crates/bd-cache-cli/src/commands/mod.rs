//! The commands: decode the request, open the cache, run the operation.
//!
//! Every surface (a) and (b) command is a `bd-cache-api` [`Command`]: its
//! request goes through [`bd_cache_api::decode`], then
//! [`bd_cache_api::execute`] over the composed cache, and the `data` comes
//! back unreshaped (contract §8, MCP design constraint). The phase-B verbs
//! a front end runs ([`Command::is_front_end`]: `cache schedule *`, `cron
//! run`, `tick`, `sync *`) are decoded by the same decoder, then run over
//! `bd-cache-sync`: `cron run` here ([`cron`]), the others through
//! [`bd_cache_sync::execute`] (ADR-BDC-0015). The request is decoded
//! **before** the cache is opened, so a bad request never touches the store.
//! `scopes ingest` decodes its snapshot from stdin as it reads it
//! (REQ-BDC-0726), once the rest of its request has decoded.

pub(crate) mod beads;
pub(crate) mod cron;
pub(crate) mod manage;
pub(crate) mod runtime;
pub(crate) mod schedule;
pub(crate) mod scopes;
pub(crate) mod signal;
pub(crate) mod sync;
pub(crate) mod tick;

use std::collections::BTreeMap;
use std::sync::Arc;

use bd_cache_api::{ApiError, CacheFacts, Command, Request, SyncFacts, decode, execute};
use bd_cache_store::SCHEMA_VERSION;
use bd_cache_sync::{ComposeError, Composed, FrontEnd, compose_with_config};
use sc_bd_cli::CliInvoker;
use sc_beads::Clock;
use serde_json::{Map, Value};

use crate::commands::signal::Signals;
use crate::config::{self, Flags, Settings};

/// What a command runs with, besides its request.
pub(crate) struct Context<'a> {
    /// The configuration flags.
    pub(crate) flags: Flags,
    /// The environment given to [`crate::run`].
    pub(crate) env: &'a BTreeMap<String, String>,
    /// The bd to use instead of the configured binary (tests).
    pub(crate) invoker: Option<Box<dyn CliInvoker>>,
    /// The command's clock.
    pub(crate) clock: Arc<dyn Clock>,
    /// What a signal did (RSH-001, REQ-BDC-0730): `cron run`, `tick` and
    /// `sync use-python` stop on its flag.
    pub(crate) signals: &'a Signals,
    /// `scopes ingest` reads its snapshot from stdin (no `--input`).
    pub(crate) ingest_stdin: bool,
}

impl std::fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("flags", &self.flags)
            .field("invoker", &self.invoker.as_ref().map(|i| i.binary()))
            .finish_non_exhaustive()
    }
}

/// Runs `command` with `request` and returns its `data`.
///
/// # Errors
///
/// The decoder's errors, then the configuration and open-time errors
/// (`CONFIG_INVALID`, `NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION`,
/// `STORE_FAILED`), then the command's own.
pub(crate) fn run(command: Command, request: &Value, ctx: Context<'_>) -> Result<Value, ApiError> {
    let mut decoded = decode(command, request)?;
    if ctx.ingest_stdin {
        decoded = scopes::with_stdin_snapshot(decoded, std::io::stdin().lock())?;
    }
    let settings = config::resolve(&ctx.flags, ctx.env)?;
    let mut composed = open(&settings, ctx.invoker, Arc::clone(&ctx.clock))?;
    if !command.is_front_end() {
        let mut facts = facts(&settings);
        if command == Command::CacheHealth {
            facts.sync = Some(sync_facts(&composed)?);
        }
        return execute(&mut composed.cache, &decoded, &facts)?.to_json();
    }
    run_front_end(composed, &decoded, ctx.signals)
}

/// Runs a phase-B verb over `bd-cache-sync` (ADR-BDC-0015): `cron run`
/// here, every other one through [`bd_cache_sync::execute`].
fn run_front_end(composed: Composed, req: &Request, signals: &Signals) -> Result<Value, ApiError> {
    let schedules = composed
        .into_schedules()
        .map_err(|e| FrontEnd::Cli.api_error(&e))?;
    if let Request::CronRun(r) = req {
        return cron::run(&schedules, r, signals);
    }
    let rt = runtime::runtime()?;
    rt.block_on(bd_cache_sync::execute(
        &schedules,
        req,
        signals.stop(),
        FrontEnd::Cli,
    ))
}

/// The facts of the cache file and `metadata.json` that `context` and
/// `cache status` report (the api opens neither).
fn facts(s: &Settings) -> CacheFacts {
    CacheFacts {
        cache_path: s.location.cache_path.display().to_string(),
        store_schema_version: i64::from(SCHEMA_VERSION),
        database: s.location.identity.dolt_database.as_str().to_owned(),
        project_id: s.location.identity.project_id.as_str().to_owned(),
        beads_dir: Some(s.beads_dir.display().to_string()),
        repo_root: Some(s.repo.display().to_string()),
        sync: None,
    }
}

/// The sync rows' state `cache health` judges (REQ-BDC-0732), read from the
/// cache file's schedule table (`bd_cache_sync::sync_facts`, as the binding reads it).
fn sync_facts(composed: &Composed) -> Result<SyncFacts, ApiError> {
    let store = composed
        .schedule_store()
        .map_err(|e| FrontEnd::Cli.api_error(&e))?;
    bd_cache_sync::sync_facts(store.as_ref(), &composed.cache)
        .map_err(|e| FrontEnd::Cli.api_error(&e))
}

/// A composition error as the contract's code (contract §2, §4).
fn compose_error(e: &ComposeError) -> ApiError {
    match e {
        ComposeError::Cache(c) => ApiError::from(c),
        ComposeError::Config(c) => ApiError::config_invalid(e.code(), c.to_string(), Map::new())
            .with_suggested_action(e.fix()),
        ComposeError::BeadsDir(_) | ComposeError::RepoRootNotUtf8(_) => {
            ApiError::config_invalid(e.code(), e.to_string(), Map::new())
                .with_suggested_action(e.fix())
        }
        _ => ApiError::internal(format!("cannot open the cache ({}): {e}", e.code())),
    }
}

/// Opens the cache of `settings`: the merged configuration through
/// `compose_with_config`, with the configured `bd`.
///
/// An injected `invoker` (an in-process test's fake bd) is composed with
/// `compose_with_config_and_backend` over the same merged configuration, so
/// env and flag overrides apply on both paths (lead ruling S72).
fn open(
    settings: &Settings,
    invoker: Option<Box<dyn CliInvoker>>,
    clock: Arc<dyn Clock>,
) -> Result<Composed, ApiError> {
    let beads_dir = settings.beads_dir_env.as_deref();
    match invoker {
        None => compose_with_config(
            &settings.repo,
            settings.config.clone(),
            &settings.bd,
            clock,
            beads_dir,
        )
        .map_err(|e| compose_error(&e)),
        Some(invoker) => open_with_invoker(settings, invoker, clock),
    }
}

#[cfg(feature = "fake-bd")]
fn open_with_invoker(
    settings: &Settings,
    invoker: Box<dyn CliInvoker>,
    clock: Arc<dyn Clock>,
) -> Result<Composed, ApiError> {
    bd_cache_sync::compose_with_config_and_backend(
        &settings.repo,
        settings.config.clone(),
        |_locks| sc_bd_cli::BdCli::with_invoker(invoker),
        Arc::new(sc_bd_cli::ProjectLocks::new()),
        clock,
        settings.beads_dir_env.as_deref(),
    )
    .map_err(|e| compose_error(&e))
}

#[cfg(not(feature = "fake-bd"))]
fn open_with_invoker(
    _settings: &Settings,
    _invoker: Box<dyn CliInvoker>,
    _clock: Arc<dyn Clock>,
) -> Result<Composed, ApiError> {
    Err(ApiError::internal(
        "an injected bd invoker needs the fake-bd feature; this build runs the configured bd",
    ))
}
