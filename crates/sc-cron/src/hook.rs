//! Hook targets: a Rust [`Job`], or a Python module run as a child process.
//!
//! A schedule's worker runs one [`HookTarget`] (REQ-CRON-0013, ADR-CRON-0001,
//! ADR-CRON-0008). The Rust target is a [`Job`]. The Python target is
//! `<interpreter> -I -m <module>`, started from a fresh temporary working
//! directory once for each hook event:
//!
//! - `cron.stop` before every run, asking whether to go on, and
//! - `cron.tick` for the run itself, if the answer was to continue.
//!
//! Each event gets one JSON object on stdin:
//!
//! ```json
//! {"protocol": "sc-cron.hook.v1", "event": "cron.tick", "schedule": "name",
//!  "payload": {}, "deadline": "2026-10-25T00:00:30.000000Z"}
//! ```
//!
//! `deadline` is the run's deadline as a UTC instant. A `cron.stop` hook
//! answers on stdout with one JSON object,
//! `{"action": "continue" | "disable" | "delete", "reason": "..."}`; `reason`
//! is required for `disable` and `delete`. Anything it prints besides that
//! object belongs on stderr. A `cron.tick` hook succeeds by exiting 0.
//!
//! # Bounds
//!
//! Every hook is killed, with everything it started, at the schedule's
//! timeout capped by the time left before the run's deadline
//! (REQ-CRON-0013); see [`kill_budget`]. Its stdout and stderr are each kept up
//! to [`MAX_HOOK_OUTPUT_BYTES`] and the rest is discarded.

use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::clock::wall_after;
use crate::process::{Capped, ChildEnv, ChildSpec, TempCwd, supervise};
use crate::{Clock, CronError, Job, RunOutcome, RunTimeout, ScheduleName, StopDecision, Timestamp};

/// The `protocol` member of every hook request.
pub const HOOK_PROTOCOL: &str = "sc-cron.hook.v1";

/// Most bytes of a hook's stdout, and of its stderr, that are kept.
///
/// Equal to [`MAX_PAYLOAD_BYTES`](crate::MAX_PAYLOAD_BYTES): a reply is a
/// small JSON object, and a hook that prints more than the largest payload
/// is misbehaving. Raising it only lets a runaway hook hold more memory.
pub const MAX_HOOK_OUTPUT_BYTES: usize = crate::MAX_PAYLOAD_BYTES;

/// Longest Python module path, in bytes.
const MAX_MODULE_LEN: usize = 256;

/// Longest slice of a hook's stderr put into a failure message, in bytes.
///
/// A failure message is stored as the schedule's last outcome; the whole of
/// stderr is available from [`HookOutput`] to a caller that wants it.
const STDERR_IN_MESSAGE: usize = 512;

/// Maps a schedule row to the target its run executes (REQ-CRON-0016, ADR-CRON-0009).
///
/// An object-safe, synchronous, open extension point: any caller closure
/// maps a row to a [`HookTarget`]. The worker of
/// [`Scheduler::spawn_worker_with`](crate::Scheduler::spawn_worker_with)
/// calls it once per launch, before the stop check, so a resolver may
/// re-check anything per run. An `Err` is recorded as
/// [`RunOutcome::Failed`], with the schedule left active and enabled.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use sc_cron::{CronError, HookTarget, PythonHook, TargetResolver};
///
/// let resolver: TargetResolver = Arc::new(|row| match row.spec.payload.get("module") {
///     Some(serde_json::Value::String(m)) => Ok(HookTarget::Python(PythonHook::new("python3", m)?)),
///     _ => Err(CronError::InvalidSpec(format!("row {} names no module", row.spec.name))),
/// });
/// # let _ = resolver;
/// ```
pub type TargetResolver =
    Arc<dyn Fn(&crate::Schedule) -> Result<HookTarget, CronError> + Send + Sync>;

/// What a schedule's worker runs on each tick.
#[derive(Clone)]
pub enum HookTarget {
    /// A Rust [`Job`], called in this process.
    Rust(Arc<dyn Job>),
    /// A Python module, run as a child process.
    Python(PythonHook),
}

