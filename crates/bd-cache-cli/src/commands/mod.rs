//! The commands: decode the request, open the cache, run the operation.
//!
//! Every surface (a) and (b) command is a `bd-cache-api` [`Command`]: its
//! request goes through [`bd_cache_api::decode`], then
//! [`bd_cache_api::execute`] over the composed cache, and the `data` comes
//! back unreshaped (contract §8, MCP design constraint). The phase-B verbs
//! a front end runs (`cache schedule *`, `cron run`, `tick`, `sync *`) are
//! decoded by the same decoder, then run over `bd-cache-sync` ([`schedule`],
//! [`cron`], [`tick`], [`sync`]; ADR-BDC-0015). The request is decoded
//! **before** the cache is opened, so a bad request never touches the store.
//!
//! [`Command`]: bd_cache_api::Command

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

use bd_cache_api::{ApiError, CacheFacts, Request, decode, execute};
use bd_cache_store::SCHEMA_VERSION;
use bd_cache_sync::{ComposeError, Composed, StopFlag, compose_with_config};
use sc_bd_cli::CliInvoker;
use sc_beads::Clock;
use serde_json::{Map, Value};

use crate::args::Target;
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
    /// Set by a signal (RSH-001, REQ-BDC-0730): `cron run`, `tick` and
    /// `sync use-python` stop on it.
    pub(crate) stop: &'a StopFlag,
}

impl std::fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("flags", &self.flags)
            .field("invoker", &self.invoker.as_ref().map(|i| i.binary()))
            .finish_non_exhaustive()
    }
}

/// Runs `target` with `request` and returns its `data`.
///
/// # Errors
///
/// The decoder's errors, then the configuration and open-time errors
/// (`CONFIG_INVALID`, `NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION`,
/// `STORE_FAILED`), then the command's own.
pub(crate) fn run(target: Target, request: &Value, ctx: Context<'_>) -> Result<Value, ApiError> {
    let (Target::Api(command) | Target::Sync(command)) = target;
    let decoded = decode(command, request)?;
    let settings = config::resolve(&ctx.flags, ctx.env)?;
    let mut composed = open(&settings, ctx.invoker, Arc::clone(&ctx.clock))?;
    match target {
        Target::Api(_) => execute(&mut composed.cache, &decoded, &facts(&settings))?.to_json(),
        Target::Sync(_) => run_sync(composed, &decoded, ctx.stop),
    }
}

/// Runs a phase-B verb over `bd-cache-sync` (ADR-BDC-0015).
fn run_sync(composed: Composed, req: &Request, stop: &StopFlag) -> Result<Value, ApiError> {
    match req {
        Request::ScheduleSet(r) => schedule::set(composed, r),
        Request::ScheduleShow(_) => schedule::show(composed),
        Request::ScheduleEnable(_) => schedule::enable(composed, true),
        Request::ScheduleDisable(_) => schedule::enable(composed, false),
        Request::ScheduleClear(_) => schedule::clear(composed),
        Request::CronRun(r) => cron::run(composed, r, stop),
        Request::CacheTick(_) => tick::run(composed, stop),
        Request::SyncUsePython(r) => sync::use_python(composed, r, stop),
        Request::SyncUseBuiltin(_) => sync::use_builtin(composed),
        other => Err(ApiError::internal(format!(
            "{} is not a front-end verb",
            other.command().path()
        ))),
    }
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
    }
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
