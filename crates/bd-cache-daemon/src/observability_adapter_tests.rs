#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "inherited scaffold test assertions"
)]
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use super::LoggerAdapter;
use sc_command::observability::CanonicalLog;
use sc_observability::v2::{LogSink, Logger, LoggerConfig, SinkRegistration};
use sc_observability_types::{
    ActionName, CorrelationId, Level, LogEvent, ProcessIdentity, SchemaVersion, ServiceName,
    SinkHealth, SinkHealthState, SinkName, TargetCategory, Timestamp,
    constants::OBSERVATION_ENVELOPE_VERSION, v2::LogSinkError,
};

const WAIT: Duration = Duration::from_secs(10);

fn event(service: ServiceName) -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service,
        target: TargetCategory::new("http.server").expect("target"),
        action: ActionName::new("request.start").expect("action"),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: Some(CorrelationId::new("request-123").expect("id")),
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}

fn health() -> SinkHealth {
    SinkHealth {
        name: SinkName::new("adapter-test").expect("sink name"),
        state: SinkHealthState::Healthy,
        last_error: None,
    }
}

struct RecordingSink(Mutex<Vec<LogEvent>>);
impl LogSink for RecordingSink {
    fn write(&self, event: &LogEvent) -> Result<(), LogSinkError> {
        self.0.lock().expect("recording lock").push(event.clone());
        Ok(())
    }
    fn health(&self) -> SinkHealth {
        health()
    }
}

fn logger(root: &std::path::Path, sink: Arc<dyn LogSink>, capacity: usize) -> Arc<Logger> {
    let name = ServiceName::new("adapter-service").expect("service");
    let mut config = LoggerConfig::default_for(name, root.to_path_buf());
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    config.queue_capacity = capacity;
    let mut builder = Logger::builder(config).expect("builder");
    builder.register_sink(SinkRegistration::typed(sink));
    Arc::new(builder.build().expect("logger"))
}

#[test]
#[allow(
    deprecated,
    reason = "the pinned 1.5.0 shutdown diagnostic is the contract under test"
)]
fn forwards_real_events_and_native_shutdown_error() {
    let root = tempfile::tempdir().expect("temp root");
    let sink = Arc::new(RecordingSink(Mutex::new(Vec::new())));
    let logger = logger(root.path(), Arc::<RecordingSink>::clone(&sink), 8);
    let adapter = LoggerAdapter {
        logger: Arc::clone(&logger),
    };
    assert_eq!(adapter.service_name(), logger.service_name());
    let original = event(adapter.service_name().clone());
    adapter.emit(original.clone()).expect("admitted");
    logger.flush().expect("owner flush");
    let events = sink.0.lock().expect("recording lock");
    assert_eq!(events.len(), 1);
    assert_eq!(
        serde_json::to_value(&events[0]).expect("wire"),
        serde_json::to_value(original).expect("wire")
    );
    drop(events);
    logger.shutdown().expect("owner shutdown");
    let emitted = adapter
        .emit(event(adapter.service_name().clone()))
        .expect_err("closed");
    let direct = logger
        .try_log(event(logger.service_name().clone()))
        .expect_err("closed");
    assert_eq!(emitted.diagnostic().code, direct.diagnostic().code);
    assert_eq!(
        emitted.diagnostic().code,
        sc_observability::error_codes::LOGGER_SHUTDOWN
    );
}

struct BlockingSink {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}
impl LogSink for BlockingSink {
    fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
        let _ = self.entered.send(());
        let _ = self.release.lock().expect("release lock").recv();
        Ok(())
    }
    fn health(&self) -> SinkHealth {
        health()
    }
}

// Each adversarial fixture owns only Rust threads, so killing its process
// bounds every admission, sink callback, shutdown and thread join together.
struct FixtureChild(Child);

