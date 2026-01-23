//! Placeholder Recovery for Translated Variants with Word Reordering
//!
//! This module handles recovery of placeholders ($1, $2, etc.) from anchor tokens (_ID1_, _ID2_, etc.)
//! in translated text, including detection and handling of word-order changes that may occur during
//! machine translation.
//!
//! # Example: Japanese Word Reordering
//!
//! ```ignore
//! Source:        "$1 sent $2"
//! Expanded:      "_ID1_ sent _ID2_"
//! Translated:    "_ID2_ は _ID1_ によって送信された"  (Japanese reorders: agent becomes object)
//! Recovered:     "$2 は $1 によって送信された"
//! ```
//!
//! The recovery process:
//! 1. Locate all anchor tokens in the translated text
//! 2. Map each anchor to its original placeholder index
//! 3. Replace anchors with $N format in their new positions
//! 4. Detect and warn about significant reordering
//! 5. Validate all expected anchors are present

use super::anchor::AnchorToken;
use super::error::{MtError, MtResult};

/// Represents a located anchor token with its position in text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedAnchor {
    /// Original placeholder index (e.g., 1 for $1)
    pub placeholder_index: usize,
    /// Position in the text where anchor token starts
    pub position: usize,
    /// Length of the anchor token string
    pub length: usize,
}

/// Result of placeholder recovery operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryResult {
    /// Text with anchor tokens replaced by placeholders ($1, $2, etc.)
    pub recovered_text: String,
    /// Whether any significant reordering was detected
    pub reordering_detected: bool,
    /// Warnings from the recovery process
    pub warnings: Vec<String>,
}

/// Locate all anchor tokens in a given text
///
/// Scans the text for all anchor tokens from the provided list and returns
/// their positions. This is the first step in placeholder recovery.
///
/// # Arguments
/// * `text` - The text containing anchor tokens
/// * `anchors` - The expected anchor tokens
///
/// # Returns
/// * `Ok(Vec<LocatedAnchor>)` - Found anchors with positions, sorted by position
/// * `Err(MtError::AnchorTokenError)` - If an expected anchor is missing (strict mode)
///
/// # Example
/// ```ignore
/// let text = "_ID2_ は _ID1_ によって";
/// let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
/// let located = locate_anchors_in_text(text, &anchors)?;
/// assert_eq!(located.len(), 2);
/// assert_eq!(located[0].placeholder_index, 2); // First found is _ID2_
/// assert_eq!(located[1].placeholder_index, 1); // Second found is _ID1_
/// ```
pub fn locate_anchors_in_text(text: &str, anchors: &[AnchorToken]) -> MtResult<Vec<LocatedAnchor>> {
    let mut located = Vec::new();
    let mut found_indices = std::collections::HashSet::new();

    // For each anchor, find ALL occurrences in the text
    for anchor in anchors {
        let mut search_start = 0;
        let mut found_any = false;

        // Find all occurrences of this anchor token
        while let Some(pos) = text[search_start..].find(&anchor.token) {
            let absolute_pos = search_start + pos;
            located.push(LocatedAnchor {
                placeholder_index: anchor.placeholder_index,
                position: absolute_pos,
                length: anchor.token.len(),
            });
            found_any = true;
            search_start = absolute_pos + anchor.token.len();
        }

        if found_any {
            found_indices.insert(anchor.placeholder_index);
        }
    }

    // Check if all anchors were found (strict mode: fail if any missing)
    let missing: Vec<usize> = anchors
        .iter()
        .filter(|a| !found_indices.contains(&a.placeholder_index))
        .map(|a| a.placeholder_index)
        .collect();

    if !missing.is_empty() {
        return Err(MtError::AnchorTokenError(format!(
            "Missing anchor tokens for placeholders: {:?}",
            missing
        )));
    }

    // Sort by position for sequential replacement
    located.sort_by_key(|a| a.position);

    Ok(located)
}

