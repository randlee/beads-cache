//! Gated logging: the two runtime switches and the `log_*!` macros.
//!
//! Library code logs through `tracing` (ADR-BDC-0007); this module gates those calls
//! on [`LOGGING_ENABLED`] (and, for `log_debug!`, also [`VERBOSE_LOGGING`]). It does
//! not install or wrap a logger: the app owns the bridge. The app re-exports both
//! statics so its `get_/set_logging_enabled` and `get_/set_verbose_logging` commands
//! flip the same switches every backend crate reads.

use std::sync::atomic::AtomicBool;

/// Re-export of `tracing` so the exported macros resolve it through
/// `$crate` in every calling crate.
pub use tracing;

/// When `false` (the default), every `log_*!` call is skipped.
pub static LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

/// When `true` together with [`LOGGING_ENABLED`], `log_debug!` calls are emitted.
pub static VERBOSE_LOGGING: AtomicBool = AtomicBool::new(false);

/// `tracing::info!` gated on [`LOGGING_ENABLED`](crate::logging::LOGGING_ENABLED).
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        if $crate::logging::LOGGING_ENABLED.load(::std::sync::atomic::Ordering::Relaxed) {
            $crate::logging::tracing::info!($($arg)*);
        }
    };
}

/// `tracing::warn!` gated on [`LOGGING_ENABLED`](crate::logging::LOGGING_ENABLED).
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        if $crate::logging::LOGGING_ENABLED.load(::std::sync::atomic::Ordering::Relaxed) {
            $crate::logging::tracing::warn!($($arg)*);
        }
    };
}

/// `tracing::error!` gated on [`LOGGING_ENABLED`](crate::logging::LOGGING_ENABLED).
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        if $crate::logging::LOGGING_ENABLED.load(::std::sync::atomic::Ordering::Relaxed) {
            $crate::logging::tracing::error!($($arg)*);
        }
    };
}

/// `tracing::debug!` gated on both [`LOGGING_ENABLED`](crate::logging::LOGGING_ENABLED)
/// and [`VERBOSE_LOGGING`](crate::logging::VERBOSE_LOGGING).
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::logging::LOGGING_ENABLED.load(::std::sync::atomic::Ordering::Relaxed) && $crate::logging::VERBOSE_LOGGING.load(::std::sync::atomic::Ordering::Relaxed) {
            $crate::logging::tracing::debug!($($arg)*);
        }
    };
}
