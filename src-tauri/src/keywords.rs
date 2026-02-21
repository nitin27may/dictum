/// Keyword detection for voice-triggered actions.
///
/// Checks if a transcription ends with a trigger keyword (e.g. "rephrase", "rewrite")
/// and returns the action + cleaned text with the keyword stripped.

pub struct KeywordMatch {
    pub action: String,
    pub clean_text: String,
}

/// Trigger phrases mapped to actions, ordered longest-first for greedy matching.
const TRIGGERS: &[(&str, &str)] = &[
    ("rephrase it", "rephrase"),
    ("rewrite it", "rephrase"),
    ("rephrase", "rephrase"),
    ("rewrite", "rephrase"),
];

/// Detect a trailing keyword in the transcription text.
///
/// Returns `None` if:
/// - No keyword matches
/// - The text is *only* the keyword (no content to act on)
pub fn detect_keyword(text: &str) -> Option<KeywordMatch> {
    // Strip trailing punctuation and whitespace for matching
    let trimmed = text
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_punctuation())
        .trim();

    let lower = trimmed.to_lowercase();

    for &(phrase, action) in TRIGGERS {
        if lower.ends_with(phrase) {
            let prefix = trimmed[..trimmed.len() - phrase.len()]
                .trim()
                .trim_end_matches(|c: char| c.is_ascii_punctuation() || c == ',')
                .trim();

            // If nothing left after stripping keyword, there's nothing to rephrase
            if prefix.is_empty() {
                return None;
            }

            return Some(KeywordMatch {
                action: action.to_string(),
                clean_text: prefix.to_string(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rephrase_at_end() {
        let m = detect_keyword("I want to follow up on our meeting. Rephrase").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "I want to follow up on our meeting.");
    }

    #[test]
    fn detects_rephrase_it_at_end() {
        let m = detect_keyword("Hello world, rephrase it").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Hello world");
    }

    #[test]
    fn detects_rewrite_at_end() {
        let m = detect_keyword("Fix the bug in the login flow. Rewrite").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Fix the bug in the login flow.");
    }

    #[test]
    fn detects_rewrite_it_at_end() {
        let m = detect_keyword("Send the report to the team, rewrite it").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Send the report to the team");
    }

    #[test]
    fn case_insensitive() {
        let m = detect_keyword("Some text REPHRASE IT").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Some text");
    }

    #[test]
    fn strips_trailing_punctuation() {
        let m = detect_keyword("Some text. Rephrase it.").unwrap();
        assert_eq!(m.action, "rephrase");
        assert_eq!(m.clean_text, "Some text.");
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
    }

    #[test]
    fn no_match_empty() {
        assert!(detect_keyword("").is_none());
        assert!(detect_keyword("   ").is_none());
    }

    #[test]
    fn prefers_longest_match() {
        // "rephrase it" should match, not just "rephrase" leaving "it" in clean_text
        let m = detect_keyword("My text rephrase it").unwrap();
        assert_eq!(m.clean_text, "My text");
    }
}
