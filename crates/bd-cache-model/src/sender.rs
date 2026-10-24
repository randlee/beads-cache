//! The escalation delivery port; a-7 implements it with `atm send`.

use std::fmt;

/// Delivery failed; the tick reports `refused` and retries next tick (REQ-BDC-0706).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError {
    /// What went wrong.
    pub detail: String,
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "message not delivered: {}", self.detail)
    }
}

impl std::error::Error for SendError {}

/// Sends one message to a phase lead.
pub trait EscalationSender: Send + Sync {
    /// Delivers `body` to `lead` in `team`.
    ///
    /// # Errors
    ///
    /// [`SendError`] when the message was not delivered.
    fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError>;
}