impl fmt::Debug for HookTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust(_) => f.write_str("HookTarget::Rust(..)"),
            Self::Python(hook) => f.debug_tuple("HookTarget::Python").field(hook).finish(),
        }
    }
}

/// The event a hook is asked about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookEvent {
    /// The run itself: `cron.tick`.
    Tick,
    /// The stop check before a run: `cron.stop`.
    Stop,
}

impl HookEvent {
    /// Returns the event name sent to the hook.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tick => "cron.tick",
            Self::Stop => "cron.stop",
        }
    }
}

/// A Python module run as `<interpreter> -I -m <module>`.
///
/// # Examples
///
/// ```
/// use sc_cron::PythonHook;
///
/// let hook = PythonHook::new("python3", "my_pkg.cron_hook")?;
/// assert_eq!(hook.module(), "my_pkg.cron_hook");
/// assert!(PythonHook::new("python3", "-c").is_err()); // not a module path
/// assert!(PythonHook::new("./venv/bin/python", "m").is_err()); // relative path
/// # Ok::<(), sc_cron::CronError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonHook {
    interpreter: PathBuf,
    module: String,
    env: ChildEnv,
}

impl PythonHook {
    /// Validates the interpreter and the module, once, before anything runs.
    ///
    /// `interpreter` is an absolute path, or a bare command name looked up on
    /// `PATH`. A relative path with directories is refused: the hook runs in
    /// a temporary directory, so it would not mean what the caller wrote.
    /// `module` is a dotted path of ASCII identifiers (`pkg.sub.mod`), at most
    /// 256 bytes, so it cannot be read as a Python option.
    ///
    /// # Errors
    ///
    /// [`CronError::InvalidSpec`] if either value breaks a rule above.
    pub fn new(interpreter: impl Into<PathBuf>, module: &str) -> Result<Self, CronError> {
        let interpreter = interpreter.into();
        let bare = interpreter.components().count() == 1
            && matches!(interpreter.components().next(), Some(Component::Normal(_)));
        if !interpreter.is_absolute() && !bare {
            return Err(CronError::InvalidSpec(format!(
                "interpreter {} must be an absolute path or a bare command name",
                interpreter.display()
            )));
        }
        let identifier = |part: &str| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        };
        if module.len() > MAX_MODULE_LEN || !module.split('.').all(identifier) {
            return Err(CronError::InvalidSpec(format!(
                "module {module:?} is not a dotted path of identifiers of at most \
                 {MAX_MODULE_LEN} bytes"
            )));
        }
        Ok(Self {
            interpreter,
            module: module.to_owned(),
            env: ChildEnv::Inherit,
        })
    }

    /// The same hook, started with `env` (REQ-CRON-0018).
    ///
    /// The default is [`ChildEnv::Inherit`]. `-I` does not clear the
    /// environment, so a caller that must not pass its own variables on sets
    /// [`ChildEnv::Only`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_cron::PythonHook;
    /// use sc_cron::process::ChildEnv;
    ///
    /// let hook = PythonHook::new("python3", "pkg.hook")?
    ///     .with_env(ChildEnv::Only { keep: vec!["PATH".into()], set: vec![] });
    /// assert!(matches!(hook.env(), ChildEnv::Only { .. }));
    /// # Ok::<(), sc_cron::CronError>(())
    /// ```
    #[must_use]
    pub fn with_env(mut self, env: ChildEnv) -> Self {
        self.env = env;
        self
    }

    /// Returns the environment the hook starts with.
    #[must_use]
    pub fn env(&self) -> &ChildEnv {
        &self.env
    }

    /// Returns the interpreter.
    #[must_use]
    pub fn interpreter(&self) -> &Path {
        &self.interpreter
    }

    /// Returns the module path.
    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Runs the hook once for `call` and returns what it did.
    ///
    /// The hook is killed at [`kill_budget`] of `call`, measured on `clock`'s
    /// monotonic timeline, and the budget is waited out on the tokio timer. A
    /// run whose deadline has already passed starts nothing and reports
    /// [`HookStatus::TimedOut`].
    ///
    /// # Errors
    ///
    /// [`CronError::HookFailed`] if the temporary directory cannot be made or
    /// the interpreter cannot be started. A hook that starts and fails is
    /// not an error here: it is the [`HookOutput`].
    pub async fn invoke<C: Clock + ?Sized>(
        &self,
        clock: &C,
        call: &HookCall<'_>,
    ) -> Result<HookOutput, CronError> {
        let now = clock.monotonic_now();
        let budget = kill_budget(call.timeout.get(), call.deadline, now);
        if budget.is_zero() {
            return Ok(HookOutput::timed_out());
        }
        let left = call.deadline.saturating_duration_since(now);
        let deadline = Timestamp::from_datetime(wall_after(clock.now(), left));
        self.run(call, deadline, tokio::time::sleep(budget)).await
    }

    /// Runs the hook, killing it when `expiry` completes.
    pub(crate) async fn run(
        &self,
        call: &HookCall<'_>,
        deadline: Timestamp,
        expiry: impl Future<Output = ()>,
    ) -> Result<HookOutput, CronError> {
        let input = request(call, deadline).to_string();
        let cwd = TempCwd::new()
            .map_err(|error| CronError::HookFailed(format!("no temporary directory: {error}")))?;
        let spec = ChildSpec {
            program: &self.interpreter,
            args: vec!["-I".into(), "-m".into(), self.module.clone().into()],
            cwd: cwd.path(),
            input: input.as_bytes(),
            env: self.env.clone(),
        };
        let captured = supervise(&spec, MAX_HOOK_OUTPUT_BYTES, expiry)
            .await
            .map_err(|error| {
                CronError::HookFailed(format!(
                    "{} -m {}: {error}",
                    self.interpreter.display(),
                    self.module
                ))
            })?;
        let text = |capped: Capped| {
            (
                String::from_utf8_lossy(&capped.bytes).into_owned(),
                capped.truncated,
            )
        };
        let ((stdout, stdout_truncated), (stderr, stderr_truncated)) =
            (text(captured.stdout), text(captured.stderr));
        Ok(HookOutput {
            status: captured
                .status
                .map_or(HookStatus::TimedOut, HookStatus::Exited),
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
        })
    }
}

