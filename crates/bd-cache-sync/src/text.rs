//! Bounded, redacted text for replies, recorded outcomes and logs (REQ-BDC-0723, REQ-BDC-0724).
//!
//! Anything a child process wrote (a hook's or a script's stderr, the error
//! message of a hook's answer, an outcome sc-cron built from a child's
//! stderr) goes through a [`Redactor`] before it reaches a reply, a stored
//! outcome, `hook_state.last_error` or a log line: the value of each of the
//! row's `--env` variables is replaced by `<redacted:NAME>`.

use std::borrow::Cow;

/// The most bytes of a child's stderr, or of a hook's result, kept in a reply
/// or in `schedule show` (REQ-BDC-0723, 0724, ADR-SYNC-0008).
pub const MAX_KEPT_STDERR_BYTES: usize = 8 * 1024;

/// The most bytes of a child's message kept in an error's `message` (the
/// part of its stderr or answer that explains a failure).
pub(crate) const MAX_MESSAGE_BYTES: usize = 512;

/// `text` cut to at most `limit` bytes on a character boundary, no marker:
/// the result is never longer than `limit`.
pub(crate) fn cut(text: &str, limit: usize) -> &str {
    let (head, _) = text.split_at(text.floor_char_boundary(limit));
    head
}

/// The values of a row's `--env` variables, read once from this process's
/// environment, and the redaction of child-derived text with them.
#[derive(Clone, Default)]
pub(crate) struct Redactor {
    /// `(value, "<redacted:NAME>")`, longest value first so a value that
    /// contains another is replaced whole.
    pairs: Vec<(String, String)>,
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the values: they are the secrets this type hides.
        f.debug_struct("Redactor")
            .field("names", &self.pairs.len())
            .finish()
    }
}

impl Redactor {
    /// The redactor of `names` as this process sees them. An unset or empty
    /// variable is not redacted (an empty value would match everywhere).
    pub(crate) fn of(names: &[String]) -> Self {
        let mut pairs: Vec<(String, String)> = names
            .iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|v| !v.is_empty())
                    .map(|v| (v, format!("<redacted:{name}>")))
            })
            .collect();
        pairs.sort_by_key(|(value, _)| std::cmp::Reverse(value.len()));
        Self { pairs }
    }

    /// `text` with every value replaced; borrowed when nothing matched.
    pub(crate) fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut out = Cow::Borrowed(text);
        for (value, marker) in &self.pairs {
            if out.contains(value.as_str()) {
                out = Cow::Owned(out.replace(value.as_str(), marker));
            }
        }
        out
    }

    /// [`Redactor::redact`], then cut to `limit` bytes. Redaction runs
    /// first, so a value that straddles the cut is not half-shown.
    pub(crate) fn redact_and_cut(&self, text: &str, limit: usize) -> String {
        cut(&self.redact(text), limit).to_owned()
    }

    /// A child's stderr as a reply keeps it: redacted, then cut to
    /// [`MAX_KEPT_STDERR_BYTES`] (REQ-BDC-0724).
    pub(crate) fn stderr(&self, text: &str) -> String {
        self.redact_and_cut(text, MAX_KEPT_STDERR_BYTES)
    }

    /// A child-derived message for an error's `message`, a stored outcome or
    /// a log line: redacted, then cut to [`MAX_MESSAGE_BYTES`].
    pub(crate) fn message(&self, text: &str) -> String {
        self.redact_and_cut(text, MAX_MESSAGE_BYTES)
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_KEPT_STDERR_BYTES, MAX_MESSAGE_BYTES, Redactor, cut};

    #[test]
    fn text_is_cut_on_a_character_boundary() {
        assert_eq!(cut("\u{e9}\u{e9}", 3), "\u{e9}");
        assert_eq!(cut("abc", 10), "abc");
    }

    /// Over 8 KiB of stderr holding an `--env` value is cut to 8 KiB and
    /// shows `<redacted:NAME>`, at the start and past the message's cut
    /// (acceptance 4); planted negatives: with no name listed the value
    /// shows, and without the cut the text keeps its length. `PATH` stands
    /// for an `--env` variable (every test process has it, and setting one
    /// is `unsafe` in 2024).
    #[test]
    fn stderr_is_redacted_then_cut_to_8_kib() {
        let value = std::env::var("PATH").unwrap_or_default();
        assert!(!value.is_empty());
        let text = format!("{value} {} {value}", "x".repeat(10_000));
        let r = Redactor::of(&["PATH".to_owned()]);
        let out = r.stderr(&text);
        assert!(out.starts_with("<redacted:PATH> "), "{}", cut(&out, 40));
        assert!(!out.contains(&value));
        assert_eq!(out.len(), MAX_KEPT_STDERR_BYTES);
        let message = r.message(&text);
        assert!(message.len() <= MAX_MESSAGE_BYTES && !message.contains(&value));
        let plain = Redactor::of(&[]).stderr(&text);
        assert!(plain.starts_with(&value));
        assert!(r.redact(&text).len() > MAX_KEPT_STDERR_BYTES);
        assert!(!format!("{r:?}").contains(&value), "Debug hides the values");
    }

    /// Nothing to redact borrows the input.
    #[test]
    fn unredacted_text_is_borrowed() {
        let r = Redactor::of(&["PATH".to_owned()]);
        assert!(matches!(
            r.redact("nothing here"),
            std::borrow::Cow::Borrowed(_)
        ));
    }
}
