//! Process-wide `sc-observability` logger for the generated daemon.
//!
//! `main` constructs [`Observability`] after config load and keeps it alive for
//! the whole process. Runtime and cron share one canonical handle. The caller
//! drains and drops both consumers before consuming the owner with `shutdown`.

use std::path::PathBuf;
use std::sync::Arc;

use sc_command::observability::{CanonicalLog, SharedCanonicalLog};
use sc_observability_types::{
    ErrorCode, ErrorContext, LogEvent, Remediation, ValueValidationError,
    v2::{EventError, InitError},
};

use sc_observability::{LogSettings, LoggerConfig, ServiceName};

/// Observability failed to start, retaining its concrete upstream cause.
#[derive(Debug)]
pub struct ObservabilityStart {
    cause: StartCause,
}

#[derive(Debug)]
enum StartCause {
    ServiceName {
        source: ValueValidationError,
        remediation: Remediation,
    },
    Logger(InitError),
}

impl ObservabilityStart {
    /// Returns the upstream diagnostic code, independent of the process envelope.
    #[must_use]
    pub fn code(&self) -> &ErrorCode {
        match &self.cause {
            StartCause::ServiceName { source, .. } => source.code(),
            // InitError::code() in the pinned release is a generic variant code;
            // the original diagnostic carries the operation-specific logger code.
            StartCause::Logger(source) => &source.diagnostic().code,
        }
    }

    /// Returns the original logger remediation or actionable service-name guidance.
    #[must_use]
    pub fn remediation(&self) -> &Remediation {
        match &self.cause {
            StartCause::ServiceName { remediation, .. } => remediation,
            StartCause::Logger(source) => &source.diagnostic().remediation,
        }
    }

    /// Returns the first recovery action for the bounded process diagnostic.
    #[must_use]
    pub fn recovery(&self) -> &str {
        match self.remediation() {
            Remediation::Recoverable { steps } => steps
                .first_step()
                .unwrap_or("fix logging configuration or sink paths and retry"),
            Remediation::NotRecoverable { justification } => justification,
        }
    }
}

impl From<ValueValidationError> for ObservabilityStart {
    fn from(source: ValueValidationError) -> Self {
        Self {
            cause: StartCause::ServiceName {
                source,
                // ValueValidationError supplies a code and message, but no remediation.
                remediation: Remediation::recoverable(
                    "use a non-empty service name containing only letters, digits, '.', '_' or '-'",
                    [] as [&str; 0],
                ),
            },
        }
    }
}

impl From<InitError> for ObservabilityStart {
    fn from(source: InitError) -> Self {
        Self {
            cause: StartCause::Logger(source),
        }
    }
}

impl std::fmt::Display for ObservabilityStart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.cause {
            StartCause::ServiceName { source, .. } => std::fmt::Display::fmt(source, formatter),
            StartCause::Logger(source) => std::fmt::Display::fmt(source, formatter),
        }
    }
}

impl std::error::Error for ObservabilityStart {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.cause {
            StartCause::ServiceName { source, .. } => source,
            StartCause::Logger(source) => source,
        })
    }
}

/// Running logger: file + console sinks under the resolved log root.
pub struct Observability {
    logger: Arc<sc_observability::v2::Logger>,
    shared: SharedCanonicalLog,
}

impl Observability {
    /// Builds the service logger for `app_name` using config and `log_root`.
    ///
    /// `log_root` is usually the transport instance root when
    /// `logging.log_root` is unset. File events land in
    /// `<log_root>/logs/<service>.log.jsonl`.
    ///
    /// # Errors
    ///
    /// Returns [`ObservabilityStart`] when service name or logger build fails.
    pub fn init(
        app_name: &str,
        logging: &LogSettings,
        log_root: PathBuf,
    ) -> Result<Self, ObservabilityStart> {
        let service = ServiceName::new(app_name).map_err(ObservabilityStart::from)?;
        let mut settings = LoggerConfig::default_for(service, log_root);
        if let Some(level) = logging.level {
            settings.level = level;
        }
        if let Some(enabled) = logging.enable_file_sink {
            settings.enable_file_sink = enabled;
        }
        // The released default console is stdout. The daemon reserves stdout
        // and registers the same public sink on stderr instead.
        let console = logging
            .enable_console_sink
            .unwrap_or(settings.enable_console_sink);
        settings.enable_console_sink = false;
        let mut builder =
            sc_observability::v2::LoggerBuilder::new(settings).map_err(ObservabilityStart::from)?;
        if console {
            builder.register_sink(sc_observability::v2::SinkRegistration::typed(Arc::new(
                sc_observability::ConsoleSink::stderr(),
            )));
        }
        let logger = builder.build().map_err(ObservabilityStart::from)?;
        let logger = Arc::new(logger);
        let shared: SharedCanonicalLog = Arc::new(LoggerAdapter {
            logger: Arc::clone(&logger),
        });
        Ok(Self { logger, shared })
    }

    /// Handle for library injection (`Daemon::builder().logger(...)`, cron scheduler).
    #[must_use]
    pub fn logger(&self) -> SharedCanonicalLog {
        Arc::clone(&self.shared)
    }

    /// Flushes and joins the logger after all injected consumers have dropped.
    ///
    /// # Errors
    /// Returns the production logger's sink or drain failure.
    pub fn shutdown(self) -> Result<(), sc_observability::v2::ShutdownError> {
        let Self { logger, shared } = self;
        drop(shared);
        logger.shutdown()
    }
}

// Application-only orphan-rule bridge. Remove after actual upstream capability:
// https://github.com/randlee/sc-runtime/issues/97 (ADR-RUN-0017).
struct LoggerAdapter {
    logger: Arc<sc_observability::v2::Logger>,
}

impl CanonicalLog for LoggerAdapter {
    fn service_name(&self) -> &ServiceName {
        self.logger.service_name()
    }

    fn emit(&self, event: LogEvent) -> Result<(), EventError> {
        // Only admission is contained. The logger remains owned by the app;
        // unwind cannot expose partially mutated adapter state. This does not
        // repair poison, catch abort, suppress panic hooks, or make locks wait-free.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.logger.try_log(event)))
        {
            Ok(result) => result,
            Err(_payload) => Err(admission_panicked()),
        }
    }
}

fn admission_panicked() -> EventError {
    EventError::Routing {
        context: Box::new(ErrorContext::new(
            ErrorCode::new_static("DAEMON.OBSERVABILITY_ADMISSION_PANIC"),
            "Logging admission panicked.",
            Remediation::recoverable(
                "Restart the daemon to restore logging; the application operation is unaffected.",
                [] as [&str; 0],
            ),
        )),
    }
}

#[cfg(test)]
#[path = "observability_adapter_tests.rs"]
pub(crate) mod adapter_tests;

impl std::fmt::Debug for Observability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observability").finish_non_exhaustive()
    }
}
