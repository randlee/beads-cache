// Private store-test checkpoints, independent of captured stdout/stderr.
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::fixture_child::FixtureChild;
use serde_json::{Value, json};

const MAX_STREAM_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Normal,
    Stalled,
    PostReturnGap,
}

impl Mode {
    pub(super) fn parse(mode: &std::ffi::OsStr) -> io::Result<Self> {
        match mode.to_str() {
            Some("normal") => Ok(Self::Normal),
            Some("stalled") => Ok(Self::Stalled),
            Some("post-return-gap") => Ok(Self::PostReturnGap),
            _ => Err(io::Error::other("invalid private store fixture mode")),
        }
    }
}

pub(super) fn record(started: Instant, stream: &str, phase: &str, data: &Value) -> io::Result<()> {
    let Some(root) = std::env::var_os("BD_CACHE_FIXTURE_ROOT") else {
        return Ok(()); // Ordinary in-process store tests have no child protocol.
    };
    let owner = std::env::var("BD_CACHE_FIXTURE_OWNER_UNIX_NS")
        .map_err(io::Error::other)?
        .parse::<u64>()
        .map_err(io::Error::other)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?;
    let wall_delta = i128::try_from(now.as_nanos()).map_err(io::Error::other)? - i128::from(owner);
    let value = json!({
        "pid": std::process::id(), "stream": stream, "phase": phase,
        "child_elapsed_ns": u64::try_from(started.elapsed().as_nanos()).map_err(io::Error::other)?,
        "wall_since_owner_ns": i64::try_from(wall_delta).map_err(io::Error::other)?, "data": data,
    });
    let mut bytes = serde_json::to_vec(&value)?;
    bytes.push(b'\n');
    let path = std::path::PathBuf::from(root).join(format!("store-{stream}-phases.jsonl"));
    // Each stream has one writing thread. No shared logging mutex or reader task.
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&bytes)
}

pub(super) fn worker_returned<T: std::fmt::Debug>(
    started: Instant,
    result: &Result<T, sc_cron::CronError>,
    mode: Mode,
) {
    let sqlite_code = result.as_ref().err().and_then(|error| {
        let mut source: &dyn std::error::Error = error;
        for _ in 0..8 {
            if let Some(rusqlite::Error::SqliteFailure(code, _)) = source.downcast_ref() {
                return Some(format!("{:?}", code.code));
            }
            source = source.source()?;
        }
        None
    });
    record(
        started,
        "worker",
        "worker-call-returned",
        &json!({
            "is_error": result.is_err(), "sqlite_code": sqlite_code, "debug": format!("{result:?}"),
        }),
    )
    .unwrap();
    if mode == Mode::PostReturnGap {
        // The real call and independent checkpoint write have both returned.
        // Park before either result stderr or the original required stall text.
        record(started, "worker", "planted-post-return-gap", &Value::Null).unwrap();
        loop {
            std::thread::park();
        }
    }
    eprintln!(
        "RQA9 store phase=worker-call-returned elapsed={:?} result={result:?}",
        started.elapsed()
    );
}

#[derive(Default)]
struct Receipts {
    seen: BTreeMap<(String, usize), Value>,
}

impl Receipts {
    fn collect(&mut self, child: &FixtureChild, final_read: bool) -> io::Result<()> {
        for stream in ["main", "worker"] {
            let path = child
                .scratch
                .path()
                .join(format!("store-{stream}-phases.jsonl"));
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let mut text = String::new();
            file.take(MAX_STREAM_BYTES + 1).read_to_string(&mut text)?;
            if u64::try_from(text.len()).unwrap() > MAX_STREAM_BYTES {
                return Err(io::Error::other("store phase stream exceeds finite limit"));
            }
            if final_read && !text.is_empty() && !text.ends_with('\n') {
                return Err(io::Error::other("incomplete final store phase receipt"));
            }
            // A live append may be incomplete; only complete lines are received.
            // Final collection after root wait rejects any remaining partial line.
            for (index, line) in text.split_terminator('\n').enumerate() {
                if !text.ends_with('\n') && index + 1 == text.lines().count() {
                    break;
                }
                let value: Value = serde_json::from_str(line)?;
                if value["pid"].as_u64() != Some(u64::from(child.process.id()))
                    || value["stream"].as_str() != Some(stream)
                    || value["phase"].as_str().is_none()
                    || value["child_elapsed_ns"].as_u64().is_none()
                {
                    return Err(io::Error::other("invalid store phase identity or payload"));
                }
                let key = (stream.to_owned(), index);
                if let Some(previous) = self.seen.get(&key) {
                    if previous["execution"] != value {
                        return Err(io::Error::other("store phase receipt was rewritten"));
                    }
                } else {
                    self.seen.insert(key, json!({
                        "execution": value,
                        "owner_received_ns": u64::try_from(child.started.elapsed().as_nanos()).unwrap(),
                    }));
                }
            }
        }
        Ok(())
    }
}

pub(super) struct Observation {
    status: Option<std::process::ExitStatus>,
    cleaned: bool,
    elapsed: Duration,
    output: String,
    receipts: Receipts,
}

impl Observation {
    fn phase(&self, name: &str) -> Option<&Value> {
        self.receipts
            .seen
            .values()
            .map(|value| &value["execution"])
            .find(|value| value["phase"].as_str() == Some(name))
    }

