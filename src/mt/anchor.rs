/// Anchor Token System for protecting placeholders during machine translation
///
/// Anchor tokens are unique, non-translatable strings used to replace placeholders ($1, $2, etc.)
/// before sending text to the machine translation system. This prevents the MT system from
/// translating the placeholder numbers (e.g., "1" → "un" in French).
///
/// Format: _ID{index}_ where index is the placeholder number (1-indexed)
/// Examples: _ID1_, _ID2_, _ID3_, etc.
use crate::mt::error::MtResult;

/// An anchor token is a unique, non-translatable string used to mark placeholder positions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorToken {
    /// The placeholder index (1 for $1, 2 for $2, etc.)
    pub placeholder_index: usize,
    /// The unique anchor token string
    pub token: String,
}

impl AnchorToken {
    /// Create a new anchor token
    pub fn new(placeholder_index: usize) -> Self {
        AnchorToken {
            placeholder_index,
            token: format!("_ID{}_", placeholder_index),
        }
    }

    /// Get the placeholder pattern this token replaces (e.g., "$1")
    pub fn placeholder_pattern(&self) -> String {
        format!("${}", self.placeholder_index)
    }
}

/// Generate anchor tokens for a given count
///
/// # Arguments
/// * `count` - Number of anchor tokens to generate
///
/// # Returns
/// A vector of anchor tokens for $1, $2, ..., $count
///
/// # Example
/// ```ignore
/// let tokens = generate_anchor_tokens(3);
/// assert_eq!(tokens.len(), 3);
/// assert_eq!(tokens[0].placeholder_index, 1);
/// assert_eq!(tokens[0].token, "_ID1_");
/// ```
pub fn generate_anchor_tokens(count: usize) -> Vec<AnchorToken> {
    (1..=count).map(AnchorToken::new).collect()
}

/// Replace placeholders with anchor tokens in text
///
/// Scans through the text and replaces all occurrences of $1, $2, etc. with their
/// corresponding anchor tokens. This prevents machine translation systems from
/// attempting to translate the placeholder numbers.
///
/// # Arguments
/// * `text` - The input text containing placeholders
/// * `anchors` - The anchor tokens to use for replacement
///
/// # Returns
/// Text with all placeholders replaced by anchor tokens
///
/// # Example
/// ```ignore
/// let text = "Hello, $1! You have $2 messages.";
/// let anchors = generate_anchor_tokens(2);
/// let result = replace_placeholders_with_anchors(text, &anchors)?;
/// assert_eq!(result, "Hello, _ID1_! You have _ID2_ messages.");
/// ```
pub fn replace_placeholders_with_anchors(text: &str, anchors: &[AnchorToken]) -> MtResult<String> {
    let mut result = text.to_string();

    // Replace placeholders in order, starting from the highest index to avoid conflicts
    // For example, replace $10 before $1 to avoid "$1" being replaced in "$10"
    let mut sorted_anchors = anchors.to_vec();
    sorted_anchors.sort_by(|a, b| b.placeholder_index.cmp(&a.placeholder_index));

    for anchor in sorted_anchors {
        let placeholder = anchor.placeholder_pattern();
        result = result.replace(&placeholder, &anchor.token);
    }

    Ok(result)
}