/// Detect if anchors have been reordered compared to the source
///
/// Compares the order of located anchors to the expected order from the source.
/// Warning-mode: reordering is detected and reported, but doesn't cause failure.
///
/// # Arguments
/// * `located_anchors` - Anchors found in translated text (sorted by position)
/// * `anchors` - Expected anchors in source order
///
/// # Returns
/// * `Ok(bool)` - True if significant reordering detected, false otherwise
/// * The function never fails in warning mode
///
/// # Example
/// ```ignore
/// // No reordering: anchors appear in same order as source
/// let located = vec![
///     LocatedAnchor { placeholder_index: 1, position: 0, length: 5 },
///     LocatedAnchor { placeholder_index: 2, position: 10, length: 5 },
/// ];
/// let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
/// assert!(!detect_anchor_reordering(&located, &anchors));
///
/// // Reordering: anchors appear in different order
/// let located = vec![
///     LocatedAnchor { placeholder_index: 2, position: 0, length: 5 },
///     LocatedAnchor { placeholder_index: 1, position: 10, length: 5 },
/// ];
/// assert!(detect_anchor_reordering(&located, &anchors));
/// ```
pub fn detect_anchor_reordering(
    located_anchors: &[LocatedAnchor],
    anchors: &[AnchorToken],
) -> bool {
    if located_anchors.is_empty() || anchors.is_empty() {
        return false;
    }

    // Extract the order of placeholder indices from located anchors
    let located_order: Vec<usize> = located_anchors
        .iter()
        .map(|a| a.placeholder_index)
        .collect();

    // Extract the order from expected anchors
    let expected_order: Vec<usize> = anchors.iter().map(|a| a.placeholder_index).collect();

    // Check if the orders match (ignoring anchors that weren't found)
    let mut expected_idx = 0;
    for &located_idx in &located_order {
        while expected_idx < expected_order.len() && expected_order[expected_idx] != located_idx {
            expected_idx += 1;
        }
        if expected_idx >= expected_order.len() {
            // Located index not found in expected order (shouldn't happen with strict locate)
            return true;
        }
        expected_idx += 1;
    }

    false
}

/// Recover placeholders from anchor tokens in translated text
///
/// Replaces all anchor tokens with their original placeholder format ($1, $2, etc.).
/// This handles both simple recovery and word-reordered cases.
///
/// # Arguments
/// * `text` - Text containing anchor tokens
/// * `anchors` - The anchor tokens that were used
///
/// # Returns
/// * `Ok(RecoveryResult)` - Successfully recovered text with metadata
/// * `Err(MtError::AnchorTokenError)` - If required anchors are missing
///
/// # Example
/// ```ignore
/// let text = "_ID2_ は _ID1_ によって";
/// let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
/// let result = recover_placeholders(text, &anchors)?;
/// assert_eq!(result.recovered_text, "$2 は $1 によって");
/// assert!(result.reordering_detected);
/// ```
pub fn recover_placeholders(text: &str, anchors: &[AnchorToken]) -> MtResult<RecoveryResult> {
    // Step 1: Locate all anchors in the text (strict: fails if any missing)
    let located = locate_anchors_in_text(text, anchors)?;

    // Step 2: Detect reordering (warning mode: just report, don't fail)
    let reordering_detected = detect_anchor_reordering(&located, anchors);

    // Step 3: Build a map of position ranges to placeholder strings
    // We need to replace anchors in reverse order of position to avoid offset issues
    let mut replacements: Vec<(usize, usize, String)> = located
        .iter()
        .map(|a| {
            (
                a.position,
                a.position + a.length,
                format!("${}", a.placeholder_index),
            )
        })
        .collect();

    // Sort by position descending so we can replace from end to start
    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    // Step 4: Perform replacements from end to start (to preserve positions)
    let mut recovered = text.to_string();
    for (start, end, placeholder) in replacements {
        recovered.replace_range(start..end, &placeholder);
    }

    // Step 5: Generate warnings if reordering detected
    let warnings = if reordering_detected {
        vec!["Anchor tokens were reordered during translation, which may indicate word-order changes in the target language.".to_string()]
    } else {
        vec![]
    };

    Ok(RecoveryResult {
        recovered_text: recovered,
        reordering_detected,
        warnings,
    })
}

