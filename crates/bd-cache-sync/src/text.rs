//! Bounded text for messages and recorded outcomes.

/// `text` cut to at most `limit` bytes on a character boundary, with a marker
/// when something was cut.
pub(crate) fn clip(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    // The last character boundary at or before `limit` (0 is always one).
    let end = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= limit)
        .last()
        .unwrap_or(0);
    let (head, _) = text.split_at(end);
    format!("{head} ... (truncated)")
}

#[cfg(test)]
mod tests {
    use super::clip;

    #[test]
    fn short_text_is_kept_and_long_text_is_cut_on_a_character_boundary() {
        assert_eq!(clip("short", 10), "short");
        let clipped = clip(&"\u{e9}".repeat(100), 11);
        assert!(clipped.ends_with(" ... (truncated)"), "{clipped}");
        assert_eq!(clipped.matches('\u{e9}').count(), 5, "{clipped}");
        assert_eq!(clip("abc", 0), " ... (truncated)");
    }
}