impl Drop for FixtureChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn run_fixture_child(name: &str, deadline: Duration) -> Result<(), String> {
    let marker = tempfile::tempdir().map_err(|_error| "fixture marker directory failed")?;
    let done = marker.path().join("completed");
    let mut child = FixtureChild(
        Command::new(std::env::current_exe().map_err(|_error| "test executable unavailable")?)
            .args(["--exact", name, "--nocapture"])
            .env("SC_ADAPTER_FIXTURE", name)
            .env("SC_ADAPTER_FIXTURE_DONE", &done)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_error| "fixture child could not start")?,
    );
    let until = Instant::now() + deadline;
    loop {
        match child
            .0
            .try_wait()
            .map_err(|_error| "fixture status inspection failed")?
        {
            Some(status) => {
                if !status.success() {
                    return Err("fixture child failed".into());
                }
                if std::fs::read(&done).ok().as_deref() != Some(b"completed") {
                    return Err("fixture did not execute its exact test".into());
                }
                return Ok(());
            }
            None if Instant::now() >= until => {
                child
                    .0
                    .kill()
                    .map_err(|_error| "fixture timeout: child kill failed")?;
                child
                    .0
                    .wait()
                    .map_err(|_error| "fixture timeout: child reap failed")?;
                return Err("fixture timed out; child killed and reaped".into());
            }
            None => std::thread::sleep(Duration::from_millis(10)), // allow: test polls its owned child
        }
    }
}

fn bounded_fixture(name: &str, scenario: fn()) {
    if std::env::var("SC_ADAPTER_FIXTURE").ok().as_deref() == Some(name) {
        scenario();
        let done = std::env::var_os("SC_ADAPTER_FIXTURE_DONE").expect("completion marker");
        std::fs::write(done, b"completed").expect("fixture completion");
    } else {
        run_fixture_child(name, WAIT).expect("bounded real logger fixture");
    }
}

#[test]
fn fixture_timeout_kills_and_reaps_child() {
    const NAME: &str = "observability::adapter_tests::fixture_timeout_kills_and_reaps_child";
    if std::env::var("SC_ADAPTER_FIXTURE").ok().as_deref() == Some(NAME) {
        loop {
            std::thread::park();
        }
    }
    let error = run_fixture_child(NAME, Duration::from_millis(100)).unwrap_err();
    assert_eq!(error, "fixture timed out; child killed and reaped");
}

#[test]
fn saturated_real_queue_rejects_without_waiting_for_sink() {
    bounded_fixture(
        "observability::adapter_tests::saturated_real_queue_rejects_without_waiting_for_sink",
        saturation_scenario,
    );
}