/// Validate placeholder recovery result
///
/// Performs consistency checks on the recovered text to ensure:
/// - All expected placeholders are present
/// - Placeholders match expected format ($N)
/// - No corruption or extra placeholders
///
/// # Arguments
/// * `recovered_text` - Text after placeholder recovery
/// * `anchors` - Original expected anchors
///
/// # Returns
/// * `Ok(RecoveryReport)` - Validation result with status and details
/// * `Err(MtError)` - If validation encounters errors
///
/// # Example
/// ```ignore
/// let text = "$1 sent $2 messages";
/// let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
/// let report = validate_recovery(&text, &anchors)?;
/// assert!(report.all_placeholders_present);
/// assert_eq!(report.validation_warnings.len(), 0);
/// ```
pub fn validate_recovery(
    recovered_text: &str,
    anchors: &[AnchorToken],
) -> MtResult<RecoveryReport> {
    let mut validation_warnings = Vec::new();
    let mut found_placeholders = std::collections::HashSet::new();

    // Find all $N placeholders in the recovered text
    for (i, ch) in recovered_text.char_indices() {
        if ch == '$' && i + 1 < recovered_text.len() {
            let rest = &recovered_text[i + 1..];
            // Try to parse the number
            let mut num_str = String::new();
            for c in rest.chars() {
                if c.is_numeric() {
                    num_str.push(c);
                } else {
                    break;
                }
            }
            if !num_str.is_empty() {
                if let Ok(idx) = num_str.parse::<usize>() {
                    found_placeholders.insert(idx);
                }
            }
        }
    }

    // Check if all expected anchors have corresponding placeholders
    let expected_indices: std::collections::HashSet<usize> =
        anchors.iter().map(|a| a.placeholder_index).collect();

    if found_placeholders != expected_indices {
        let missing: Vec<usize> = expected_indices
            .difference(&found_placeholders)
            .copied()
            .collect();
        if !missing.is_empty() {
            validation_warnings.push(format!(
                "Missing placeholders after recovery: {:?}",
                missing
            ));
        }

        let extra: Vec<usize> = found_placeholders
            .difference(&expected_indices)
            .copied()
            .collect();
        if !extra.is_empty() {
            validation_warnings.push(format!(
                "Unexpected placeholders in recovered text: {:?}",
                extra
            ));
        }
    }

    let all_placeholders_present = found_placeholders == expected_indices;

    Ok(RecoveryReport {
        all_placeholders_present,
        found_placeholder_indices: found_placeholders,
        validation_warnings,
    })
}

