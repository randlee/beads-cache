//! Stable agent-facing process failures and shutdown result precedence.

/// Formats a secret-free cause with its stable code and recovery.
#[must_use]
pub fn format_agent_error(code: &str, cause: impl std::fmt::Display, recovery: &str) -> String {
    format!(
        "{} {} — {}",
        bounded(code, 128),
        bounded(&cause.to_string(), 2048),
        bounded(recovery, 1024)
    )
}

/// Formats runtime fields once, retaining the library's actionable recovery.
#[must_use]
pub fn runtime_error(error: &sc_runtime::RuntimeError) -> String {
    let cause = match error.cause() {
        Some(cause) => format!("{}: {cause}", error.message()),
        None => error.message().to_owned(),
    };
    format_agent_error(error.code(), cause, error.remediation())
}

/// Formats transport facts without embedding its Display code or recovery.
#[must_use]
pub fn transport_error(error: &sc_transport::TransportError) -> String {
    use sc_transport::TransportError;
    let cause = match error {
        TransportError::InvalidName { name, detail, .. } => format!("{name}: {detail}"),
        TransportError::InvalidEndpoint { source, detail, .. } => format!("{source}: {detail}"),
        TransportError::MissingTcpPort { detail, .. } => (*detail).to_owned(),
        TransportError::InstanceRootUnavailable { detail, .. } => detail.clone(),
        TransportError::ResponseDecode {
            status,
            cause,
            body_preview,
            ..
        } => {
            if body_preview.is_empty() {
                format!("http {status}: {cause}")
            } else {
                format!("http {status}: {cause}: {body_preview}")
            }
        }
        TransportError::DaemonNotRunning {
            endpoint, cause, ..
        }
        | TransportError::TimedOut {
            endpoint, cause, ..
        }
        | TransportError::Request {
            endpoint, cause, ..
        }
        | TransportError::Bind {
            endpoint, cause, ..
        }
        | TransportError::EndpointBusy {
            endpoint, cause, ..
        }
        | TransportError::NotASocket {
            endpoint, cause, ..
        }
        | TransportError::UnsupportedEndpoint {
            endpoint, cause, ..
        } => format!("{endpoint}: {cause}"),
        _ => std::error::Error::source(error).map_or_else(
            || "transport operation failed".to_owned(),
            ToString::to_string,
        ),
    };
    format_agent_error(error.code(), cause, error.recovery())
}

/// Preserves an operation failure and reports a logger shutdown failure second.
///
/// The caller has already drained and dropped runtime and scheduler consumers.
/// `shutdown` is the result of consuming the observability owner.
///
/// # Errors
/// Returns the operation failure, or shutdown failure if the operation succeeded.
pub fn finish(
    primary: Result<(), String>,
    shutdown: Result<(), sc_observability::v2::ShutdownError>,
) -> Result<(), String> {
    let shutdown = shutdown.map_err(|cause| {
        format_agent_error(
            "DAEMON.OBSERVABILITY_SHUTDOWN",
            cause,
            "inspect sink health and retry shutdown",
        )
    });
    match (primary, shutdown) {
        (Err(primary), Err(secondary)) => {
            eprintln!("{secondary}");
            Err(primary)
        }
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), shutdown) => shutdown,
    }
}

fn bounded(text: &str, limit: usize) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
            .chars()
            .collect::<Vec<_>>()
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "test assertions")]
    use super::{finish, format_agent_error};
    use sc_observability_types::{ErrorCode, ErrorContext, Remediation, v2::ShutdownError};

    fn shutdown_failure() -> ShutdownError {
        ShutdownError::Drain {
            context: Box::new(ErrorContext::new(
                ErrorCode::new_static("TEST.DRAIN"),
                "sink drain failed",
                Remediation::not_recoverable("inspect sink"),
            )),
        }
    }

    #[test]
    fn structured_runtime_and_transport_errors_keep_exact_library_recovery() {
        let error = sc_runtime::RuntimeError::stores_failed(
            "STORE.OPEN",
            "opening application stores failed",
            "repair the database path",
            Some("permission denied".to_owned()),
        );
        assert_eq!(
            super::runtime_error(&error),
            "STORE.OPEN opening application stores failed: permission denied — repair the database path"
        );
        let error = sc_runtime::RuntimeError::stores_failed(
            "STORE.OPEN",
            "opening stores failed",
            "repair stores",
            None,
        );
        assert_eq!(
            super::runtime_error(&error),
            "STORE.OPEN opening stores failed — repair stores"
        );
        let error = "invalid-endpoint"
            .parse::<sc_transport::Endpoint>()
            .expect_err("endpoint rejected");
        assert_eq!(
            super::transport_error(&error),
            "TRANSPORT.INVALID_ENDPOINT invalid-endpoint: endpoint must start with uds: or tcp: — Use uds:<path> or tcp:<socket-addr>."
        );
    }

    #[test]
    fn operation_remains_primary_and_shutdown_only_fails_success() {
        let primary = format_agent_error(
            "RUNTIME.BIND_FAILED",
            "endpoint occupied",
            "read the cause and retry",
        );
        assert_eq!(finish(Err(primary.clone()), Ok(())), Err(primary.clone()));
        assert_eq!(
            finish(Err(primary.clone()), Err(shutdown_failure())),
            Err(primary)
        );
        let error =
            finish(Ok(()), Err(shutdown_failure())).expect_err("shutdown must fail process");
        assert!(error.starts_with("DAEMON.OBSERVABILITY_SHUTDOWN "));
        assert!(error.contains("sink drain failed"));
        assert!(error.ends_with(" — inspect sink health and retry shutdown"));
        assert_eq!(finish(Ok(()), Ok(())), Ok(()));
    }
}
