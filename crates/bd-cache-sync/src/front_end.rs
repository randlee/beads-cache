//! The front-end verbs of the contract over [`Schedules`], for every front
//! end (ADR-BDC-0015, ADR-SYNC-0008 as amended by sprint b-8).
//!
//! `bd-cache-api` decodes `cache schedule *`, `cache tick` and `cache sync *`
//! (so every front end refuses the same input with the same code) but cannot
//! run them: they need this crate. [`execute`] runs a decoded request, maps
//! the request to this crate's inputs ([`ScheduleSet::of`],
//! [`UsePython::of`]) and every error to the contract's error once
//! (`ApiError::from` over the typed [`CacheError`]; a tick refused in mode
//! Python is `PRECONDITION` with [`PreconditionReason::PythonSyncEnabled`]).
//! A front end only says which it is ([`FrontEnd`]), which picks the wording
//! of the suggested action: `bdc`'s commands or the `PyO3` methods. The
//! details of an error are the same for both. `cache cron run` is the CLI's
//! alone (signals, shutdown) and runs [`Schedules::cron_run`] itself.

use bd_cache_api::{ApiError, Request};
use bd_cache_model::{CacheError, CacheStore, DrainLock, PreconditionReason, ScheduleReason};
use sc_beads::CacheBackend;
use sc_cron::{Clock as CronClock, JitterSource, ScheduleStore};
use serde_json::Value;

use crate::control::StopFlag;
use crate::schedule::{ScheduleSet, Schedules, UsePython};

/// Which front end runs a verb: it picks the wording of suggested actions only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontEnd {
    /// `bdc` (`bd-cache-cli`).
    Cli,
    /// `bd_cache_rs` (`bd-cache-py`).
    Python,
}

impl FrontEnd {
    fn use_builtin(self) -> &'static str {
        match self {
            Self::Cli => "bdc cache sync use-builtin",
            Self::Python => "cache.sync_use_builtin()",
        }
    }

    fn schedule_set(self) -> &'static str {
        match self {
            Self::Cli => "bdc cache schedule set <cron-expr>",
            Self::Python => "cache.schedule_set(cron)",
        }
    }

    /// This front end's suggested action for `e`, when it words one itself;
    /// `None` keeps the contract table's (`CacheError::fix`).
    #[must_use]
    pub fn suggested_action(self, e: &CacheError) -> Option<String> {
        match e {
            CacheError::Precondition {
                reason: PreconditionReason::PythonSyncEnabled,
                ..
            } => Some(format!(
                "{} makes the built-in row the sync again",
                self.use_builtin()
            )),
            CacheError::Precondition {
                reason: PreconditionReason::InsideRun,
                ..
            } => Some(
                "call it outside the sync run or post-sync hook: the run's lease and schedule are the caller's"
                    .to_owned(),
            ),
            CacheError::ScheduleInvalid { reason, .. } => match reason {
                ScheduleReason::NoBuiltinRow => Some(format!(
                    "create the built-in row with {}",
                    self.schedule_set()
                )),
                ScheduleReason::PythonSyncEnabled => {
                    Some(format!("switch with {}", self.use_builtin()))
                }
                _ => match self {
                    Self::Cli => None,
                    Self::Python => Some(
                        "fix the named argument; switch rows with cache.sync_use_python() or cache.sync_use_builtin()"
                            .to_owned(),
                    ),
                },
            },
            CacheError::TickRunFailed { .. } => match self {
                Self::Cli => None,
                Self::Python => Some(
                    e.fix()
                        .replace("bdc cache sync use-builtin", self.use_builtin())
                        .replace("--timeout-ms", "timeout_ms")
                        .replace("--python", "python"),
                ),
            },
            _ => None,
        }
    }

    /// `e` as the contract's error, with this front end's suggested action.
    #[must_use]
    pub fn api_error(self, e: &CacheError) -> ApiError {
        let out = ApiError::from(e);
        match self.suggested_action(e) {
            Some(action) => out.with_suggested_action(action),
            None => out,
        }
    }
}

