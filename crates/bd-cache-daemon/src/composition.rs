//! Application-owned adaptation of the pinned stores/routes/cleanup assembly.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::config::AppConfig;
use bd_cache_service::Application;
use sc_runtime::RuntimeError;
use sc_transport::{AppName, Endpoint};

/// Typed loopback listener policy refusal, before cache startup.
#[derive(Debug)]
pub struct ListenerPolicyError;
impl std::fmt::Display for ListenerPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("listener must use a loopback TCP address")
    }
}
impl std::error::Error for ListenerPolicyError {}

/// Application failure preserving runtime errors and nested transport causes.
#[derive(Debug)]
pub enum RunError {
    /// The pinned runtime failed; its typed sources remain intact.
    Runtime(RuntimeError),
    /// The endpoint is syntactically valid but outside the phase listener policy.
    ListenerPolicy(ListenerPolicyError),
    /// Application-owned cleanup failed after drain.
    Cleanup(bd_cache_service::ShutdownError),
}
impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line = match self {
            Self::Runtime(e) => crate::agent_error::runtime_error(e),
            Self::ListenerPolicy(e) => crate::agent_error::format_agent_error(
                "DAEMON.LISTENER_POLICY",
                e,
                "use tcp:127.0.0.1:<port> or tcp:[::1]:<port>",
            ),
            Self::Cleanup(e) => crate::agent_error::format_agent_error(
                "DAEMON.CLEANUP",
                e,
                "inspect worker shutdown diagnostics before restarting",
            ),
        };
        f.write_str(&line)
    }
}
impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Runtime(e) => e,
            Self::ListenerPolicy(e) => e,
            Self::Cleanup(e) => e,
        })
    }
}

/// Runs cache scheduling and HTTP, retaining ownership through both cleanup joins.
///
/// # Errors
/// Returns typed runtime/policy/cleanup failures without flattening their sources.
pub async fn run(
    config: &AppConfig,
    repo: &Path,
    name: &AppName,
    observability: &crate::observability::Observability,
) -> Result<(), RunError> {
    // Invalid syntax is deliberately left to the runtime's EndpointFailed adapter.
    // Freeze a valid resolution so environment cannot change after policy checking.
    let mut daemon = config.daemon.clone();
    if let Ok(endpoint) = sc_transport::resolve_endpoint_from_env(
        name,
        daemon.endpoint_flag.as_deref().map(OsStr::new),
        &daemon.transport,
    ) {
        if !matches!(endpoint, Endpoint::Tcp(address) if address.ip().is_loopback()) {
            return Err(RunError::ListenerPolicy(ListenerPolicyError));
        }
        daemon.endpoint_flag = Some(endpoint.to_string());
    }
    let repo = repo.to_owned();
    let binary = config.bd_binary.clone();
    let timeout = Duration::from_millis(config.shutdown_timeout_ms);
    let (owner_tx, owner_rx) = tokio::sync::oneshot::channel();
    let mut cleanup = Ok(());
    let result = sc_runtime::Daemon::builder(name, &daemon)
        .logger(observability.logger())
        .stores(move || async move {
            let composed = tokio::task::spawn_blocking(move || {
                bd_cache_sync::compose(&repo, &binary, Arc::new(sc_beads::SystemClock))
            })
            .await
            .map_err(|e| {
                RuntimeError::stores_failed(
                    "DAEMON.STARTUP",
                    "cache composition task failed",
                    "inspect startup diagnostics and retry",
                    Some(e.to_string()),
                )
            })?
            .map_err(|e| {
                RuntimeError::stores_failed(
                    e.code(),
                    "cache composition failed",
                    e.fix(),
                    Some(e.to_string()),
                )
            })?;
            let app = Application::start(composed, timeout).map_err(|e| map_start(&e))?;
            let state = app.state();
            let _ = owner_tx.send(app);
            Ok(state)
        })
        .routes(crate::routes::router)
        .run_with_cleanup(|| async {
            // The runtime drops unused stores closures on endpoint/lock failure.
            if let Ok(app) = owner_rx.await {
                cleanup = app.shutdown().await;
            }
        })
        .await;
    // Preserve a primary runtime failure; report secondary cleanup separately.
    if let Err(primary) = result {
        if let Err(secondary) = cleanup {
            eprintln!("{}", RunError::Cleanup(secondary));
        }
        return Err(RunError::Runtime(primary));
    }
    cleanup.map_err(RunError::Cleanup)
}

fn map_start(error: &bd_cache_service::StartError) -> RuntimeError {
    use bd_cache_service::StartError;
    match error {
        StartError::Cache(e) => RuntimeError::stores_failed(
            e.code(),
            "cache schedule startup failed",
            e.fix(),
            Some(error.to_string()),
        ),
        StartError::Request(e) => RuntimeError::stores_failed(
            "DAEMON.STARTUP",
            "health request failed",
            "report the invalid static request",
            Some(e.to_string()),
        ),
        StartError::Thread(e) => RuntimeError::stores_failed(
            "DAEMON.READINESS_START",
            "readiness thread failed",
            "inspect available process resources and retry",
            Some(e.to_string()),
        ),
    }
}
