/// Keyword detection for voice-triggered actions.
///
/// Scans the transcription for a trigger phrase anywhere in the text
/// (start, middle, or end). Supports formats with or without "as":
///   - "... rephrase as email"
///   - "rephrase this email. ..."
///   - "... some text, rephrase it, more text ..."

pub struct KeywordMatch {
    pub action: String,
    /// Optional format specifier: "email", "message", "slack message", etc.
    pub format: Option<String>,
    pub clean_text: String,
}

/// Trigger verbs mapped to actions, ordered longest-first for greedy matching.
const TRIGGER_VERBS: &[(&str, &str)] = &[
    ("rephrase this as", "rephrase"),
    ("rephrase it as", "rephrase"),
    ("rewrite this as", "rephrase"),
    ("rewrite it as", "rephrase"),
    ("format this as", "rephrase"),
    ("format it as", "rephrase"),
    ("rephrase as", "rephrase"),
    ("rewrite as", "rephrase"),
    ("format as", "rephrase"),
    ("rephrase this", "rephrase"),
    ("rephrase it", "rephrase"),
    ("rewrite this", "rephrase"),
    ("rewrite it", "rephrase"),
    ("format this", "rephrase"),
    ("format it", "rephrase"),
    ("rephrase", "rephrase"),
    ("rewrite", "rephrase"),
    ("format", "rephrase"),
];

/// Known format specifiers.
const KNOWN_FORMATS: &[&str] = &[
    "professional email",
    "formal email",
    "casual message",
    "text message",
    "slack message",
    "teams message",
    "chat message",
    "bullet points",
    "code comment",
    "email",
    "message",
    "professional",
    "formal",
    "casual",
    "friendly",
    "bullets",
    "summary",
    "comment",
];

/// Detect a trigger keyword anywhere in the transcription.
///
/// Scans for the longest matching trigger verb, then checks if a known
/// format follows (with or without "as"). Everything outside the keyword
/// span becomes the clean_text.
///
/// Returns `None` if no keyword matches or there's no content left.
pub fn detect_keyword(text: &str) -> Option<KeywordMatch> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_lowercase();

    // Try each trigger verb (longest first = most specific first).
    // For each verb, find all occurrences and pick the best match.
    let mut best: Option<Match> = None;

    for &(verb, action) in TRIGGER_VERBS {
        let mut search_from = 0;
        while let Some(rel_pos) = lower[search_from..].find(verb) {
            let start = search_from + rel_pos;
            let verb_end = start + verb.len();

            // Word boundary check: verb must not be part of a larger word
            if start > 0 {
                let prev = lower.as_bytes()[start - 1];
                if prev.is_ascii_alphanumeric() {
                    search_from = verb_end;
                    continue;
                }
            }
            if verb_end < lower.len() {
                let next = lower.as_bytes()[verb_end];
                if next.is_ascii_alphanumeric() {
                    search_from = verb_end;
                    continue;
                }
            }

            // Determine the full keyword span (verb + optional format)
            let (span_end, format) = extract_format(&lower, verb_end, verb.ends_with(" as"));

            let candidate = Match {
                start,
                end: span_end,
                action,
                format,
            };

            // Prefer: longer verb span > earlier position
            if best.as_ref().map_or(true, |b| candidate.is_better_than(b)) {
                best = Some(candidate);
            }

            search_from = verb_end;
        }
    }

    let m = best?;

    // Build clean_text from everything outside the keyword span
    let before = trimmed[..m.start]
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_punctuation() || c == ',')
        .trim();
    let after = trimmed[m.end..]
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_punctuation() || c == ',')
        .trim();

    let clean_text = match (before.is_empty(), after.is_empty()) {
        (true, true) => return None,
        (true, false) => after.to_string(),
        (false, true) => before.to_string(),
        (false, false) => format!("{} {}", before, after),
    };

    Some(KeywordMatch {
        action: m.action.to_string(),
        format: m.format,
        clean_text,
    })
}

struct Match<'a> {
    start: usize,
    end: usize,
    action: &'a str,
    format: Option<String>,
}

impl Match<'_> {
    fn span_len(&self) -> usize {
        self.end - self.start
    }

    fn is_better_than(&self, other: &Match) -> bool {
        // Prefer longer span (more specific), then earlier position
        if self.span_len() != other.span_len() {
            self.span_len() > other.span_len()
        } else {
            self.start < other.start
        }
    }
}

