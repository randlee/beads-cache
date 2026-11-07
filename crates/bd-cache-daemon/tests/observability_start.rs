//! Public initialization errors retain native diagnostic and source identity.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "integration test assertions"
)]

use bd_cache_daemon::observability::{Observability, ObservabilityStart};
use sc_observability::{LogSettings, LoggerConfig, ServiceName, v2::LoggerBuilder};
use sc_observability_types::{Remediation, ValueValidationError, v2::InitError};
use std::error::Error;
use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const PROCESS_WAIT: Duration = Duration::from_secs(10);
const OUTPUT_LIMIT: u64 = 3205;

// Startup fails before stores or helpers can start. Own this direct child even
// if output validation or an assertion unwinds; never wait on pipe EOF.
struct StartupChild(Child);

impl Drop for StartupChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn bounded_startup(command: &mut Command, wait: Duration) -> Result<Output, &'static str> {
    let until = Instant::now() + wait;
    let stdout = tempfile::NamedTempFile::new().map_err(|_error| "stdout file")?;
    let stderr = tempfile::NamedTempFile::new().map_err(|_error| "stderr file")?;
    let mut child = StartupChild(
        command
            .stdin(Stdio::null())
            .stdout(stdout.reopen().map_err(|_error| "stdout handle")?)
            .stderr(stderr.reopen().map_err(|_error| "stderr handle")?)
            .spawn()
            .map_err(|_error| "startup child")?,
    );
    let status = loop {
        for file in [&stdout, &stderr] {
            if file
                .as_file()
                .metadata()
                .map_err(|_error| "output size")?
                .len()
                > OUTPUT_LIMIT
            {
                return Err("startup output exceeded its bound; child killed and reaped");
            }
        }
        if let Some(status) = child.0.try_wait().map_err(|_error| "startup status")? {
            break status;
        }
        if Instant::now() >= until {
            return Err("startup timed out; child killed and reaped");
        }
        std::thread::sleep(Duration::from_millis(10)); // allow: poll this owned child
    };
    let capture = |file: &tempfile::NamedTempFile| {
        let mut bytes = Vec::new();
        file.reopen()
            .map_err(|_error| "output handle")?
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_error| "output read")?;
        if bytes.len() as u64 > OUTPUT_LIMIT {
            return Err("startup output exceeded its bound; child killed and reaped");
        }
        Ok(bytes)
    };
    Ok(Output {
        status,
        stdout: capture(&stdout)?,
        stderr: capture(&stderr)?,
    })
}

#[test]
fn real_service_name_failure_survives_an_ordinary_error_boundary() {
    let root = tempfile::tempdir().expect("root");
    let error = Observability::init(
        "secret-service\ninvalid",
        &LogSettings::default(),
        root.path().to_owned(),
    )
    .expect_err("actual upstream name validation");
    let boxed: Box<dyn Error> = Box::new(error);
    let retained = boxed
        .source()
        .expect("native source")
        .downcast_ref::<ValueValidationError>()
        .expect("concrete upstream validation error");
    let wrapper = boxed
        .downcast_ref::<ObservabilityStart>()
        .expect("public wrapper");
    let direct = ServiceName::new("secret-service\ninvalid").expect_err("invalid identifier");
    assert_eq!(retained, &direct);
    assert_eq!(wrapper.code(), retained.code());
    assert!(wrapper.recovery().contains("service name"));
    assert!(!wrapper.to_string().contains("secret-service"));
}

#[test]
fn real_build_failure_retains_the_original_diagnostic_and_remediation() {
    let root = tempfile::tempdir().expect("root");
    let settings = LogSettings {
        enable_file_sink: Some(false),
        enable_console_sink: Some(false),
        ..LogSettings::default()
    };
    let error = Observability::init("test-service", &settings, root.path().to_owned())
        .expect_err("actual upstream build requires a sink");
    let retained = error
        .source()
        .expect("native source")
        .downcast_ref::<InitError>()
        .expect("concrete upstream init error");
    assert!(matches!(retained, InitError::Configuration { .. }));
    assert_eq!(
        error.code(),
        &sc_observability::error_codes::LOGGER_INIT_FAILED
    );
    assert_eq!(
        retained.diagnostic().message,
        "logger must have at least one registered sink"
    );
    assert!(std::ptr::eq(
        error.code(),
        &raw const retained.diagnostic().code
    ));
    assert!(std::ptr::eq(
        error.remediation(),
        &raw const retained.diagnostic().remediation
    ));
    let Remediation::Recoverable { steps } = error.remediation() else {
        panic!("upstream init has actionable remediation");
    };
    assert_eq!(steps.steps().len(), 3);
    assert_eq!(Some(error.recovery()), steps.first_step());
    assert_eq!(error.to_string(), retained.to_string());
}