fn json<T: serde::Serialize>(v: &T) -> Result<Value, ApiError> {
    serde_json::to_value(v)
        .map_err(|e| ApiError::internal(format!("the reply does not serialize: {e}")))
}

/// Runs a decoded front-end verb over `schedules` (module docs); `stop` is
/// the flag a signal sets (it stops a tick and kills the immediate run of
/// `use-python`, which then restores).
///
/// # Errors
///
/// The verb's error as the contract's, worded for `front`; `INTERNAL` for a
/// request that is not a front-end verb (or `cron run`, which the CLI runs).
pub async fn execute<St, S, L, B, C, J>(
    schedules: &Schedules<St, S, L, B, C, J>,
    req: &Request,
    stop: &StopFlag,
    front: FrontEnd,
) -> Result<Value, ApiError>
where
    St: ScheduleStore + 'static,
    S: CacheStore + 'static,
    L: DrainLock + 'static,
    B: CacheBackend + 'static,
    C: CronClock + 'static,
    J: JitterSource + 'static,
{
    let err = |e: &CacheError| front.api_error(e);
    match req {
        Request::ScheduleSet(r) => json(
            &schedules
                .set(&ScheduleSet::of(r))
                .await
                .map_err(|e| err(&e))?,
        ),
        Request::ScheduleShow(_) => json(&schedules.show().await.map_err(|e| err(&e))?),
        Request::ScheduleEnable(_) => json(&schedules.enable(true).await.map_err(|e| err(&e))?),
        Request::ScheduleDisable(_) => json(&schedules.enable(false).await.map_err(|e| err(&e))?),
        Request::ScheduleClear(_) => json(&schedules.clear().await.map_err(|e| err(&e))?),
        Request::SyncUseBuiltin(_) => json(&schedules.use_builtin().await.map_err(|e| err(&e))?),
        Request::SyncUsePython(r) => json(
            &schedules
                .use_python(&UsePython::of(r), stop.clone())
                .await
                .map_err(|e| err(&e))?,
        ),
        Request::CacheTick(_) => json(
            &schedules
                .tick(stop.clone())
                .await
                .map_err(|e| err(&e.into_cache_error()))?,
        ),
        other => Err(ApiError::internal(format!(
            "{} is not a front-end verb run here",
            other.command().path()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both front ends give the same code and details for a tick refused in
    /// mode Python; only the suggested action is worded per front end
    /// (B8Q1-022). Planted negative: another precondition keeps the
    /// contract's fix and no sync reason.
    #[test]
    fn both_front_ends_give_identical_details_for_python_sync_enabled() {
        let e = crate::schedule::SyncError::PythonSyncEnabled.into_cache_error();
        let (cli, py) = (FrontEnd::Cli.api_error(&e), FrontEnd::Python.api_error(&e));
        assert_eq!(cli.code(), "BD_CACHE.PRECONDITION");
        assert_eq!(cli.code(), py.code());
        assert_eq!(cli.details(), py.details());
        assert_eq!(
            cli.details().get("reason").and_then(Value::as_str),
            Some("python_sync_enabled")
        );
        assert!(
            cli.suggested_action()
                .is_some_and(|a| a.contains("bdc cache sync use-builtin"))
        );
        assert!(
            py.suggested_action()
                .is_some_and(|a| a.contains("cache.sync_use_builtin()"))
        );
        let other = CacheError::Precondition {
            detail: "x".into(),
            unmet: std::collections::BTreeMap::new(),
            reason: PreconditionReason::PreconditionFailed,
            source: None,
        };
        let plain = FrontEnd::Python.api_error(&other);
        assert_eq!(plain.details().get("reason"), None);
        assert_eq!(plain.suggested_action(), Some(other.fix()));
    }
}
