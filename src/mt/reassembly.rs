//! Reassembly Engine for Reconstructing Wikitext from Translated Variants  
//!
//! This module implements the core algorithm from the Python reference implementation
//! for reconstructing wikitext from machine translated variants. The algorithm uses
//! axis-collapsing to systematically combine variants back into structured wikitext.
//!
//! # Algorithm Overview
//!
//! The reassembly follows the Python `Reassembler` class design:
//! 1. **Consistency Check** - Verify MT didn't hallucinate (similarity > 70%)
//! 2. **LCP/LCS Extraction** - Find longest common prefix/suffix across variants
//! 3. **Word Boundary Snapping** - Snap prefix/suffix to clean word boundaries
//! 4. **Axis Collapsing** - Systematically collapse each dimension (GENDER, PLURAL)
//! 5. **Wikitext Reconstruction** - Wrap differences in {{TAG:VAR|opt1|opt2}} format
//!
//! # Python Reference
//!
//! This implementation closely matches the Python lines 199-334:
//! - `Reassembler.reassemble()` - Main entry point with axis collapsing
//! - `Reassembler._collapse_axis()` - Groups variants and collapses one dimension
//! - `Reassembler._fold_strings()` - Extracts LCP/LCS and builds wikitext
//! - Word boundary snapping (lines 278-298 in Python)

use super::data::{MessageContext, TranslationVariant};
use super::error::{MtError, MtResult};
use regex::Regex;
use std::collections::HashMap;

/// Consistency threshold for MT translation similarity
/// Below this threshold, we consider the MT output too inconsistent to reassemble
const CONSISTENCY_THRESHOLD: f32 = 0.7;

/// Reassembler handles reconstruction of wikitext from translated variants
///
/// This struct implements the axis-collapsing algorithm from the Python reference,
/// taking a set of translated variants and systematically combining them back
/// into the original wikitext structure with {{PLURAL|...}} and {{GENDER|...}} syntax.
#[derive(Debug)]
pub struct Reassembler {
    /// Maps variable IDs to their magic word type (e.g., {"$1": "GENDER", "$2": "PLURAL"})
    variable_types: HashMap<String, String>,
}

impl Reassembler {
    /// Create a new reassembler with variable type information
    pub fn new(variable_types: HashMap<String, String>) -> Self {
        Self { variable_types }
    }

    /// Main reassembly entry point - collapses all dimensions
    ///
    /// This function implements the Python `Reassembler.reassemble()` method,
    /// systematically collapsing each axis (variable dimension) until only
    /// a single reconstructed wikitext remains.
    ///
    /// # Arguments
    /// * `variants` - All translated variants with their state information
    ///
    /// # Returns
    /// * `Ok(String)` - Reconstructed wikitext with proper {{TAG:VAR|...}} syntax
    /// * `Err(MtError)` - If inconsistency detected or reassembly fails
    ///
    /// # Algorithm (matches Python lines 204-218)
    /// ```text
    /// 1. Determine axes to collapse (all variable IDs from first variant)
    /// 2. For each axis:
    ///    - Group variants by all other dimensions  
    ///    - Collapse the current axis using LCP/LCS + word boundary snapping
    ///    - Replace group with single "virtual" variant containing wikitext
    /// 3. Restore placeholders (_ID1_ → $1)
    /// ```
    pub fn reassemble(&self, variants: Vec<TranslationVariant>) -> MtResult<String> {
        if variants.is_empty() {
            return Err(MtError::ReassemblyError(
                "No variants to reassemble".to_string(),
            ));
        }

        // Handle single variant case (no magic words)
        if variants.len() == 1 {
            let final_text = &variants[0].translated_text;
            return Ok(self.restore_placeholders(final_text));
        }

        // 1. Determine the axes to collapse (Python line 209)
        let axes: Vec<String> = if variants[0].state.is_empty() {
            // No state means no magic words
            let final_text = &variants[0].translated_text;
            return Ok(self.restore_placeholders(final_text));
        } else {
            variants[0].state.keys().cloned().collect()
        };

        // 2. Collapse each axis one by one (Python lines 212-214)
        let mut current_set = variants;
        for axis in &axes {
            current_set = self.collapse_axis(current_set, axis)?;
        }

        // 3. Should have single variant left after all collapses
        if current_set.len() != 1 {
            return Err(MtError::ReassemblyError(format!(
                "Expected 1 variant after collapse, got {}",
                current_set.len()
            )));
        }

        // 4. Restore placeholders (_ID1_ → $1) - Python line 217
        let final_text = &current_set[0].translated_text;
        Ok(self.restore_placeholders(final_text))
    }