/// Report from placeholder recovery validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Whether all expected placeholders are present and no extras
    pub all_placeholders_present: bool,
    /// The set of placeholder indices found in the text
    pub found_placeholder_indices: std::collections::HashSet<usize>,
    /// Validation warnings (empty if no issues)
    pub validation_warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locate_anchors_simple() {
        let text = "_ID1_ sent _ID2_";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let located = locate_anchors_in_text(text, &anchors).unwrap();

        assert_eq!(located.len(), 2);
        assert_eq!(located[0].placeholder_index, 1);
        assert_eq!(located[0].position, 0);
        assert_eq!(located[1].placeholder_index, 2);
        assert_eq!(located[1].position, 11);
    }

    #[test]
    fn test_locate_anchors_multiple_occurrences() {
        let text = "_ID1_ is talking to _ID1_ about _ID2_";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let located = locate_anchors_in_text(text, &anchors).unwrap();

        // Should find 3 total: _ID1_ at pos 0, _ID1_ at pos 20, _ID2_ at pos 32
        assert_eq!(located.len(), 3);
        assert_eq!(located[0].placeholder_index, 1);
        assert_eq!(located[0].position, 0);
        assert_eq!(located[1].placeholder_index, 1);
        assert_eq!(located[1].position, 20);
        assert_eq!(located[2].placeholder_index, 2);
        assert_eq!(located[2].position, 32);
    }

    #[test]
    fn test_locate_anchors_reordered() {
        let text = "_ID2_ は _ID1_ によって";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let located = locate_anchors_in_text(text, &anchors).unwrap();

        assert_eq!(located.len(), 2);
        // First in text order: _ID2_
        assert_eq!(located[0].placeholder_index, 2);
        // Second in text order: _ID1_
        assert_eq!(located[1].placeholder_index, 1);
    }

    #[test]
    fn test_locate_anchors_missing() {
        let text = "_ID1_ has _ID2_ removed";
        let anchors = vec![
            AnchorToken::new(1),
            AnchorToken::new(2),
            AnchorToken::new(3),
        ];
        let result = locate_anchors_in_text(text, &anchors);

        assert!(result.is_err());
        match result.unwrap_err() {
            MtError::AnchorTokenError(msg) => {
                assert!(msg.contains("Missing anchor"));
                assert!(msg.contains("3"));
            }
            _ => panic!("Expected AnchorTokenError"),
        }
    }

    #[test]
    fn test_locate_anchors_empty_text() {
        let text = "";
        let anchors = vec![AnchorToken::new(1)];
        let result = locate_anchors_in_text(text, &anchors);

        assert!(result.is_err());
    }

    #[test]
    fn test_detect_reordering_no_reorder() {
        let located = vec![
            LocatedAnchor {
                placeholder_index: 1,
                position: 0,
                length: 5,
            },
            LocatedAnchor {
                placeholder_index: 2,
                position: 10,
                length: 5,
            },
        ];
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];

        assert!(!detect_anchor_reordering(&located, &anchors));
    }

    #[test]
    fn test_detect_reordering_with_reorder() {
        let located = vec![
            LocatedAnchor {
                placeholder_index: 2,
                position: 0,
                length: 5,
            },
            LocatedAnchor {
                placeholder_index: 1,
                position: 10,
                length: 5,
            },
        ];
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];

        assert!(detect_anchor_reordering(&located, &anchors));
    }

    #[test]
    fn test_recover_placeholders_simple() {
        let text = "_ID1_ sent _ID2_";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let result = recover_placeholders(text, &anchors).unwrap();

        assert_eq!(result.recovered_text, "$1 sent $2");
        assert!(!result.reordering_detected);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_recover_placeholders_reordered_japanese() {
        let text = "_ID2_ は _ID1_ によって送信";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let result = recover_placeholders(text, &anchors).unwrap();

        assert_eq!(result.recovered_text, "$2 は $1 によって送信");
        assert!(result.reordering_detected);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_recover_placeholders_multiple_same_anchor() {
        let text = "_ID1_ is talking to _ID1_ about _ID2_";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let result = recover_placeholders(text, &anchors).unwrap();

        assert_eq!(result.recovered_text, "$1 is talking to $1 about $2");
    }

    #[test]
    fn test_recover_placeholders_missing_anchor_fails() {
        let text = "_ID1_ has _ID2_ only";
        let anchors = vec![
            AnchorToken::new(1),
            AnchorToken::new(2),
            AnchorToken::new(3),
        ];
        let result = recover_placeholders(text, &anchors);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_recovery_all_present() {
        let text = "$1 sent $2 messages";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let report = validate_recovery(&text, &anchors).unwrap();

        assert!(report.all_placeholders_present);
        assert_eq!(report.validation_warnings.len(), 0);
    }

    #[test]
    fn test_validate_recovery_missing_placeholder() {
        let text = "$1 sent messages";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let report = validate_recovery(&text, &anchors).unwrap();

        assert!(!report.all_placeholders_present);
        assert!(!report.validation_warnings.is_empty());
    }

    #[test]
    fn test_validate_recovery_extra_placeholder() {
        let text = "$1 sent $2 to $3";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let report = validate_recovery(&text, &anchors).unwrap();

        assert!(!report.all_placeholders_present);
        assert!(!report.validation_warnings.is_empty());
    }

    #[test]
    fn test_validate_recovery_reordered() {
        let text = "$2 は $1 によって";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let report = validate_recovery(&text, &anchors).unwrap();

        // Both placeholders present, just in different order
        assert!(report.all_placeholders_present);
        assert!(report.validation_warnings.is_empty());
    }

    #[test]
    fn test_roundtrip_recovery_simple() {
        use super::super::anchor::{generate_anchor_tokens, replace_placeholders_with_anchors};

        let original = "Hello, $1! You have $2 messages.";
        let anchors = generate_anchor_tokens(2);

        // Expand to anchors
        let expanded = replace_placeholders_with_anchors(original, &anchors).unwrap();
        assert_eq!(expanded, "Hello, _ID1_! You have _ID2_ messages.");

        // Recover placeholders
        let result = recover_placeholders(&expanded, &anchors).unwrap();
        assert_eq!(result.recovered_text, original);
    }

    #[test]
    fn test_roundtrip_recovery_with_reordering() {
        use super::super::anchor::generate_anchor_tokens;

        let original_expanded = "_ID2_ は _ID1_ によって送信";
        let anchors = generate_anchor_tokens(2);

        // Recover placeholders (even though order is different)
        let result = recover_placeholders(original_expanded, &anchors).unwrap();
        assert_eq!(result.recovered_text, "$2 は $1 によって送信");
        assert!(result.reordering_detected);
    }

    #[test]
    fn test_large_placeholder_indices() {
        let text = "Value: _ID42_ and total: _ID100_";
        let anchors = vec![AnchorToken::new(42), AnchorToken::new(100)];
        let result = recover_placeholders(text, &anchors).unwrap();

        assert_eq!(result.recovered_text, "Value: $42 and total: $100");
    }

    #[test]
    fn test_consecutive_anchors() {
        let text = "_ID1__ID2__ID3_";
        let anchors = vec![
            AnchorToken::new(1),
            AnchorToken::new(2),
            AnchorToken::new(3),
        ];
        let result = recover_placeholders(text, &anchors).unwrap();

        assert_eq!(result.recovered_text, "$1$2$3");
    }

    #[test]
    fn test_partial_recovery_fails_strict() {
        // One anchor missing - strict mode requires all anchors
        let text = "_ID1_ only";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let result = recover_placeholders(text, &anchors);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_with_duplicates() {
        let text = "$1 told $1 that $2 is here";
        let anchors = vec![AnchorToken::new(1), AnchorToken::new(2)];
        let report = validate_recovery(&text, &anchors).unwrap();

        assert!(report.all_placeholders_present);
        assert!(report.validation_warnings.is_empty());
    }
}