/// One hook invocation: the event, the schedule it is for and its bounds.
#[derive(Clone, Copy, Debug)]
pub struct HookCall<'a> {
    /// The event being asked about.
    pub event: HookEvent,
    /// The schedule the hook runs for.
    pub schedule: &'a ScheduleName,
    /// The schedule's payload, sent as is.
    pub payload: &'a Value,
    /// The schedule's timeout.
    pub timeout: RunTimeout,
    /// The run's deadline, on the clock's monotonic timeline.
    pub deadline: Instant,
}

/// What a hook did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookOutput {
    /// How it ended.
    pub status: HookStatus,
    /// Its stdout, up to [`MAX_HOOK_OUTPUT_BYTES`] (invalid UTF-8 replaced).
    pub stdout: String,
    /// Its stderr, up to [`MAX_HOOK_OUTPUT_BYTES`] (invalid UTF-8 replaced).
    pub stderr: String,
    /// Whether stdout had more than was kept.
    pub stdout_truncated: bool,
    /// Whether stderr had more than was kept.
    pub stderr_truncated: bool,
}

impl HookOutput {
    fn timed_out() -> Self {
        Self {
            status: HookStatus::TimedOut,
            stdout: String::new(),
            stderr: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    /// What this output of a `cron.stop` hook decides (REQ-CRON-0005, REQ-CRON-0018).
    ///
    /// `Ok` with the decision when the hook exited 0 and printed one of the
    /// three actions; otherwise `Err` with the outcome to record for a run
    /// that is skipped: [`RunOutcome::TimedOut`] for a hook killed at its
    /// timeout, [`RunOutcome::Failed`] for a non-zero exit or a reply that is
    /// not one of the actions. A caller that runs a [`PythonHook`] inside its
    /// own [`Job`] applies the protocol's rules with this, without copying them.
    ///
    /// # Errors
    ///
    /// The outcome of a stop check that did not decide, as above.
    pub fn stop_decision(&self) -> Result<StopDecision, RunOutcome> {
        if self.status == HookStatus::TimedOut {
            return Err(RunOutcome::TimedOut);
        }
        if let Some(failure) = self.failure() {
            return Err(RunOutcome::Failed(format!("stop check: {failure}")));
        }
        parse_reply(self).map_err(|why| RunOutcome::Failed(format!("stop check: {why}")))
    }

    /// The outcome of this output of a `cron.tick` hook (REQ-CRON-0013, REQ-CRON-0018).
    ///
    /// Exit 0 is [`RunOutcome::Ok`], a kill at the timeout
    /// [`RunOutcome::TimedOut`], any other exit [`RunOutcome::Failed`] with
    /// the start of its stderr.
    #[must_use]
    pub fn run_outcome(&self) -> RunOutcome {
        if self.status == HookStatus::TimedOut {
            return RunOutcome::TimedOut;
        }
        self.failure().map_or(RunOutcome::Ok, RunOutcome::Failed)
    }

    /// Describes how the hook failed, or `None` if it exited 0.
    fn failure(&self) -> Option<String> {
        match &self.status {
            HookStatus::Exited(status) if status.success() => None,
            HookStatus::Exited(status) => {
                let stderr = self.stderr.trim();
                let cut = stderr.floor_char_boundary(STDERR_IN_MESSAGE);
                let shown = stderr.get(..cut).unwrap_or_default();
                Some(if shown.is_empty() {
                    format!("hook exited with {status}")
                } else {
                    format!("hook exited with {status}: {shown}")
                })
            }
            HookStatus::TimedOut => Some("hook was killed at its timeout".to_owned()),
        }
    }
}

/// How a hook ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookStatus {
    /// It exited, or was ended by a signal, on its own.
    Exited(ExitStatus),
    /// It was killed, with everything it started, at its timeout.
    TimedOut,
}

/// The kill time for a hook: `timeout`, capped by the time left before `deadline`.
///
/// Every hook a schedule starts gets this (REQ-CRON-0013), so the hook for the
/// stop check and the hook for the run both end by the run's deadline.
/// `now` is on the same timeline as `deadline`; a deadline already passed
/// gives zero.
#[must_use]
pub fn kill_budget(timeout: Duration, deadline: Instant, now: Instant) -> Duration {
    timeout.min(deadline.saturating_duration_since(now))
}

/// The JSON object a hook reads on stdin.
fn request(call: &HookCall<'_>, deadline: Timestamp) -> Value {
    json!({
        "protocol": HOOK_PROTOCOL,
        "event": call.event.as_str(),
        "schedule": call.schedule.as_str(),
        "payload": call.payload,
        "deadline": deadline.to_string(),
    })
}

/// What the worker does with one run of a Python target.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Run the tick hook.
    Run,
    /// Skip this run with this outcome; the schedule stays active and enabled.
    Skip(RunOutcome),
    /// The stop check said to disable the schedule.
    Disable(String),
    /// The stop check said to end the schedule.
    Delete(String),
}