    /// Collapse one axis by grouping variants and folding strings
    ///
    /// This implements the Python `_collapse_axis()` method (lines 220-249),
    /// grouping variants by all dimensions except the current axis, then
    /// folding the strings for each group.
    ///
    /// # Arguments
    /// * `variants` - Current set of variants to collapse
    /// * `axis` - The variable ID to collapse (e.g., "$1", "$2")
    ///
    /// # Returns
    /// * `Ok(Vec<TranslationVariant>)` - New set with this axis collapsed
    /// * `Err(MtError)` - If folding fails due to consistency issues
    fn collapse_axis(
        &self,
        variants: Vec<TranslationVariant>,
        axis: &str,
    ) -> MtResult<Vec<TranslationVariant>> {
        // Group variants by all dimensions EXCEPT the current axis (Python lines 225-231)
        let mut groups: HashMap<Vec<(String, usize)>, Vec<TranslationVariant>> = HashMap::new();

        for variant in variants {
            // Create a key from all state dimensions except the axis we're collapsing
            let mut other_dims: Vec<(String, usize)> = variant
                .state
                .iter()
                .filter(|(k, _)| k.as_str() != axis)
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            other_dims.sort(); // Ensure consistent ordering

            groups
                .entry(other_dims)
                .or_insert_with(Vec::new)
                .push(variant);
        }

        // Collapse each group (Python lines 234-248)
        let mut collapsed = Vec::new();
        for (other_dims, group_members) in groups {
            // Sort members by their index for the current axis (Python line 236)
            let mut sorted_members = group_members;
            sorted_members.sort_by_key(|v| v.state.get(axis).copied().unwrap_or(0));

            // Perform the fold using LCP/LCS (Python line 239)
            let new_text = self.fold_strings(&sorted_members, axis)?;

            // Create a new "virtual" variant for the next iteration (Python lines 242-247)
            let new_state: HashMap<String, usize> = other_dims.into_iter().collect();
            collapsed.push(TranslationVariant {
                state: new_state,
                source_text: String::new(), // Not needed for virtual variants
                translated_text: new_text,
            });
        }

        Ok(collapsed)
    }

    /// Fold a group of strings, wrapping differences in wikitext syntax
    ///
    /// This implements the Python `_fold_strings()` method (lines 251-311),
    /// using LCP/LCS extraction with word boundary snapping to identify
    /// stable and variable parts, then wrapping in {{TAG:VAR|opt1|opt2}} format.
    ///
    /// # Arguments
    /// * `members` - Variants in this group (sorted by axis value)
    /// * `var_id` - Variable ID being collapsed (e.g., "$1")
    ///
    /// # Returns
    /// * `Ok(String)` - Collapsed text with wikitext magic word syntax
    /// * `Err(MtError)` - If consistency check fails
    fn fold_strings(&self, members: &[TranslationVariant], var_id: &str) -> MtResult<String> {
        let texts: Vec<String> = members.iter().map(|m| m.translated_text.clone()).collect();

        // If all texts are identical, no magic word needed (Python line 257-259)
        if texts.len() <= 1 {
            return Ok(texts.first().cloned().unwrap_or_default());
        }

        let all_same = texts.windows(2).all(|w| w[0] == w[1]);
        if all_same {
            return Ok(texts[0].clone());
        }

        // === CONSISTENCY GUARD === (Python lines 263-272)
        // Check similarity between variants - if too different, MT likely hallucinated
        for i in 1..texts.len() {
            let sim = get_similarity(&texts[0], &texts[i]);
            if sim < CONSISTENCY_THRESHOLD {
                return Err(MtError::ConsistencyError(format!(
                    "MT Inconsistency detected on {}. Variants are too different (similarity: {:.1}%):\n1: {}\n2: {}",
                    var_id,
                    sim * 100.0,
                    texts[0],
                    texts[i]
                )));
            }
        }

        // Get raw LCP and LCS (Python lines 275-276)
        let raw_prefix = get_lcp(&texts);
        let raw_suffix = get_lcs(&texts);

        // Snap prefix BACK to last word boundary (Python lines 278-285)
        let prefix = if raw_prefix.is_empty() || raw_prefix.ends_with(' ') {
            raw_prefix
        } else {
            // Find last space and include it
            if let Some(last_space) = raw_prefix.rfind(' ') {
                raw_prefix[..=last_space].to_string()
            } else {
                String::new() // No space found, no prefix
            }
        };

        // Snap suffix FORWARD to first word boundary (Python lines 287-297)
        let suffix = if raw_suffix.is_empty() || raw_suffix.starts_with(' ') {
            raw_suffix
        } else {
            // Find first space
            if let Some(first_space) = raw_suffix.find(' ') {
                raw_suffix[first_space..].to_string()
            } else {
                String::new() // No space found, no suffix
            }
        };

        // Extract the differing "middles" (Python lines 300-305)
        let mut middles = Vec::new();
        for text in &texts {
            let start = prefix.len();
            let end = if suffix.is_empty() {
                text.len()
            } else {
                text.len().saturating_sub(suffix.len())
            };

            let middle = if start <= end {
                text[start..end].to_string()
            } else {
                String::new() // Handle edge case where prefix/suffix overlap
            };
            middles.push(middle);
        }

        // Get tag type and construct wikitext (Python lines 307-311)
        let tag_type = self
            .variable_types
            .get(var_id)
            .cloned()
            .unwrap_or_else(|| "PLURAL".to_string());

        let options = middles.join("|");
        Ok(format!(
            "{}{{{{{}:{}|{}}}}}{}",
            prefix, tag_type, var_id, options, suffix
        ))
    }

