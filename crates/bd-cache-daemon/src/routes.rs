//! Read-only readiness adapter; no domain operation HTTP surface is mounted.

use axum::extract::{Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::{Json, http::StatusCode};
use bd_cache_service::{ReadyError, ReadyState};
use sc_command::{Envelope, ErrorKind, OpError};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::Instrument;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Application router. Runtime adds its reserved liveness and `OpenAPI` routes.
pub fn router(state: ReadyState) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(ready_route))
        .route_layer(middleware::from_fn(request_id))
        .with_state(state)
}

#[utoipa::path(get, path = "/ready", responses((status = 200, description = "Cache health is ok"), (status = 503, description = "Degraded health or unavailable readiness")))]
async fn ready_route(
    State(state): State<ReadyState>,
) -> (StatusCode, Json<Envelope<serde_json::Value>>) {
    match state.ready().await {
        Ok(data) => {
            let data = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
            let healthy = data.get("state").and_then(serde_json::Value::as_str) == Some("ok");
            (
                if healthy {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                Json(Envelope::success(data)),
            )
        }
        Err(error) => {
            let error = match error {
                ReadyError::Busy | ReadyError::Closing => {
                    let closing = matches!(error, ReadyError::Closing);
                    OpError::from_code(
                        ErrorKind::Unavailable,
                        "DAEMON.READINESS_BUSY",
                        "readiness capacity or cache is busy",
                    )
                    .with_suggestion(if closing {
                        "retry after the daemon restarts"
                    } else {
                        "retry the readiness probe shortly"
                    })
                }
                ReadyError::Cache(e) => {
                    // Deserialize the shared error contract, preserving code/cause/details.
                    serde_json::to_value(&e)
                        .and_then(serde_json::from_value::<OpError>)
                        .unwrap_or_else(|_| {
                            OpError::from_code(
                                ErrorKind::Internal,
                                "DAEMON.READINESS_SERIALIZE",
                                "health error could not be serialized",
                            )
                        })
                        .with_suggestion(if e.code() == "BD_CACHE.STORE_FAILED" {
                            "retry and inspect cache contention if persistent"
                        } else {
                            e.suggested_action()
                                .unwrap_or("inspect cache health and retry")
                        })
                }
            };
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Envelope::failure(error)),
            )
        }
    }
}

static NEXT: AtomicU64 = AtomicU64::new(1);

async fn request_id(request: Request, next: Next) -> Response {
    let supplied = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| {
            (1..=128).contains(&s.len())
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        });
    let id = supplied.map_or_else(
        || format!("req-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
        str::to_owned,
    );
    let span = tracing::info_span!("request", request_id = %id);
    let mut response = next.run(request).instrument(span).await;
    if let Ok(value) = axum::http::HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}