    pub(super) fn classification(&self) -> &str {
        match (
            self.phase("worker-call-returned"),
            self.output
                .contains("stalled store cleanup after real worker call"),
        ) {
            (Some(_), false) => "worker_returned_required_marker_missing",
            (Some(_), true) => "worker_return_and_stall_marker_observed",
            (None, _) => "worker_return_unobserved_phase_unresolved",
        }
    }

    pub(super) fn qualify(&self) {
        assert!(
            self.status.is_none(),
            "stalled fixture exited: {}",
            self.output
        );
        assert!(self.cleaned, "test child was not killed and reaped");
        assert!(
            self.elapsed < Duration::from_secs(7),
            "supervisor cleanup exceeded its finite watchdog"
        );
        assert!(
            self.output
                .contains("stalled store cleanup after real worker call"),
            "required post-call/stall receipt missing; classification={}: {}",
            self.classification(),
            self.output
        );
        assert!(
            self.output
                .contains("store cleanup observation expired after 500ms"),
            "failure join path was not reached: {}",
            self.output
        );
        self.assert_real_return_and_reap();
        assert!(
            self.phase("stall-marker-written").is_some(),
            "independent marker-write receipt absent"
        );
    }

    pub(super) fn assert_planted_return(&self) {
        assert!(self.phase("planted-post-return-gap").is_some());
        assert!(self.phase("stall-marker-written").is_none());
        assert!(
            !self
                .output
                .contains("stalled store cleanup after real worker call")
        );
        assert_eq!(
            self.phase("worker-call-returned").unwrap()["data"]["sqlite_code"],
            "DatabaseBusy"
        );
    }

    pub(super) fn assert_real_return_and_reap(&self) {
        assert!(self.status.is_none() && self.cleaned && self.elapsed < Duration::from_secs(7));
        for phase in [
            "fixture-body-entered",
            "schedule-setup-complete",
            "worker-call-start",
            "cancellation-installed",
            "cancellation-deadline-installed",
            "worker-call-returned",
            "worker-join-start",
        ] {
            assert!(
                self.phase(phase).is_some(),
                "required structured phase absent: {phase}"
            );
        }
        assert_eq!(self.phase("wait-result").unwrap()["data"]["ok"], true);
        assert_eq!(
            self.phase("worker-call-returned").unwrap()["data"]["is_error"],
            true
        );
        assert_eq!(
            self.phase("completion-result").unwrap()["data"]["timeout"],
            true
        );
        assert!(
            self.output
                .contains("store cleanup observation expired after 500ms")
        );
    }
}

pub(super) fn observe(test: &str, key: &str, mode: &str) -> io::Result<Observation> {
    let started = Instant::now();
    let mut child = FixtureChild::spawn(test, key, mode)?;
    let mut receipts = Receipts::default();
    let mut owner_clock = None;
    let status =
        child.finish_observing(Duration::from_secs(5), |child, final_read, spawn, unix| {
            owner_clock = Some((spawn, unix));
            receipts.collect(child, final_read)
        })?;
    let (spawn_returned, owner_unix_ns) = owner_clock.unwrap();
    let observation = Observation {
        status,
        cleaned: child.cleaned.load(Ordering::SeqCst),
        elapsed: started.elapsed(),
        output: child.output()?,
        receipts,
    };
    // Parent monotonic receive times share the original pre-spawn owner origin.
    // They bound observation, not execution. Wall deltas are only correlation:
    // clock adjustments/cross-process precision are never used for a decision.
    eprintln!(
        "RQA9_PROTOCOL {}",
        json!({
            "test": test, "mode": mode, "classification": observation.classification(),
            "owner_origin": "Instant before Command.spawn", "owner_unix_ns": owner_unix_ns,
            "outer_to_owner_ns": u64::try_from(child.started.duration_since(started).as_nanos()).unwrap(),
        "spawn_returned_ns": u64::try_from(spawn_returned.as_nanos()).unwrap(),
        "owner_reported_ns": u64::try_from(child.started.elapsed().as_nanos()).unwrap(),
        "absolute_lifetime_ns": 5_000_000_000_u64,
            "root_joined_and_cleaned": observation.cleaned,
            "receipts": observation.receipts.seen.values().collect::<Vec<_>>(),
            "clock_limits": "receive is not execution; wall correlation may move; all deadlines use original owner Instant",
        })
    );
    eprintln!("RQA9 owned child {test} output:\n{}", observation.output);
    eprintln!(
        "FTQ-009 {test}: owned timeout and root reap in {:?}",
        observation.elapsed
    );
    Ok(observation)
}

pub(super) fn wait_result(
    started: Instant,
    entered: Result<(), std::sync::mpsc::RecvTimeoutError>,
) {
    record(
        started,
        "main",
        "wait-result",
        &json!({"ok": entered.is_ok()}),
    )
    .unwrap();
    eprintln!(
        "RQA9 store phase=wait-receipt elapsed={:?} result={entered:?}",
        started.elapsed()
    );
}

pub(super) fn worker_started(started: Instant) {
    record(started, "worker", "worker-call-start", &Value::Null).unwrap();
}

pub(super) fn cancel(started: Instant, control: &crate::ScheduleShutdownControl) {
    let deadline = Instant::now() + Duration::from_millis(30);
    control.finish_by(deadline);
    record(started, "main", "cancellation-deadline-installed", &json!({
        "child_deadline_ns": u64::try_from(deadline.saturating_duration_since(started).as_nanos()).unwrap(),
    })).unwrap();
}