    /// Restore placeholders: _ID1_ → $1 (Python lines 329-334)
    fn restore_placeholders(&self, text: &str) -> String {
        let re = Regex::new(r"_ID(\d+)_").unwrap();
        re.replace_all(text, "$$$1").to_string()
    }
}

/// Calculate similarity ratio between two strings using sequence matching
///
/// This implements a simple LCS-based similarity measure similar to Python's
/// `difflib.SequenceMatcher.ratio()` function (Python line 189-190).
///
/// # Arguments
/// * `a` - First string
/// * `b` - Second string
///
/// # Returns
/// Similarity ratio between 0.0 (completely different) and 1.0 (identical)
pub fn get_similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }

    if a.is_empty() && b.is_empty() {
        return 1.0;
    }

    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Convert to character vectors for LCS computation
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    // Calculate LCS length using dynamic programming
    let lcs_length = longest_common_subsequence(&a_chars, &b_chars);

    // Similarity ratio: 2 * LCS / (|a| + |b|)
    // This matches the SequenceMatcher.ratio() formula
    let total_length = a_chars.len() + b_chars.len();
    (2.0 * lcs_length as f32) / total_length as f32
}

/// Get Longest Common Prefix of all strings (Python line 313-320)
fn get_lcp(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    if strings.len() == 1 {
        return strings[0].clone();
    }

    // Find the shortest string length
    let min_len = strings.iter().map(|s| s.len()).min().unwrap_or(0);

    // Find common prefix length
    let mut prefix_len = 0;
    'outer: for i in 0..min_len {
        let first_char = strings[0].chars().nth(i);
        for string in &strings[1..] {
            if string.chars().nth(i) != first_char {
                break 'outer;
            }
        }
        prefix_len = i + 1;
    }

    strings[0].chars().take(prefix_len).collect()
}

/// Get Longest Common Suffix by reversing and using LCP (Python lines 322-327)
fn get_lcs(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    // Reverse all strings
    let reversed: Vec<String> = strings.iter().map(|s| s.chars().rev().collect()).collect();

    // Get LCP of reversed strings
    let lcp_reversed = get_lcp(&reversed);

    // Reverse the result back
    lcp_reversed.chars().rev().collect()
}

