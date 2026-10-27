//! The stderr log (contract §2): `-v` / `-q` win over `BD_CACHE__LOG`; stdout never changes.
//!
//! The libraries log through `tracing` (ADR-BDC-0007). A command runs under
//! a [`StderrLog`] subscriber installed for its thread only, so an in-process
//! [`crate::run`] does not change any global state. The level is `warn` by
//! default, and `info` for `cache cron run`, a long-running service whose
//! runs and steps are its heartbeat (REQ-BDC-0730; a service unit runs it as
//! is, or with `-q`); `-q` keeps errors only, `-v` adds `info`, `-vv`
//! `debug` and `-vvv` `trace`. `BD_CACHE__LOG` takes `off`, `error`,
//! `warn`, `info`, `debug` or `trace`.
//!
//! Each event is one line: control characters in the message and the field
//! values (a child's stderr in a recorded outcome) are escaped, so no value
//! can break a line or forge another.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::io::Write as _;

use sc_beads::logging::tracing;
use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// The default level of a one-shot command.
const DEFAULT: LevelFilter = LevelFilter::WARN;

/// The default level of `command`: `info` for `cache cron run`, else `warn`.
#[must_use]
pub(crate) fn default_for(command: bd_cache_api::Command) -> LevelFilter {
    if command == bd_cache_api::Command::CronRun {
        LevelFilter::INFO
    } else {
        DEFAULT
    }
}

/// The level from the flags (`-v` count, `-q`), `BD_CACHE__LOG` and the
/// command's `default`.
///
/// A flag wins over the env; an env value that is not a level is ignored
/// with the default (the env never makes a command fail).
#[must_use]
pub(crate) fn level(
    verbose: u8,
    quiet: bool,
    env: Option<&str>,
    default: LevelFilter,
) -> LevelFilter {
    if quiet {
        return LevelFilter::ERROR;
    }
    match verbose {
        0 => env.and_then(parse).unwrap_or(default),
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

/// `text` with every control character escaped (`\n`, `\r`, `\t`,
/// `\u{..}`): one line whatever it holds; borrowed when there is none.
fn escape(text: &str) -> Cow<'_, str> {
    if !text.chars().any(char::is_control) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

impl Visit for Line {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            // A message is `format_args!` text: written as is, then escaped.
            let text = format!("{value:?}");
            self.message.push_str(&escape(&text));
        } else {
            // `Display` values (`%x`) reach here too: escape what they wrote.
            let text = format!("{value:?}");
            let _ = write!(self.fields, " {}={}", field.name(), escape(&text));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(&escape(value));
        } else {
            // `Debug` quotes the text and escapes its control characters.
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
        assert_eq!(level(0, false, None, DEFAULT), LevelFilter::WARN);
        assert_eq!(level(0, false, Some("debug"), DEFAULT), LevelFilter::DEBUG);
        assert_eq!(level(1, false, Some("off"), DEFAULT), LevelFilter::INFO);
        assert_eq!(level(0, true, Some("trace"), DEFAULT), LevelFilter::ERROR);
        assert_eq!(level(3, false, None, DEFAULT), LevelFilter::TRACE);
        assert_eq!(level(0, false, Some("loud"), DEFAULT), LevelFilter::WARN);
    }

    /// `cron run` logs at `info` by default (its runs are its heartbeat);
    /// planted negatives: `-q` still keeps errors only, and any other
    /// command stays at `warn` (B8Q1-052).
    #[test]
    fn cron_run_logs_at_info_by_default() {
        use bd_cache_api::Command;
        let cron = default_for(Command::CronRun);
        assert_eq!(level(0, false, None, cron), LevelFilter::INFO);
        assert_eq!(level(0, true, None, cron), LevelFilter::ERROR);
        assert_eq!(default_for(Command::CacheTick), LevelFilter::WARN);
    }

    /// A value with a newline cannot break the line or forge another
    /// (B8Q1-051); planted negative: plain text is kept as it is.
    #[test]
    fn control_characters_are_escaped() {
        assert_eq!(escape("one\nbdc: ERROR forged"), "one\\nbdc: ERROR forged");
        assert_eq!(escape("a\rb\u{1b}c"), "a\\rb\\u{1b}c");
        assert!(matches!(escape("plain text"), Cow::Borrowed("plain text")));
        let mut line = Line::default();
        line.record_str(&FieldStub::message(), "first\nsecond");
        assert!(!line.message.contains('\n'), "{}", line.message);
    }

    /// A `Field` named `message`, from a real callsite.
    struct FieldStub;

    impl FieldStub {
        fn message() -> Field {
            struct Cs;
            static CS: Cs = Cs;
            static META: tracing::Metadata<'static> = tracing::metadata! {
                name: "stub",
                target: "bdc",
                level: Level::INFO,
                fields: &["message"],
                callsite: &CS,
                kind: tracing::metadata::Kind::EVENT,
            };
            impl tracing::callsite::Callsite for Cs {
                fn set_interest(&self, _: tracing::subscriber::Interest) {}
                fn metadata(&self) -> &tracing::Metadata<'_> {
                    &META
                }
            }
            match META.fields().field("message") {
                Some(f) => f,
                None => unreachable_field(),
            }
        }
    }

    #[expect(clippy::panic, reason = "test code: the stub callsite names its field")]
    fn unreachable_field() -> Field {
        panic!("the stub callsite has a message field")
    }

    #[test]
    fn the_subscriber_keeps_only_events_at_its_level() {
        let log = StderrLog::new(LevelFilter::WARN);
        assert_eq!(log.max_level_hint(), Some(LevelFilter::WARN));
        assert!(Level::ERROR <= log.max && Level::WARN <= log.max);
        assert!(Level::INFO > log.max);
    }
}
