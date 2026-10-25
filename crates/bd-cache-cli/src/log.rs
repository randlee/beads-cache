//! The stderr log (contract §2): `-v` / `-q` win over `BD_CACHE__LOG`; stdout never changes.
//!
//! The libraries log through `tracing` (ADR-BDC-0007). A command runs under
//! a [`StderrLog`] subscriber installed for its thread only, so an in-process
//! [`crate::run`] does not change any global state. The level is `warn` by
//! default; `-q` keeps errors only, `-v` adds `info`, `-vv` `debug` and
//! `-vvv` `trace`. `BD_CACHE__LOG` takes `off`, `error`, `warn`, `info`,
//! `debug` or `trace`.

use std::fmt::Write as _;
use std::io::Write as _;

use sc_beads::logging::tracing;
use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// The default level.
const DEFAULT: LevelFilter = LevelFilter::WARN;

/// The level from the flags (`-v` count, `-q`) and `BD_CACHE__LOG`.
///
/// A flag wins over the env; an env value that is not a level is ignored
/// with the default (the env never makes a command fail).
#[must_use]
pub(crate) fn level(verbose: u8, quiet: bool, env: Option<&str>) -> LevelFilter {
    if quiet {
        return LevelFilter::ERROR;
    }
    match verbose {
        0 => env.and_then(parse).unwrap_or(DEFAULT),
        1 => LevelFilter::INFO,
        2 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    }
}

fn parse(text: &str) -> Option<LevelFilter> {
    match text.trim().to_ascii_lowercase().as_str() {
        "off" => Some(LevelFilter::OFF),
        "error" => Some(LevelFilter::ERROR),
        "warn" => Some(LevelFilter::WARN),
        "info" => Some(LevelFilter::INFO),
        "debug" => Some(LevelFilter::DEBUG),
        "trace" => Some(LevelFilter::TRACE),
        _ => None,
    }
}

/// A `tracing` subscriber that writes each event as one stderr line:
/// `bdc: LEVEL target: message key=value ...`. Spans are not tracked.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StderrLog {
    max: LevelFilter,
}

impl StderrLog {
    /// A log that keeps events at `max` and below.
    #[must_use]
    pub(crate) fn new(max: LevelFilter) -> Self {
        Self { max }
    }
}

/// Collects an event's fields: `message` first, the rest as `key=value`.
#[derive(Default)]
struct Line {
    message: String,
    fields: String,
}

impl Visit for Line {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.fields, " {}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.fields, " {}={value:?}", field.name());
        }
    }
}

impl Subscriber for StderrLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        *metadata.level() <= self.max
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(self.max)
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let meta = event.metadata();
        let mut line = Line::default();
        event.record(&mut line);
        let level = match *meta.level() {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };
        let text = format!(
            "bdc: {level} {}: {}{}\n",
            meta.target(),
            line.message,
            line.fields
        );
        // A closed stderr leaves nobody to tell; stdout still carries the reply.
        let _written = std::io::stderr().write_all(text.as_bytes());
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flag_wins_over_the_env_and_the_default_is_warn() {
        assert_eq!(level(0, false, None), LevelFilter::WARN);
        assert_eq!(level(0, false, Some("debug")), LevelFilter::DEBUG);
        assert_eq!(level(1, false, Some("off")), LevelFilter::INFO);
        assert_eq!(level(0, true, Some("trace")), LevelFilter::ERROR);
        assert_eq!(level(3, false, None), LevelFilter::TRACE);
        assert_eq!(level(0, false, Some("loud")), LevelFilter::WARN);
    }

    #[test]
    fn the_subscriber_keeps_only_events_at_its_level() {
        let log = StderrLog::new(LevelFilter::WARN);
        assert_eq!(log.max_level_hint(), Some(LevelFilter::WARN));
        assert!(Level::ERROR <= log.max && Level::WARN <= log.max);
        assert!(Level::INFO > log.max);
    }
}