/// Calculate longest common subsequence length using dynamic programming
fn longest_common_subsequence(a: &[char], b: &[char]) -> usize {
    let m = a.len();
    let n = b.len();

    if m == 0 || n == 0 {
        return 0;
    }

    // DP table: dp[i][j] = LCS length of a[0..i] and b[0..j]
    let mut dp = vec![vec![0; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    dp[m][n]
}

/// Convenience function to reassemble variants from a MessageContext
///
/// This function provides a high-level interface matching the expected
/// workflow from the integration tests.
pub fn reassemble_from_context(context: &MessageContext) -> MtResult<String> {
    let reassembler = Reassembler::new(context.variable_types.clone());
    reassembler.reassemble(context.variants.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper to create a variant with state and translation
    fn create_variant(state: &[(&str, usize)], translated_text: &str) -> TranslationVariant {
        let state_map: HashMap<String, usize> =
            state.iter().map(|(k, v)| (k.to_string(), *v)).collect();

        TranslationVariant::with_translation(
            state_map,
            String::new(), // source_text not used in reassembly
            translated_text.to_string(),
        )
    }

    // ========== LCP/LCS Tests ==========

    #[test]
    fn test_get_lcp_identical() {
        let strings = vec!["hello world".to_string(), "hello world".to_string()];
        let lcp = get_lcp(&strings);
        assert_eq!(lcp, "hello world");
    }

    #[test]
    fn test_get_lcp_partial() {
        let strings = vec!["hello world".to_string(), "hello everyone".to_string()];
        let lcp = get_lcp(&strings);
        assert_eq!(lcp, "hello ");
    }

    #[test]
    fn test_get_lcp_no_common() {
        let strings = vec!["abc".to_string(), "xyz".to_string()];
        let lcp = get_lcp(&strings);
        assert_eq!(lcp, "");
    }

    #[test]
    fn test_get_lcp_empty() {
        let strings: Vec<String> = vec![];
        let lcp = get_lcp(&strings);
        assert_eq!(lcp, "");
    }

    #[test]
    fn test_get_lcp_single() {
        let strings = vec!["hello".to_string()];
        let lcp = get_lcp(&strings);
        assert_eq!(lcp, "hello");
    }

    #[test]
    fn test_get_lcs_identical() {
        let strings = vec!["hello world".to_string(), "hello world".to_string()];
        let lcs = get_lcs(&strings);
        assert_eq!(lcs, "hello world");
    }

    #[test]
    fn test_get_lcs_partial() {
        let strings = vec!["say hello".to_string(), "big hello".to_string()];
        let lcs = get_lcs(&strings);
        assert_eq!(lcs, " hello");
    }

    #[test]
    fn test_get_lcs_no_common() {
        let strings = vec!["abc".to_string(), "xyz".to_string()];
        let lcs = get_lcs(&strings);
        assert_eq!(lcs, "");
    }

    // ========== Similarity Tests ==========

    #[test]
    fn test_get_similarity_identical() {
        assert_eq!(get_similarity("hello", "hello"), 1.0);
    }

    #[test]
    fn test_get_similarity_completely_different() {
        let sim = get_similarity("abc", "xyz");
        assert!(sim < 0.5); // Should be low similarity
    }

    #[test]
    fn test_get_similarity_partial() {
        // "hello world" vs "hello there" should have reasonable similarity
        let sim = get_similarity("hello world", "hello there");
        assert!(sim > 0.4 && sim < 0.8);
    }

    #[test]
    fn test_get_similarity_empty() {
        assert_eq!(get_similarity("", ""), 1.0);
        assert_eq!(get_similarity("abc", ""), 0.0);
        assert_eq!(get_similarity("", "xyz"), 0.0);
    }

    // ========== LCS Algorithm Tests ==========

    #[test]
    fn test_longest_common_subsequence() {
        let a: Vec<char> = "ABCDGH".chars().collect();
        let b: Vec<char> = "AEDFHR".chars().collect();
        let lcs = longest_common_subsequence(&a, &b);
        assert_eq!(lcs, 3); // "ADH"
    }

    #[test]
    fn test_longest_common_subsequence_identical() {
        let a: Vec<char> = "hello".chars().collect();
        let b: Vec<char> = "hello".chars().collect();
        let lcs = longest_common_subsequence(&a, &b);
        assert_eq!(lcs, 5);
    }

    #[test]
    fn test_longest_common_subsequence_empty() {
        let a: Vec<char> = vec![];
        let b: Vec<char> = "hello".chars().collect();
        let lcs = longest_common_subsequence(&a, &b);
        assert_eq!(lcs, 0);
    }

    // ========== Word Boundary Snapping Tests ==========

    #[test]
    fn test_fold_strings_word_boundary_snapping() {
        let mut var_types = HashMap::new();
        var_types.insert("$1".to_string(), "GENDER".to_string());
        let reassembler = Reassembler::new(var_types);

        // Test case: prefix should snap back to word boundary
        let variants = vec![
            create_variant(&[("$1", 0)], "He sent a message"),
            create_variant(&[("$1", 1)], "She sent a message"),
        ];

        let result = reassembler.fold_strings(&variants, "$1").unwrap();

        // Should be "{{GENDER:$1|He|She}} sent a message"
        // NOT "{{GENDER:$1|He s|She s}}ent a message" (broken word boundary)
        assert!(result.contains("He|She"));
        assert!(result.contains("sent a message"));
        assert!(!result.contains("He s|She s")); // Should not break words
    }

    // ========== Consistency Checking Tests ==========

    #[test]
    fn test_consistency_error_detection() {
        let mut var_types = HashMap::new();
        var_types.insert("$1".to_string(), "GENDER".to_string());
        let reassembler = Reassembler::new(var_types);

        // Create variants that are too different (low similarity)
        let variants = vec![
            create_variant(&[("$1", 0)], "He sent a message"),
            create_variant(&[("$1", 1)], "Completely different sentence"), // Very different
        ];

        let result = reassembler.fold_strings(&variants, "$1");

        assert!(result.is_err());
        match result {
            Err(MtError::ConsistencyError(msg)) => {
                assert!(msg.contains("MT Inconsistency"));
                assert!(msg.contains("$1"));
            }
            _ => panic!("Expected ConsistencyError"),
        }
    }

    #[test]
    fn test_consistency_passes_similar_variants() {
        let mut var_types = HashMap::new();
        var_types.insert("$1".to_string(), "GENDER".to_string());
        let reassembler = Reassembler::new(var_types);

        // Create variants that are similar enough
        let variants = vec![
            create_variant(&[("$1", 0)], "He sent a message"),
            create_variant(&[("$1", 1)], "She sent a message"), // Very similar
        ];

        let result = reassembler.fold_strings(&variants, "$1");
        assert!(result.is_ok());
    }

    // ========== Placeholder Restoration Tests ==========

    #[test]
    fn test_restore_placeholders() {
        let reassembler = Reassembler::new(HashMap::new());

        let text = "_ID1_ sent _ID2_ messages to _ID3_";
        let result = reassembler.restore_placeholders(text);
        assert_eq!(result, "$1 sent $2 messages to $3");
    }

    #[test]
    fn test_restore_placeholders_no_anchors() {
        let reassembler = Reassembler::new(HashMap::new());

        let text = "No anchors here";
        let result = reassembler.restore_placeholders(text);
        assert_eq!(result, "No anchors here");
    }

    #[test]
    fn test_restore_placeholders_mixed() {
        let reassembler = Reassembler::new(HashMap::new());

        let text = "_ID1_ and normal _ID text and _ID2_";
        let result = reassembler.restore_placeholders(text);
        assert_eq!(result, "$1 and normal _ID text and $2");
    }

    // ========== Simple Reassembly Tests ==========

    #[test]
    fn test_reassemble_single_variant() {
        let reassembler = Reassembler::new(HashMap::new());

        let variants = vec![create_variant(&[], "Hello _ID1_!")];

        let result = reassembler.reassemble(variants).unwrap();
        assert_eq!(result, "Hello $1!");
    }

    #[test]
    fn test_reassemble_identical_variants() {
        let mut var_types = HashMap::new();
        var_types.insert("$1".to_string(), "GENDER".to_string());
        let reassembler = Reassembler::new(var_types);

        // If all variants are identical, should just return the text
        let variants = vec![
            create_variant(&[("$1", 0)], "Same message"),
            create_variant(&[("$1", 1)], "Same message"),
        ];

        let result = reassembler.reassemble(variants).unwrap();
        assert_eq!(result, "Same message"); // No magic word needed
    }

    #[test]
    fn test_reassemble_empty_variants() {
        let reassembler = Reassembler::new(HashMap::new());

        let result = reassembler.reassemble(vec![]);
        assert!(result.is_err());
        match result {
            Err(MtError::ReassemblyError(msg)) => {
                assert!(msg.contains("No variants"));
            }
            _ => panic!("Expected ReassemblyError"),
        }
    }

    // ========== Integration Test ==========

    #[test]
    fn test_reassemble_gender_variants() {
        let mut var_types = HashMap::new();
        var_types.insert("$1".to_string(), "GENDER".to_string());
        let reassembler = Reassembler::new(var_types);

        let variants = vec![
            create_variant(&[("$1", 0)], "He sent a message"),
            create_variant(&[("$1", 1)], "She sent a message"),
            create_variant(&[("$1", 2)], "They sent a message"),
        ];

        let result = reassembler.reassemble(variants).unwrap();

        // Should reconstruct: "{{GENDER:$1|He|She|They}} sent a message"
        assert!(result.contains("{{GENDER:$1|"));
        assert!(result.contains("|He|She|They}"));
        assert!(result.contains("}} sent a message"));
    }

    // ========== MessageContext Convenience Test ==========

    #[test]
    fn test_reassemble_from_context() {
        let mut context = MessageContext::new("test".to_string());
        context.add_variable("$1".to_string(), "GENDER".to_string());

        let variants = vec![
            create_variant(&[("$1", 0)], "He is here"),
            create_variant(&[("$1", 1)], "She is here"),
        ];

        for variant in variants {
            context.add_variant(variant);
        }

        let result = reassemble_from_context(&context).unwrap();

        assert!(result.contains("{{GENDER:$1|"));
        assert!(result.contains("|He|She}"));
        assert!(result.contains("}} is here"));
    }
}
