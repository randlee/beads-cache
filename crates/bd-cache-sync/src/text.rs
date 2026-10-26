//! Bounded text for replies, recorded outcomes and logs (REQ-BDC-0723, REQ-BDC-0724).

/// The most bytes of a child's stderr, or of a hook's result, kept in a reply
/// or in `schedule show` (REQ-BDC-0723, 0724, ADR-SYNC-0008).
pub const MAX_KEPT_STDERR_BYTES: usize = 8 * 1024;

/// `text` cut to at most `limit` bytes on a character boundary, no marker:
/// the result is never longer than `limit`.
pub(crate) fn cut(text: &str, limit: usize) -> &str {
    let (head, _) = text.split_at(text.floor_char_boundary(limit));
    head
}

/// `text` with the value of each variable in `names` (as this process sees
/// it) replaced by `<redacted:NAME>`, then cut to [`MAX_KEPT_STDERR_BYTES`]
/// (REQ-BDC-0724: a script's stderr in a reply never shows an `--env` value).
///
/// Redaction runs before the cut, so a value that straddles the cut is not
/// half-shown. An empty value is not redacted (it would match everywhere).
pub(crate) fn redact_and_cut(text: &str, names: &[String]) -> String {
    let mut out = text.to_owned();
    for name in names {
        if let Ok(value) = std::env::var(name)
            && !value.is_empty()
        {
            out = out.replace(&value, &format!("<redacted:{name}>"));
        }
    }
    cut(&out, MAX_KEPT_STDERR_BYTES).to_owned()
}

#[cfg(test)]
mod tests {
    use super::{cut, redact_and_cut};

    #[test]
    fn text_is_cut_on_a_character_boundary() {
        assert_eq!(cut("\u{e9}\u{e9}", 3), "\u{e9}");
        assert_eq!(cut("abc", 10), "abc");
    }

    /// Over 8 KiB of stderr holding an `--env` value is cut to 8 KiB and
    /// shows `<redacted:NAME>` (acceptance 4); planted negative: with no
    /// name listed the value shows. `PATH` stands for an `--env` variable
    /// (every test process has it, and setting one is `unsafe` in 2024).
    #[test]
    fn stderr_is_redacted_then_cut_to_8_kib() {
        let value = std::env::var("PATH").unwrap();
        assert!(!value.is_empty());
        let text = format!("{value} {}", "x".repeat(10_000));
        let out = redact_and_cut(&text, &["PATH".to_owned()]);
        assert!(out.starts_with("<redacted:PATH> "), "{}", &out[..40]);
        assert!(!out.contains(&value));
        assert_eq!(out.len(), super::MAX_KEPT_STDERR_BYTES);
        let plain = redact_and_cut(&text, &[]);
        assert!(plain.starts_with(&value));
    }
}