/// Look for a known format after the verb.
///
/// If `verb_has_as` is true, the verb already ends with "as" so we look
/// for a format directly. Otherwise we look for either a direct format
/// or skip it (plain verb).
fn extract_format(lower: &str, verb_end: usize, verb_has_as: bool) -> (usize, Option<String>) {
    let remainder = &lower[verb_end..];
    let trimmed_remainder = remainder
        .trim_start()
        .trim_start_matches(|c: char| c == '.' || c == ',');
    let skip = remainder.len() - trimmed_remainder.len();

    if verb_has_as {
        // Verb ends with "as" — look for format directly
        if let Some((fmt, fmt_len)) = match_format(trimmed_remainder) {
            return (verb_end + skip + fmt_len, Some(fmt));
        }
    } else {
        // Check for "as {format}" or direct "{format}" after the verb
        let after = trimmed_remainder.trim_start();
        let extra_skip = trimmed_remainder.len() - after.len();

        // Try "as {format}"
        if after.starts_with("as ") {
            let after_as = &after[3..];
            if let Some((fmt, fmt_len)) = match_format(after_as) {
                return (verb_end + skip + extra_skip + 3 + fmt_len, Some(fmt));
            }
        }

        // Try direct "{format}" (e.g. "rephrase this email")
        if let Some((fmt, fmt_len)) = match_format(after) {
            // Make sure the format is followed by a boundary (punctuation, whitespace, or end)
            let after_fmt = &after[fmt_len..];
            if after_fmt.is_empty()
                || after_fmt.starts_with(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            {
                return (verb_end + skip + extra_skip + fmt_len, Some(fmt));
            }
        }
    }

    // No format found — plain verb
    (verb_end, None)
}

/// Try to match a known format at the start of the string.
/// Returns the format name and the byte length consumed.
fn match_format(text: &str) -> Option<(String, usize)> {
    let text_clean = text
        .trim_end_matches(|c: char| c.is_ascii_punctuation())
        .trim();

    // Try longest formats first (KNOWN_FORMATS is ordered longest-first)
    for &fmt in KNOWN_FORMATS {
        if text_clean.starts_with(fmt) {
            // Ensure it's not a prefix of a longer word
            let after = &text_clean[fmt.len()..];
            if after.is_empty()
                || after.starts_with(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            {
                // Find the actual byte position in the original text
                let pos = text.to_lowercase().find(fmt)?;
                return Some((fmt.to_string(), pos + fmt.len()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Trailing triggers ───────────────────────────────────────────────────

    #[test]
    fn trailing_rephrase() {
        let m = detect_keyword("I want to follow up on our meeting. Rephrase").unwrap();
        assert_eq!(m.action, "rephrase");
        assert!(m.format.is_none());
        assert_eq!(m.clean_text, "I want to follow up on our meeting");
    }

    #[test]
    fn trailing_rephrase_it() {
        let m = detect_keyword("Hello world, rephrase it").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Hello world");
    }

    #[test]
    fn trailing_rewrite() {
        let m = detect_keyword("Fix the bug in the login flow. Rewrite").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Fix the bug in the login flow");
    }

    #[test]
    fn trailing_rephrase_as_email() {
        let m = detect_keyword("Meeting is moved to 3pm. Rephrase as email").unwrap();
        assert_eq!(m.format.as_deref(), Some("email"));
        assert_eq!(m.clean_text, "Meeting is moved to 3pm");
    }

    #[test]
    fn trailing_format_as_slack_message() {
        let m = detect_keyword("Build is broken. Format this as slack message").unwrap();
        assert_eq!(m.format.as_deref(), Some("slack message"));
        assert_eq!(m.clean_text, "Build is broken");
    }

    #[test]
    fn trailing_rephrase_as_professional_email() {
        let m = detect_keyword("I can't make it tomorrow. Rephrase as professional email").unwrap();
        assert_eq!(m.format.as_deref(), Some("professional email"));
        assert_eq!(m.clean_text, "I can't make it tomorrow");
    }

    #[test]
    fn trailing_format_as_bullet_points() {
        let m = detect_keyword("We discussed roadmap and budget. Format as bullet points").unwrap();
        assert_eq!(m.format.as_deref(), Some("bullet points"));
        assert_eq!(m.clean_text, "We discussed roadmap and budget");
    }

    // ── Leading triggers ────────────────────────────────────────────────────

    #[test]
    fn leading_rephrase() {
        let m = detect_keyword("Rephrase. I want to follow up on our meeting").unwrap();
        assert_eq!(m.action, "rephrase");
        assert!(m.format.is_none());
        assert_eq!(m.clean_text, "I want to follow up on our meeting");
    }

    #[test]
    fn leading_rephrase_it() {
        let m = detect_keyword("Rephrase it. Hello world and thanks").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Hello world and thanks");
    }

    #[test]
    fn leading_rephrase_as_email() {
        let m = detect_keyword("Rephrase as email. Hi Ryan please find the attached").unwrap();
        assert_eq!(m.format.as_deref(), Some("email"));
        assert_eq!(m.clean_text, "Hi Ryan please find the attached");
    }

    #[test]
    fn leading_format_as_message() {
        let m = detect_keyword("Format as message, tell John the deploy is done").unwrap();
        assert_eq!(m.format.as_deref(), Some("message"));
        assert_eq!(m.clean_text, "tell John the deploy is done");
    }

    // ── "verb {format}" without "as" ────────────────────────────────────────

    #[test]
    fn leading_rephrase_this_email() {
        let m = detect_keyword("Rephrase this email. Hi Ryan, how are you? I just wanted to check on the documents.").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.format.as_deref(), Some("email"));
        assert_eq!(m.clean_text, "Hi Ryan, how are you? I just wanted to check on the documents.");
    }

    #[test]
    fn trailing_rephrase_this_email() {
        let m = detect_keyword("Hi Ryan, how are you? Please review the docs. Rephrase this email").unwrap();
        assert_eq!(m.format.as_deref(), Some("email"));
        assert_eq!(m.clean_text, "Hi Ryan, how are you? Please review the docs");
    }

    #[test]
    fn leading_format_this_message() {
        let m = detect_keyword("Format this message. Hey John the build is done and deployed").unwrap();
        assert_eq!(m.format.as_deref(), Some("message"));
        assert_eq!(m.clean_text, "Hey John the build is done and deployed");
    }

    #[test]
    fn leading_rewrite_this_professional_email() {
        let m = detect_keyword("Rewrite this professional email. We need to postpone the deadline").unwrap();
        assert_eq!(m.format.as_deref(), Some("professional email"));
        assert_eq!(m.clean_text, "We need to postpone the deadline");
    }

    // ── Middle of text ──────────────────────────────────────────────────────

    #[test]
    fn middle_rephrase() {
        let m = detect_keyword("Hello team, rephrase it, we need to ship by Friday").unwrap();
        assert_eq!(m.action, "rephrase");
        assert!(m.format.is_none());
        assert_eq!(m.clean_text, "Hello team we need to ship by Friday");
    }

    #[test]
    fn middle_rephrase_as_email() {
        let m = detect_keyword("Hi Ryan, rephrase as email, please review the documents").unwrap();
        assert_eq!(m.format.as_deref(), Some("email"));
        assert_eq!(m.clean_text, "Hi Ryan please review the documents");
    }

    // ── Edge cases ──────────────────────────────────────────────────────────

    #[test]
    fn case_insensitive() {
        let m = detect_keyword("Some text REPHRASE IT").unwrap();
        assert_eq!(m.clean_text, "Some text");
    }

    #[test]
    fn case_insensitive_with_format() {
        let m = detect_keyword("Some text REPHRASE AS EMAIL").unwrap();
        assert_eq!(m.format.as_deref(), Some("email"));
        assert_eq!(m.clean_text, "Some text");
    }

    #[test]
    fn strips_trailing_punctuation() {
        let m = detect_keyword("Some text. Rephrase it.").unwrap();
        assert_eq!(m.clean_text, "Some text");
    }

    #[test]
    fn no_match_without_keyword() {
        assert!(detect_keyword("Hello world").is_none());
    }

    #[test]
    fn no_match_keyword_only() {
        assert!(detect_keyword("rephrase").is_none());
        assert!(detect_keyword("Rephrase it").is_none());
        assert!(detect_keyword("rewrite").is_none());
        assert!(detect_keyword("format as email").is_none());
        assert!(detect_keyword("rephrase this email").is_none());
    }

    #[test]
    fn no_match_empty() {
        assert!(detect_keyword("").is_none());
        assert!(detect_keyword("   ").is_none());
    }

    #[test]
    fn prefers_longest_match() {
        // "rephrase it" should match over just "rephrase"
        let m = detect_keyword("My text rephrase it").unwrap();
        assert_eq!(m.clean_text, "My text");
    }

    // ── Real Whisper transcription patterns ─────────────────────────────────

    #[test]
    fn whisper_keyword_at_start() {
        let m = detect_keyword(
            "Rephrase it. Hi Ryan, please find attached presentations for the project."
        ).unwrap();
        assert_eq!(m.action, "rephrase");
        assert!(m.format.is_none());
        assert_eq!(m.clean_text, "Hi Ryan, please find attached presentations for the project.");
    }

    #[test]
    fn whisper_format_at_start() {
        let m = detect_keyword(
            "Format as email. Hey I wanted to let you know the deadline moved"
        ).unwrap();
        assert_eq!(m.format.as_deref(), Some("email"));
        assert_eq!(m.clean_text, "Hey I wanted to let you know the deadline moved");
    }

    #[test]
    fn whisper_rephrase_this_email_at_start() {
        let m = detect_keyword(
            "Rephrase this email. Hi Ryan, how are you? Hope you are doing well. I just wanted to check that have you reviewed the documents which I had sent for the small business accelerator? Thank you."
        ).unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.format.as_deref(), Some("email"));
    }
}
