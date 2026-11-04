//! Foreground scheduler host using the pinned sc-runtime lifecycle.
//!
//! Runtime owns `/health`, instance locking, HTTP draining and signal handling.
//! The application adds only read-only `/ready` and the existing cache worker.

pub mod agent_error;
pub mod composition;
pub mod config;
pub mod observability;
mod routes;

/// Application identity used by runtime and logging.
pub const APP_NAME: &str = "bd-cache";

#[cfg(test)]
mod readiness_tests;
