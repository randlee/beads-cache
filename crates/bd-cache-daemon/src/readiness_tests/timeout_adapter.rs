//! Explicit timeout inputs for the production router in owned test fixtures.
//!
//! Replaces the removed upstream test seam without a production configuration
//! change. Only the first readiness request is controlled; later probes exercise
//! ordinary busy/closing admission. The caller expires it after actual reader
//! admission and a concurrent busy response, so elapsed wall time never orders
//! cancellation. Real runtime serving, signals, cleanup and locks remain upstream.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    extract::Request,
    middleware::{self, Next},
    response::IntoResponse,
};
use sc_command::{Envelope, ErrorKind, OpError};
use tokio::sync::watch;

pub(super) fn instrument_router(
    router: axum::Router,
    expiration: watch::Receiver<bool>,
    shutdown: watch::Receiver<bool>,
) -> axum::Router {
    let first = Arc::new(AtomicBool::new(true));
    router.layer(middleware::from_fn(move |request: Request, next: Next| {
        let controlled = request.uri().path() == "/ready" && first.swap(false, Ordering::AcqRel);
        let mut expiration = expiration.clone();
        let mut shutdown = shutdown.clone();
        async move {
            if !controlled {
                return next.run(request).await;
            }
            let handler = next.run(request);
            tokio::pin!(handler);
            let draining = tokio::select! {
                biased;
                response = &mut handler => return response,
                () = signaled(&mut expiration) => false,
                () = signaled(&mut shutdown) => true,
            };
            if draining {
                // Shutdown starts tracking drain; it does not drop the handler.
                // The same explicit expiration input bounds that drain.
                tokio::select! {
                    biased;
                    response = &mut handler => return response,
                    () = signaled(&mut expiration) => {},
                }
            }
            let (code, message, recovery) = if draining {
                ("RUNTIME.DRAIN_TIMEOUT", "the handler exceeded the shutdown drain time", "completion may be unknown; after serving resumes, reconcile the operation outcome before considering a replay")
            } else {
                ("RUNTIME.REQUEST_TIMEOUT", "the handler exceeded its time limit", "completion may be unknown; reconcile the operation outcome before considering a replay")
            };
            tracing::info!(code, draining, "fixture handler expired");
            Envelope::<()>::failure(
                OpError::from_code(ErrorKind::Unavailable, code, message).with_suggestion(recovery),
            ).into_response()
            // Returning drops the actual handler future; it does not cancel
            // or detach the application-owned synchronous readiness work.
        }
    }))
}

async fn signaled(receiver: &mut watch::Receiver<bool>) {
    while !*receiver.borrow_and_update() {
        if receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