/// Recover placeholders from anchor tokens in translated text
///
/// Reverses the replacement done by `replace_placeholders_with_anchors`.
/// Scans through the text and replaces all anchor tokens back to their original
/// placeholder format ($1, $2, etc.).
///
/// # Arguments
/// * `text` - The translated text containing anchor tokens
/// * `anchors` - The anchor tokens that were used
///
/// # Returns
/// Text with all anchor tokens replaced by original placeholders
///
/// # Example
/// ```ignore
/// let translated = "_ID1_ a envoyé _ID2_ messages.";
/// let anchors = generate_anchor_tokens(2);
/// let result = recover_placeholders_from_anchors(&translated, &anchors)?;
/// assert_eq!(result, "$1 a envoyé $2 messages.");
/// ```
pub fn recover_placeholders_from_anchors(text: &str, anchors: &[AnchorToken]) -> MtResult<String> {
    let mut result = text.to_string();

    for anchor in anchors {
        // Replace each anchor token with its original placeholder
        result = result.replace(&anchor.token, &anchor.placeholder_pattern());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_token_creation() {
        let token = AnchorToken::new(1);
        assert_eq!(token.placeholder_index, 1);
        assert_eq!(token.token, "_ID1_");
        assert_eq!(token.placeholder_pattern(), "$1");
    }

    #[test]
    fn test_anchor_token_creation_large_index() {
        let token = AnchorToken::new(42);
        assert_eq!(token.placeholder_index, 42);
        assert_eq!(token.token, "_ID42_");
        assert_eq!(token.placeholder_pattern(), "$42");
    }

    #[test]
    fn test_generate_anchor_tokens() {
        let tokens = generate_anchor_tokens(5);
        assert_eq!(tokens.len(), 5);
        for (i, token) in tokens.iter().enumerate() {
            assert_eq!(token.placeholder_index, i + 1);
        }
    }

    #[test]
    fn test_generate_anchor_tokens_zero() {
        let tokens = generate_anchor_tokens(0);
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_generate_anchor_tokens_uniqueness() {
        let tokens = generate_anchor_tokens(10);
        let tokens_set: std::collections::HashSet<_> = tokens.iter().map(|t| &t.token).collect();
        assert_eq!(tokens_set.len(), 10, "All tokens should be unique");
    }

    #[test]
    fn test_single_placeholder_replacement() {
        let text = "Hello, $1!";
        let anchors = generate_anchor_tokens(1);
        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        assert_eq!(result, "Hello, _ID1_!");
    }

    #[test]
    fn test_multiple_placeholders_replacement() {
        let text = "$1 sent $2 messages to $3.";
        let anchors = generate_anchor_tokens(3);
        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        assert_eq!(result, "_ID1_ sent _ID2_ messages to _ID3_.");
    }

    #[test]
    fn test_placeholder_replacement_with_duplicates() {
        let text = "$1 is talking to $1 about $2.";
        let anchors = generate_anchor_tokens(2);
        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        assert_eq!(result, "_ID1_ is talking to _ID1_ about _ID2_.");
    }

    #[test]
    fn test_no_placeholder_replacement() {
        let text = "Hello, World!";
        let anchors = generate_anchor_tokens(1);
        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_placeholder_replacement_order_matters() {
        // Make sure we replace $10 before $1 to avoid conflicts
        let text = "$1 and $10 are different.";
        let anchors = generate_anchor_tokens(10);
        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        // Should have _ID1_ and _ID10_, not _ID10_ becoming _ID1_0_
        assert!(result.contains("_ID1_"));
        assert!(result.contains("_ID10_"));
        assert!(!result.contains("_ID1_0"));
    }

    #[test]
    fn test_single_placeholder_recovery() {
        let text = "Hello, _ID1_!";
        let anchors = generate_anchor_tokens(1);
        let result = recover_placeholders_from_anchors(text, &anchors).unwrap();
        assert_eq!(result, "Hello, $1!");
    }

    #[test]
    fn test_multiple_placeholders_recovery() {
        let text = "_ID1_ sent _ID2_ messages to _ID3_.";
        let anchors = generate_anchor_tokens(3);
        let result = recover_placeholders_from_anchors(text, &anchors).unwrap();
        assert_eq!(result, "$1 sent $2 messages to $3.");
    }

    #[test]
    fn test_placeholder_recovery_with_duplicates() {
        let text = "_ID1_ is talking to _ID1_ about _ID2_.";
        let anchors = generate_anchor_tokens(2);
        let result = recover_placeholders_from_anchors(text, &anchors).unwrap();
        assert_eq!(result, "$1 is talking to $1 about $2.");
    }

    #[test]
    fn test_recovery_handles_spaces() {
        let text = "_ID1_ _ID2_ _ID3_";
        let anchors = generate_anchor_tokens(3);
        let result = recover_placeholders_from_anchors(text, &anchors).unwrap();
        // After recovery: "$1 $2 $3"
        assert_eq!(result, "$1 $2 $3");
    }

    #[test]
    fn test_roundtrip_single_placeholder() {
        let original = "Hello, $1!";
        let anchors = generate_anchor_tokens(1);

        let expanded = replace_placeholders_with_anchors(original, &anchors).unwrap();
        let recovered = recover_placeholders_from_anchors(&expanded, &anchors).unwrap();

        assert_eq!(recovered, original);
    }

    #[test]
    fn test_roundtrip_multiple_placeholders() {
        let original = "$1 sent $2 messages to $3.";
        let anchors = generate_anchor_tokens(3);

        let expanded = replace_placeholders_with_anchors(original, &anchors).unwrap();
        let recovered = recover_placeholders_from_anchors(&expanded, &anchors).unwrap();

        assert_eq!(recovered, original);
    }

    #[test]
    fn test_roundtrip_complex_message() {
        let original = "Hi $1, you have $2 new messages from $3 colleagues.";
        let anchors = generate_anchor_tokens(3);

        let expanded = replace_placeholders_with_anchors(original, &anchors).unwrap();
        let recovered = recover_placeholders_from_anchors(&expanded, &anchors).unwrap();

        assert_eq!(recovered, original);
    }

    #[test]
    fn test_mt_placeholder_reordering_japanese() {
        // Simulating Japanese word order change (SOV to different order)
        let original = "$1 sent $2";
        let anchors = generate_anchor_tokens(2);

        // Expand
        let expanded = replace_placeholders_with_anchors(original, &anchors).unwrap();
        assert_eq!(expanded, "_ID1_ sent _ID2_");

        // Simulate MT reordering: Japanese might reorder to "$2 は $1 によって送信"
        // In anchor form, that would be "_ID2_ は _ID1_ によって送信"
        let mt_reordered = "_ID2_ は _ID1_ によって送信";

        // Even with reordering, we should recover the anchors
        let recovered = recover_placeholders_from_anchors(mt_reordered, &anchors).unwrap();
        assert_eq!(recovered, "$2 は $1 によって送信");
    }

    #[test]
    fn test_partial_anchors_vector() {
        // What if we only provide anchors for $1 and $2, but text has $3?
        let text = "$1 sent $2 to $3";
        let anchors = generate_anchor_tokens(2); // Only 2 anchors

        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        // $1 and $2 should be replaced, $3 should remain
        assert_eq!(result, "_ID1_ sent _ID2_ to $3");
    }

    #[test]
    fn test_anchor_tokens_dont_conflict_with_regular_text() {
        // Make sure anchor tokens don't accidentally match regular text
        let text = "The ID1 code is _ID1_ value";
        let anchors = generate_anchor_tokens(1);

        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        // ID1 (without underscore and $ prefix) should not be affected
        assert!(result.contains("The ID1"));
    }

    #[test]
    fn test_special_characters_in_placeholders_context() {
        let text = "$1's message to $2: \"Hello!\"";
        let anchors = generate_anchor_tokens(2);

        let expanded = replace_placeholders_with_anchors(text, &anchors).unwrap();
        let recovered = recover_placeholders_from_anchors(&expanded, &anchors).unwrap();

        assert_eq!(recovered, text);
    }

    #[test]
    fn test_edge_case_empty_text() {
        let text = "";
        let anchors = generate_anchor_tokens(1);

        let result = replace_placeholders_with_anchors(text, &anchors).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_edge_case_only_placeholder() {
        let text = "$1";
        let anchors = generate_anchor_tokens(1);

        let expanded = replace_placeholders_with_anchors(text, &anchors).unwrap();
        assert_eq!(expanded, "_ID1_");

        let recovered = recover_placeholders_from_anchors(&expanded, &anchors).unwrap();
        assert_eq!(recovered, "$1");
    }
}
