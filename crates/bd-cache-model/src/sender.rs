//! The escalation delivery port; a-7 implements it with `atm send`.

use std::fmt;
use std::time::Duration;

/// The rule a member name (a lead or a team) follows, in words: the one text
/// every message about it uses. A name that breaks it could be read as an option
/// or split in two when it becomes part of an `atm send` address.
pub const MEMBER_NAME_RULE: &str =
    "not empty, with no whitespace, control character or '@', and not starting with '-'";

/// Why a message was not delivered.
///
/// The kind says whether trying again can help; [`SendErrorKind::fix`] says
/// what to do (a-7 round 1, RBP-F004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SendErrorKind {
    /// The address cannot be used as given; retrying the same message cannot help.
    InvalidAddress,
    /// The delivery program could not be started (missing, not executable).
    NotStartable,
    /// The delivery did not finish within its time limit and was stopped.
    TimedOut,
    /// The delivery was stopped because the host is shutting down.
    Cancelled,
    /// The delivery program ran and reported a failure.
    Exited,
    /// Any other failure (the default for a sender that does not classify).
    Other,
}

impl serde::Serialize for SendErrorKind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl SendErrorKind {
    /// The stable name of the kind, for reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidAddress => "invalid_address",
            Self::NotStartable => "not_startable",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::Exited => "exited",
            Self::Other => "other",
        }
    }

    /// What the operator can do about it.
    #[must_use]
    pub fn fix(self) -> &'static str {
        match self {
            Self::InvalidAddress => {
                "correct the lead or team name in the configuration or phase file: it must be nonempty, with no whitespace, control character or '@', and must not start with '-'"
            }
            Self::NotStartable => "install atm or set its path, and check it is executable",
            Self::TimedOut => {
                "check that atm and its daemon answer; the message is retried next tick"
            }
            Self::Cancelled => "none; the message is retried by the next process",
            Self::Exited => "read the program's message in the detail, fix the mailbox or team",
            Self::Other => "check the sender; the message is retried next tick",
        }
    }
}

/// Delivery failed; the tick reports `refused` and retries next tick (REQ-BDC-0706).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError {
    /// What went wrong.
    pub detail: String,
    /// Which kind of failure it was.
    pub kind: SendErrorKind,
}

impl SendError {
    /// An unclassified failure ([`SendErrorKind::Other`]).
    #[must_use]
    pub fn new(detail: impl Into<String>) -> Self {
        Self::with_kind(SendErrorKind::Other, detail)
    }

    /// A failure of the given kind.
    #[must_use]
    pub fn with_kind(kind: SendErrorKind, detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            kind,
        }
    }

    /// What the operator can do about it ([`SendErrorKind::fix`]).
    #[must_use]
    pub fn fix(&self) -> &'static str {
        self.kind.fix()
    }
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "message not delivered: {}", self.detail)
    }
}

impl std::error::Error for SendError {}

/// Sends one message to a phase lead.
///
/// `lead` and `team` are member names the caller has already validated
/// (bd-cache-sync's `MemberName`: nonempty, no whitespace, control character or
/// `@`, no leading `-`); the port does not repeat the check. An implementation
/// that passes them to a program or a network address keeps its own defensive
/// guard, as `AtmSender` does, and answers [`SendErrorKind::InvalidAddress`].
pub trait EscalationSender: Send + Sync {
    /// Delivers `body` to `lead` in `team`.
    ///
    /// # Errors
    ///
    /// [`SendError`] when the message was not delivered.
    fn send(&self, lead: &str, team: &str, body: &str) -> Result<(), SendError>;

    /// Delivers `body` within `timeout`, stopping the delivery when it runs out.
    ///
    /// A scheduler's tick has a budget; it passes the time it has left. The
    /// provided body ignores `timeout` and calls [`send`](Self::send); a sender
    /// that can bound its work (a child process, a network call) overrides it.
    ///
    /// # Errors
    ///
    /// [`SendError`] when the message was not delivered, including
    /// [`SendErrorKind::TimedOut`] when `timeout` ran out.
    fn send_within(
        &self,
        lead: &str,
        team: &str,
        body: &str,
        timeout: Duration,
    ) -> Result<(), SendError> {
        let _ = timeout;
        self.send(lead, team, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The operator reads the fix: it states the rule in words and names no
    /// Rust identifier (RBP-F032).
    #[test]
    fn the_invalid_address_fix_states_the_rule_without_rust_identifiers() {
        let fix = SendErrorKind::InvalidAddress.fix();
        assert!(!fix.contains("MEMBER_NAME_RULE"), "{fix}");
        for part in ["whitespace", "control character", "'@'", "'-'"] {
            assert!(fix.contains(part), "{fix} lacks {part}");
            assert!(MEMBER_NAME_RULE.contains(part), "the rule lacks {part}");
        }
    }
}