/// Runs the `cron.stop` hook and reads its reply (REQ-CRON-0005).
///
/// A hook that cannot be started, exits non-zero, prints a reply that is not
/// one of the three actions, or is killed at its timeout does not decide:
/// the run is skipped and the schedule left as it is.
pub(crate) async fn stop_step<C: Clock + ?Sized>(
    hook: &PythonHook,
    clock: &C,
    call: &HookCall<'_>,
) -> Step {
    stop_outcome(hook.invoke(clock, call).await)
}

/// What the `cron.stop` hook's result means.
fn stop_outcome(result: Result<HookOutput, CronError>) -> Step {
    let output = match result {
        Ok(output) => output,
        Err(error) => return Step::Skip(RunOutcome::Failed(format!("stop check: {error}"))),
    };
    match output.stop_decision() {
        Ok(StopDecision::Continue) => Step::Run,
        Ok(StopDecision::Disable(reason)) => Step::Disable(reason),
        Ok(StopDecision::Delete(reason)) => Step::Delete(reason),
        Err(outcome) => Step::Skip(outcome),
    }
}

/// Runs the `cron.tick` hook: exit 0 is [`RunOutcome::Ok`].
pub(crate) async fn tick_outcome<C: Clock + ?Sized>(
    hook: &PythonHook,
    clock: &C,
    call: &HookCall<'_>,
) -> RunOutcome {
    tick_result(hook.invoke(clock, call).await)
}

