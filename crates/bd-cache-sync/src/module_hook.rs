//! Isolated dotted-module launcher over upstream's supervisor and hook types.
//!
//! Upstream `PythonHook` launches script paths. Cache consumers register importable
//! modules instead; this adapter only supplies argv and the `HookCall` JSON wire
//! projection. Parsing, output limits, process-tree ownership and killing stay
//! upstream. No scheduler or supervisor implementation is duplicated.

use sc_cron::process::{ChildEnv, ChildSpec, TempCwd, supervise};
use sc_cron::{
    Clock, CronError, HOOK_PROTOCOL, HookCall, HookOutput, HookStatus, MAX_HOOK_OUTPUT_BYTES,
    Timestamp, kill_budget,
};
use std::path::PathBuf;

/// An isolated Python module invocation using the upstream hook protocol.
#[derive(Debug, Clone)]
pub struct ModuleHook {
    pub(crate) interpreter: PathBuf,
    pub(crate) module: String,
    pub(crate) env: ChildEnv,
}

impl ModuleHook {
    /// Invokes a validated consumer module with `-I -m`, under the call budget.
    ///
    /// # Errors
    /// Returns upstream `HookFailed` if the temporary directory or child cannot
    /// be started. Exit failures and timeouts remain upstream `HookOutput` values.
    pub(crate) async fn invoke<C: Clock + ?Sized>(
        &self,
        clock: &C,
        call: &HookCall<'_>,
    ) -> Result<HookOutput, CronError> {
        let now = clock.monotonic_now();
        let budget = kill_budget(call.timeout.get(), call.deadline, now);
        if budget.is_zero() {
            return Ok(HookOutput {
                status: HookStatus::TimedOut,
                stdout: String::new(),
                stderr: String::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            });
        }
        let remaining = call.deadline.saturating_duration_since(now);
        let wall_deadline = chrono::Duration::from_std(remaining)
            .ok()
            .and_then(|d| clock.now().checked_add_signed(d))
            .unwrap_or(chrono::DateTime::<chrono::Utc>::MAX_UTC);
        // HookCall's upstream sc-cron.hook.v1 projection (request is private
        // upstream). HookOutput owns all response validation.
        let input = serde_json::json!({ "protocol": HOOK_PROTOCOL, "event": call.event.as_str(), "schedule": call.schedule.as_str(), "payload": call.payload, "deadline": Timestamp::from_datetime(wall_deadline).to_string() }).to_string();
        let cwd = TempCwd::new()
            .map_err(|e| CronError::HookFailed(format!("no temporary directory: {e}")))?;
        let spec = ChildSpec {
            program: &self.interpreter,
            args: vec!["-I".into(), "-m".into(), self.module.clone().into()],
            cwd: cwd.path(),
            input: input.as_bytes(),
            env: self.env.clone(),
        };
        let captured = supervise(&spec, MAX_HOOK_OUTPUT_BYTES, tokio::time::sleep(budget))
            .await
            .map_err(|e| CronError::HookFailed(format!("isolated module {}: {e}", self.module)))?;
        Ok(HookOutput {
            status: captured
                .status
                .map_or(HookStatus::TimedOut, HookStatus::Exited),
            stdout: String::from_utf8_lossy(&captured.stdout.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&captured.stderr.bytes).into_owned(),
            stdout_truncated: captured.stdout.truncated,
            stderr_truncated: captured.stderr.truncated,
        })
    }
}