fn saturation_scenario() {
    let root = tempfile::tempdir().expect("temp root");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let sink = Arc::new(BlockingSink {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
    let logger = logger(root.path(), sink, 1);
    let adapter = Arc::new(LoggerAdapter {
        logger: Arc::clone(&logger),
    });
    adapter
        .emit(event(adapter.service_name().clone()))
        .expect("initial admission");
    entered_rx.recv_timeout(WAIT).expect("writer entered sink");
    adapter
        .emit(event(adapter.service_name().clone()))
        .expect("queue slot");
    // Sink remains blocked while this real capacity-one queue rejects admission.
    let error = adapter
        .emit(event(adapter.service_name().clone()))
        .expect_err("queue full");
    assert_eq!(
        error.diagnostic().code,
        sc_observability::error_codes::LOGGER_QUEUE_FULL
    );
    drop(release_tx);
    logger.shutdown().expect("owner shutdown");
}

struct PanickingSink(mpsc::Sender<()>);

impl LogSink for PanickingSink {
    fn write(&self, _event: &LogEvent) -> Result<(), LogSinkError> {
        let _ = self.0.send(());
        panic!("test-only sink panic terminates the actual writer");
    }
    fn health(&self) -> SinkHealth {
        health()
    }
}

#[test]
fn disconnected_writer_preserves_native_error_context_and_classification() {
    bounded_fixture(
        "observability::adapter_tests::disconnected_writer_preserves_native_error_context_and_classification",
        disconnected_scenario,
    );
}

fn disconnected_scenario() {
    let root = tempfile::tempdir().expect("temp root");
    let (entered_tx, entered_rx) = mpsc::channel();
    let logger = logger(root.path(), Arc::new(PanickingSink(entered_tx)), 8);
    let adapter = LoggerAdapter {
        logger: Arc::clone(&logger),
    };
    adapter
        .emit(event(adapter.service_name().clone()))
        .expect("initial admission");
    entered_rx
        .recv_timeout(WAIT)
        .expect("writer entered panicking sink");
    // This real failed flush synchronizes with receiver disconnection after unwind.
    logger
        .flush_with_timeout(WAIT)
        .expect_err("writer receiver is disconnected");
    let direct = logger
        .try_log(event(logger.service_name().clone()))
        .expect_err("native disconnect");
    let forwarded = adapter
        .emit(event(adapter.service_name().clone()))
        .expect_err("forwarded disconnect");
    assert!(matches!(
        forwarded,
        sc_observability_types::v2::EventError::Routing { .. }
    ));
    assert_eq!(
        forwarded.diagnostic().code,
        sc_observability::error_codes::LOGGER_WRITER_DEGRADED
    );
    assert_eq!(
        forwarded.failure_classification(),
        direct.failure_classification()
    );
    let mut expected = serde_json::to_value(direct.diagnostic()).expect("native diagnostic");
    let mut actual = serde_json::to_value(forwarded.diagnostic()).expect("forwarded diagnostic");
    // Native admissions create separate timestamps; all native context is unchanged.
    expected.as_object_mut().unwrap().remove("timestamp");
    actual.as_object_mut().unwrap().remove("timestamp");
    assert_eq!(actual, expected);
    assert_eq!(
        forwarded.diagnostic().message,
        "writer thread disconnected while admitting non-blocking log work"
    );
    drop(adapter);
    let _ = logger.shutdown_with_timeout(WAIT);
}

#[test]
fn panic_mapping_is_fixed_and_value_free() {
    let error = super::admission_panicked();
    assert!(matches!(
        error,
        sc_observability_types::v2::EventError::Routing { .. }
    ));
    assert_eq!(
        error.diagnostic().code.as_str(),
        "DAEMON.OBSERVABILITY_ADMISSION_PANIC"
    );
    assert_eq!(error.diagnostic().message, "Logging admission panicked.");
}

struct PanickingRedactor;

impl sc_observability::Redactor for PanickingRedactor {
    fn redact(&self, _key: &str, _value: &mut serde_json::Value) {
        panic!("test-only secret panic payload");
    }
}

#[test]
fn real_try_log_unwind_becomes_fixed_error_without_payload() {
    let root = tempfile::tempdir().expect("temp root");
    let service = ServiceName::new("adapter-service").expect("service");
    let mut config = LoggerConfig::default_for(service, root.path().to_path_buf());
    config.enable_file_sink = true;
    config.enable_console_sink = false;
    config
        .redaction
        .custom_redactors
        .push(Box::new(PanickingRedactor));
    let logger = Arc::new(
        Logger::builder(config)
            .expect("builder")
            .build()
            .expect("logger"),
    );
    let adapter = LoggerAdapter {
        logger: Arc::clone(&logger),
    };
    let mut event = event(adapter.service_name().clone());
    event
        .fields
        .insert("secret-field".into(), serde_json::json!("secret value"));
    let error = adapter.emit(event).expect_err("unwind must be contained");
    assert_eq!(
        error.diagnostic().code.as_str(),
        "DAEMON.OBSERVABILITY_ADMISSION_PANIC"
    );
    assert_eq!(error.diagnostic().message, "Logging admission panicked.");
    let wire = serde_json::to_string(&error.diagnostic()).expect("diagnostic");
    for secret in [
        "secret-field",
        "secret value",
        "test-only secret panic payload",
    ] {
        assert!(
            !wire.contains(secret),
            "returned diagnostic must be value-free"
        );
    }
    drop(adapter);
    logger.shutdown().expect("owner shutdown");
}