/// What the `cron.tick` hook's result means.
fn tick_result(result: Result<HookOutput, CronError>) -> RunOutcome {
    match result {
        Err(error) => RunOutcome::Failed(error.to_string()),
        Ok(output) => output.run_outcome(),
    }
}

/// Reads a stop hook's stdout as `{"action": ..., "reason": ...}`.
fn parse_reply(output: &HookOutput) -> Result<StopDecision, String> {
    if output.stdout_truncated {
        return Err(format!(
            "reply is longer than {MAX_HOOK_OUTPUT_BYTES} bytes"
        ));
    }
    let reply: Value = serde_json::from_str(output.stdout.trim())
        .map_err(|error| format!("reply is not JSON: {error}"))?;
    let action = reply
        .get("action")
        .and_then(Value::as_str)
        .ok_or("reply has no string \"action\"")?;
    let reason = match reply.get("reason") {
        None | Some(Value::Null) => None,
        Some(Value::String(reason)) if !reason.trim().is_empty() => Some(reason.clone()),
        Some(_) => return Err("\"reason\" is not a non-empty string".to_owned()),
    };
    let needs_reason = || {
        reason
            .clone()
            .ok_or(format!("action {action:?} needs a \"reason\""))
    };
    match action {
        "continue" => Ok(StopDecision::Continue),
        "disable" => needs_reason().map(StopDecision::Disable),
        "delete" => needs_reason().map(StopDecision::Delete),
        other => Err(format!(
            "unknown action {other:?}; expected continue, disable or delete"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(stdout: &str) -> HookOutput {
        HookOutput {
            status: HookStatus::TimedOut,
            stdout: stdout.to_owned(),
            stderr: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    /// An output that exited with `code`, saying `stdout`.
    #[cfg(unix)]
    fn exited(code: i32, stdout: &str) -> HookOutput {
        use std::os::unix::process::ExitStatusExt;
        HookOutput {
            status: HookStatus::Exited(ExitStatus::from_raw(code << 8)),
            ..reply(stdout)
        }
    }

    fn killed() -> HookOutput {
        reply("")
    }

    #[test]
    fn a_stop_hook_killed_at_its_timeout_is_recorded_timed_out_and_skips_the_run() {
        assert_eq!(stop_outcome(Ok(killed())), Step::Skip(RunOutcome::TimedOut));
    }

    #[test]
    fn a_tick_hook_killed_at_its_timeout_is_recorded_timed_out() {
        assert_eq!(tick_result(Ok(killed())), RunOutcome::TimedOut);
    }

    #[test]
    fn a_hook_that_cannot_start_skips_the_stop_check_and_fails_the_tick() {
        let error = || Err(CronError::HookFailed("no interpreter".into()));
        assert!(matches!(
            stop_outcome(error()),
            Step::Skip(RunOutcome::Failed(why)) if why.starts_with("stop check: ")
        ));
        assert!(matches!(tick_result(error()), RunOutcome::Failed(_)));
    }

    #[cfg(unix)]
    #[test]
    fn a_stop_hook_that_exits_non_zero_decides_nothing_whatever_it_printed() {
        let step = stop_outcome(Ok(exited(2, r#"{"action": "delete", "reason": "x"}"#)));
        assert!(
            matches!(&step, Step::Skip(RunOutcome::Failed(why)) if why.contains("exit status: 2")),
            "{step:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_zero_exit_stop_hook_is_read_for_its_action() {
        assert_eq!(
            stop_outcome(Ok(exited(0, r#"{"action": "continue"}"#))),
            Step::Run
        );
        assert_eq!(
            stop_outcome(Ok(exited(0, r#"{"action": "disable", "reason": "r"}"#))),
            Step::Disable("r".into())
        );
        assert_eq!(
            stop_outcome(Ok(exited(0, r#"{"action": "delete", "reason": "r"}"#))),
            Step::Delete("r".into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_tick_hook_is_ok_on_exit_zero_and_failed_with_its_stderr_otherwise() {
        assert_eq!(tick_result(Ok(exited(0, ""))), RunOutcome::Ok);
        let mut failed = exited(3, "");
        failed.stderr = "boom\n".into();
        assert_eq!(
            tick_result(Ok(failed)),
            RunOutcome::Failed("hook exited with exit status: 3: boom".into())
        );
    }

    #[test]
    fn kill_budget_is_the_timeout_capped_by_the_time_left() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(10);
        // Plenty of time left: the row's timeout rules.
        assert_eq!(
            kill_budget(Duration::from_secs(4), deadline, now),
            Duration::from_secs(4)
        );
        // Less time left than the timeout: the deadline rules.
        assert_eq!(
            kill_budget(Duration::from_secs(30), deadline, now),
            Duration::from_secs(10)
        );
        assert_eq!(
            kill_budget(
                Duration::from_secs(30),
                deadline,
                now + Duration::from_secs(7)
            ),
            Duration::from_secs(3)
        );
        // Deadline passed: nothing left.
        assert_eq!(
            kill_budget(
                Duration::from_secs(30),
                deadline,
                deadline + Duration::from_secs(1)
            ),
            Duration::ZERO
        );
    }

    #[test]
    fn stop_replies_name_each_action() {
        assert_eq!(
            parse_reply(&reply(r#"{"action": "continue"}"#)),
            Ok(StopDecision::Continue)
        );
        assert_eq!(
            parse_reply(&reply(r#"{"action": "continue", "reason": "ok"}"#)),
            Ok(StopDecision::Continue)
        );
        assert_eq!(
            parse_reply(&reply("  {\"action\":\"disable\",\"reason\":\"quiet\"}\n")),
            Ok(StopDecision::Disable("quiet".into()))
        );
        assert_eq!(
            parse_reply(&reply(r#"{"action": "delete", "reason": "done"}"#)),
            Ok(StopDecision::Delete("done".into()))
        );
    }

    #[test]
    fn malformed_stop_replies_are_refused_not_guessed() {
        for bad in [
            "",
            "continue",
            "[]",
            r#"{"action": "stop", "reason": "x"}"#,
            r#"{"action": 1}"#,
            r#"{"action": "disable"}"#,
            r#"{"action": "delete", "reason": ""}"#,
            r#"{"action": "delete", "reason": 3}"#,
            r#"{"reason": "x"}"#,
        ] {
            assert!(parse_reply(&reply(bad)).is_err(), "{bad:?}");
        }
        let mut cut = reply(r#"{"action": "continue"}"#);
        cut.stdout_truncated = true;
        assert!(parse_reply(&cut).is_err());
    }

    #[test]
    fn interpreter_and_module_are_validated_once() {
        assert!(PythonHook::new("python3", "pkg.sub_mod.x1").is_ok());
        assert!(PythonHook::new(std::env::temp_dir().join("python"), "m").is_ok());
        for module in ["", "-c", "a..b", ".a", "a.", "1a", "a b", "a-b", "é"] {
            assert!(PythonHook::new("python3", module).is_err(), "{module:?}");
        }
        let long = "a".repeat(MAX_MODULE_LEN + 1);
        assert!(PythonHook::new("python3", &long).is_err());
        for interpreter in ["", "bin/python", "./python", "../python"] {
            assert!(
                PythonHook::new(interpreter, "m").is_err(),
                "{interpreter:?}"
            );
        }
    }

    #[test]
    fn the_request_is_one_object_with_the_protocol_members() {
        let name = ScheduleName::new("sync").expect("name");
        let payload = json!({"scope": "a"});
        let call = HookCall {
            event: HookEvent::Stop,
            schedule: &name,
            payload: &payload,
            timeout: RunTimeout::new(Duration::from_secs(5)).expect("timeout"),
            deadline: Instant::now(),
        };
        let stamp: Timestamp = "2026-10-25T00:00:30.000000Z".parse().expect("stamp");
        assert_eq!(
            request(&call, stamp),
            json!({
                "protocol": "sc-cron.hook.v1",
                "event": "cron.stop",
                "schedule": "sync",
                "payload": {"scope": "a"},
                "deadline": "2026-10-25T00:00:30.000000Z",
            })
        );
    }
}