#[test]
fn real_builder_validation_failure_keeps_native_type_and_code() {
    let root = tempfile::tempdir().expect("root");
    let mut config = LoggerConfig::default_for(
        ServiceName::new("test-service").expect("name"),
        root.path().to_owned(),
    );
    config.queue_capacity = 0;
    let Err(native) = LoggerBuilder::new(config) else {
        panic!("upstream must reject zero queue capacity");
    };
    let error = ObservabilityStart::from(native);
    let retained = error
        .source()
        .expect("native source")
        .downcast_ref::<InitError>()
        .expect("typed builder validation");
    assert_eq!(error.code(), &retained.diagnostic().code);
    assert_eq!(error.remediation(), &retained.diagnostic().remediation);
    assert_eq!(
        retained.diagnostic().message,
        "logger queue capacity must be greater than zero"
    );
    assert!(error.recovery().contains("positive value"));
}

#[test]
fn process_formats_real_init_failure_once_without_values_or_stdout() {
    let root = tempfile::tempdir().expect("root");
    let instance = root.path().join("secret-instance-must-not-open");
    let config = root.path().join("daemon.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&serde_json::json!({
            "instance_root": instance,
            "logging": {
                "logRoot": root.path().join("secret-log-root\nvalue"),
                "enableFileSink": false,
                "enableConsoleSink": false
            }
        }))
        .expect("config bytes"),
    )
    .expect("write config");
    let output = bounded_startup(
        Command::new(env!("CARGO_BIN_EXE_bd-cache-daemon"))
            .arg("--repo")
            .arg(root.path().join("missing-repository"))
            .arg("--config")
            .arg(config),
        PROCESS_WAIT,
    )
    .expect("run bounded real daemon");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr UTF-8");
    assert_eq!(
        stderr,
        "DAEMON.OBSERVABILITY logger must have at least one registered sink — enable a built-in sink or register a sink before building the logger\n"
    );
    assert!(!stderr.contains("secret-"));
    assert!(stderr.len() <= 3205);
    assert!(
        !instance.exists(),
        "logging fails before application stores"
    );
}

#[test]
fn startup_supervisor_bounds_a_stalled_owned_child() {
    const NAME: &str = "startup_supervisor_bounds_a_stalled_owned_child";
    if std::env::var("STARTUP_SUPERVISOR_CASE").ok().as_deref() == Some(NAME) {
        loop {
            std::thread::park();
        }
    }
    let error = bounded_startup(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", NAME, "--nocapture"])
            .env("STARTUP_SUPERVISOR_CASE", NAME),
        Duration::from_millis(100),
    )
    .expect_err("stalled direct child is killed and reaped");
    assert_eq!(error, "startup timed out; child killed and reaped");
}

#[test]
fn startup_supervisor_rejects_excess_output() {
    const NAME: &str = "startup_supervisor_rejects_excess_output";
    if std::env::var("STARTUP_SUPERVISOR_CASE").ok().as_deref() == Some(NAME) {
        eprintln!(
            "{}",
            "x".repeat(usize::try_from(OUTPUT_LIMIT + 1).expect("small bound"))
        );
        return;
    }
    let error = bounded_startup(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", NAME, "--nocapture"])
            .env("STARTUP_SUPERVISOR_CASE", NAME),
        PROCESS_WAIT,
    )
    .expect_err("output is capped independently of child exit");
    assert_eq!(
        error,
        "startup output exceeded its bound; child killed and reaped"
    );
}
