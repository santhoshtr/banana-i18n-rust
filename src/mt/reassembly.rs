//! Reassembly Engine for Reconstructing Wikitext from Translated Variants
//!
//! This module reconstructs the original wikitext structure from translated variants by:
//! 1. Finding stable (unchanging) and variable (changing) parts across variants
//! 2. Aligning variable parts with the source AST magic words
//! 3. Detecting scope expansion when MT changes words outside magic word boundaries
//! 4. Reconstructing complete wikitext with appropriate magic word syntax

use super::error::{MtError, MtResult};
use crate::ast::{AstNode, AstNodeList};
use std::collections::HashMap;

/// Result of reassembly operation
#[derive(Debug, Clone)]
pub struct ReassemblyResult {
    /// Full reconstructed wikitext with {{PLURAL|...}} and {{GENDER|...}} syntax
    pub reconstructed_wikitext: String,
    /// Forms extracted for each magic word position
    pub extracted_forms: Vec<ExtractedForms>,
    /// Detected scope changes (expansions outside magic word boundaries)
    pub scope_changes: Vec<ScopeChange>,
    /// User-facing warnings about the reassembly
    pub warnings: Vec<String>,
    /// Confidence score from 0.0 to 1.0
    pub confidence: f32,
}

/// Forms extracted for a single magic word position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedForms {
    /// Position in the original AST
    pub ast_index: usize,
    /// Type of magic word: "PLURAL" or "GENDER"
    pub magic_word_type: String,
    /// Extracted translated forms for this magic word
    pub forms: Vec<String>,
}

/// Detected scope change (expansion outside magic word boundary)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeChange {
    /// Position of the magic word in the original AST
    pub ast_index: usize,
    /// Original character range in source wikitext
    pub original_range: (usize, usize),
    /// Expanded character range after scope widening
    pub expanded_range: (usize, usize),
    /// Reason for the scope expansion
    pub reason: String,
}

/// Represents a stable (unchanging) part across all variants
#[derive(Debug, Clone, PartialEq, Eq)]
struct StablePart {
    /// Character position in the variant string
    position: usize,
    /// The unchanging text
    text: String,
}

/// Represents a variable (changing) part corresponding to a magic word
#[derive(Debug, Clone, PartialEq, Eq)]
struct VariablePart {
    /// Character position in the variant string
    position: usize,
    /// Which magic word this corresponds to (index in AST)
    ast_index: usize,
    /// All forms for this magic word (extracted from variants)
    forms: Vec<String>,
}

/// Alignment information between variants
#[derive(Debug)]
struct Alignment {
    /// Parts that don't change across all variants
    stable_parts: Vec<StablePart>,
    /// Parts that change (corresponding to magic words)
    #[allow(dead_code)]
    variable_parts: Vec<VariablePart>,
}

/// Main reassembly entry point
///
/// Takes source variants, translated variants, and source AST, then reconstructs wikitext.
///
/// # Arguments
/// * `source_ast` - Original parsed message AST
/// * `source_variants` - Plain-text variants generated from source (with anchor tokens)
/// * `translated_variants` - Translated variants from MT provider (with anchor tokens)
/// * `_locale` - Target locale for reference
///
/// # Returns
/// * `Ok(ReassemblyResult)` - Successfully reconstructed with extracted forms and metadata
/// * `Err(MtError)` - If reassembly fails due to inconsistency or structural issues
pub fn reassemble(
    source_ast: &AstNodeList,
    source_variants: &[String],
    translated_variants: &[String],
    _locale: &str,
) -> MtResult<ReassemblyResult> {
    // Validate inputs
    if source_variants.is_empty() || translated_variants.is_empty() {
        return Err(MtError::ReassemblyError(
            "Empty variants provided".to_string(),
        ));
    }

    if source_variants.len() != translated_variants.len() {
        return Err(MtError::ReassemblyError(format!(
            "Source ({}) and translated ({}) variant counts mismatch",
            source_variants.len(),
            translated_variants.len()
        )));
    }

    // Step 1: Find stable and variable parts in translated variants
    let alignment = find_stable_and_variable_parts(translated_variants)?;

    // Step 2: Map variable parts to source AST
    let extracted_forms =
        map_variable_parts_to_ast(source_ast, &alignment, source_variants, translated_variants)?;

    // Step 3: Detect scope changes
    let scope_changes = detect_scope_changes(source_variants, translated_variants, &alignment)?;

    // Step 4: Reconstruct wikitext
    let reconstructed_wikitext =
        reconstruct_wikitext(source_ast, &extracted_forms, &scope_changes)?;

    // Step 5: Calculate confidence score
    let confidence = calculate_confidence(&alignment, &scope_changes, translated_variants);

    // Generate warnings
    let warnings = generate_warnings(&scope_changes);

    Ok(ReassemblyResult {
        reconstructed_wikitext,
        extracted_forms,
        scope_changes,
        warnings,
        confidence,
    })
}

/// Find stable and variable parts across all translated variants
fn find_stable_and_variable_parts(variants: &[String]) -> MtResult<Alignment> {
    if variants.is_empty() {
        return Err(MtError::ReassemblyError(
            "Cannot analyze empty variants".to_string(),
        ));
    }

    // Handle single variant (all parts are stable)
    if variants.len() == 1 {
        return Ok(Alignment {
            stable_parts: vec![StablePart {
                position: 0,
                text: variants[0].clone(),
            }],
            variable_parts: vec![],
        });
    }

    let mut stable_parts = Vec::new();
    let mut variable_parts = Vec::new();

    // Convert variants to character vectors for easier comparison
    let variant_chars: Vec<Vec<char>> = variants.iter().map(|v| v.chars().collect()).collect();

    // Find minimum length (pad shorter variants conceptually)
    let max_len = variant_chars.iter().map(|v| v.len()).max().unwrap_or(0);

    let mut current_pos = 0;
    let mut in_stable_segment = true;
    let mut current_stable_text = String::new();

    for char_idx in 0..max_len {
        // Get character from each variant at this position
        let chars_at_pos: Vec<Option<char>> = variant_chars
            .iter()
            .map(|v| v.get(char_idx).copied())
            .collect();

        // Check if all variants have the same character at this position
        let all_same = chars_at_pos.iter().all(|c| {
            if let Some(&first_char) = chars_at_pos.first() {
                c == &first_char
            } else {
                false
            }
        });

        if all_same && !chars_at_pos.is_empty() && chars_at_pos[0].is_some() {
            // Character is stable
            if in_stable_segment {
                if let Some(ch) = chars_at_pos[0] {
                    current_stable_text.push(ch);
                }
            } else {
                // Transitioning from variable to stable
                in_stable_segment = true;
                if let Some(ch) = chars_at_pos[0] {
                    current_stable_text = ch.to_string();
                }
            }
        } else {
            // Character is variable (or end of variant)
            if in_stable_segment {
                // Transitioning from stable to variable
                if !current_stable_text.is_empty() {
                    stable_parts.push(StablePart {
                        position: current_pos,
                        text: current_stable_text.clone(),
                    });
                }
                in_stable_segment = false;
                current_stable_text.clear();
                current_pos = char_idx;
            }
        }
    }

    // Handle final stable segment
    if in_stable_segment && !current_stable_text.is_empty() {
        stable_parts.push(StablePart {
            position: current_pos,
            text: current_stable_text,
        });
    }

    // Extract variable segments by finding gaps between stable parts
    if stable_parts.len() < variants.len().saturating_sub(1) {
        // We need to identify variable segments
        variable_parts = extract_variable_segments(&variant_chars, &stable_parts)?;
    }

    Ok(Alignment {
        stable_parts,
        variable_parts,
    })
}

/// Extract variable segments from variants given stable parts
fn extract_variable_segments(
    variant_chars: &[Vec<char>],
    _stable_parts: &[StablePart],
) -> MtResult<Vec<VariablePart>> {
    let mut variable_parts = Vec::new();
    let mut var_index = 0;

    if variant_chars.is_empty() {
        return Ok(variable_parts);
    }

    let max_len = variant_chars.iter().map(|v| v.len()).max().unwrap_or(0);
    let mut current_var_start = 0;
    let mut in_variable_segment = false;

    for char_idx in 0..=max_len {
        if char_idx < max_len {
            // Check if all variants have same character
            let chars: Vec<Option<char>> = variant_chars
                .iter()
                .map(|vc| vc.get(char_idx).copied())
                .collect();

            let all_same = chars.iter().all(|c| {
                if let Some(&first) = chars.first() {
                    c == &first
                } else {
                    false
                }
            });

            if !all_same {
                if !in_variable_segment {
                    in_variable_segment = true;
                    current_var_start = char_idx;
                }
            } else {
                // End of variable segment
                if in_variable_segment && char_idx > current_var_start {
                    let forms: Vec<String> = variant_chars
                        .iter()
                        .map(|vc| {
                            vc[current_var_start..char_idx.min(vc.len())]
                                .iter()
                                .collect()
                        })
                        .collect();

                    // Check if forms actually differ
                    let all_same = forms.iter().all(|f| f == &forms[0]);
                    if !all_same {
                        variable_parts.push(VariablePart {
                            position: current_var_start,
                            ast_index: var_index,
                            forms,
                        });
                        var_index += 1;
                    }
                }
                in_variable_segment = false;
            }
        } else {
            // End of variants
            if in_variable_segment && char_idx > current_var_start {
                let forms: Vec<String> = variant_chars
                    .iter()
                    .map(|vc| {
                        vc[current_var_start..current_var_start.max(vc.len())]
                            .iter()
                            .collect()
                    })
                    .collect();

                let all_same = forms.iter().all(|f| f == &forms[0]);
                if !all_same {
                    variable_parts.push(VariablePart {
                        position: current_var_start,
                        ast_index: var_index,
                        forms,
                    });
                }
            }
        }
    }

    Ok(variable_parts)
}

/// Map variable parts to source AST magic words
fn map_variable_parts_to_ast(
    source_ast: &AstNodeList,
    _alignment: &Alignment,
    _source_variants: &[String],
    translated_variants: &[String],
) -> MtResult<Vec<ExtractedForms>> {
    let mut extracted_forms = Vec::new();

    // Count magic words in AST
    let mut magic_word_indices = Vec::new();
    for (idx, node) in source_ast.iter().enumerate() {
        if let AstNode::Transclusion(trans) = node {
            let name_upper = trans.name.to_uppercase();
            if name_upper == "PLURAL" || name_upper == "GENDER" {
                magic_word_indices.push((idx, name_upper));
            }
        }
    }

    // Map magic words to translated variants
    for (var_idx, (ast_idx, magic_type)) in magic_word_indices.iter().enumerate() {
        if var_idx < translated_variants.len() {
            // For now, use the entire translated variant as a single form
            // In a more sophisticated implementation, we'd extract individual forms
            extracted_forms.push(ExtractedForms {
                ast_index: *ast_idx,
                magic_word_type: magic_type.clone(),
                forms: vec![translated_variants[var_idx].clone()],
            });
        }
    }

    Ok(extracted_forms)
}

/// Detect scope changes (when MT changes words outside magic word boundaries)
fn detect_scope_changes(
    source_variants: &[String],
    translated_variants: &[String],
    _alignment: &Alignment,
) -> MtResult<Vec<ScopeChange>> {
    let mut scope_changes = Vec::new();

    if source_variants.len() < 2 || translated_variants.len() < 2 {
        return Ok(scope_changes);
    }

    // Compare consecutive variants to find unexpected changes
    for var_idx in 0..source_variants.len().saturating_sub(1) {
        let source_curr = &source_variants[var_idx];
        let source_next = &source_variants[var_idx + 1];
        let trans_curr = &translated_variants[var_idx];
        let trans_next = &translated_variants[var_idx + 1];

        // Find positions where source differs
        let source_diff_ranges = find_change_ranges(source_curr, source_next);
        // Find positions where translation differs
        let trans_diff_ranges = find_change_ranges(trans_curr, trans_next);

        // If translation differs in more places or larger ranges, scope expanded
        if trans_diff_ranges.len() > source_diff_ranges.len()
            || trans_diff_ranges.iter().map(|(a, b)| b - a).sum::<usize>()
                > source_diff_ranges.iter().map(|(a, b)| b - a).sum::<usize>()
        {
            // Calculate scope expansion
            let orig_range = if source_diff_ranges.is_empty() {
                (0, 0)
            } else {
                let min_start = source_diff_ranges
                    .iter()
                    .map(|(s, _)| *s)
                    .min()
                    .unwrap_or(0);
                let max_end = source_diff_ranges
                    .iter()
                    .map(|(_, e)| *e)
                    .max()
                    .unwrap_or(0);
                (min_start, max_end)
            };

            let expanded_range = if trans_diff_ranges.is_empty() {
                (0, 0)
            } else {
                let min_start = trans_diff_ranges.iter().map(|(s, _)| *s).min().unwrap_or(0);
                let max_end = trans_diff_ranges.iter().map(|(_, e)| *e).max().unwrap_or(0);
                (min_start, max_end)
            };

            if orig_range != expanded_range {
                scope_changes.push(ScopeChange {
                    ast_index: var_idx,
                    original_range: orig_range,
                    expanded_range,
                    reason: "Translation changed words outside original magic word boundary"
                        .to_string(),
                });
            }
        }
    }

    Ok(scope_changes)
}

/// Find character ranges where two strings differ
fn find_change_ranges(str1: &str, str2: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();

    let chars1: Vec<char> = str1.chars().collect();
    let chars2: Vec<char> = str2.chars().collect();

    let mut in_diff = false;
    let mut diff_start = 0;

    for i in 0..chars1.len().max(chars2.len()) {
        let c1 = chars1.get(i);
        let c2 = chars2.get(i);

        let chars_differ = c1 != c2;

        if chars_differ && !in_diff {
            in_diff = true;
            diff_start = i;
        } else if !chars_differ && in_diff {
            in_diff = false;
            ranges.push((diff_start, i));
        }
    }

    if in_diff {
        ranges.push((diff_start, chars1.len().max(chars2.len())));
    }

    ranges
}

/// Reconstruct full wikitext from extracted forms
fn reconstruct_wikitext(
    source_ast: &AstNodeList,
    extracted_forms: &[ExtractedForms],
    _scope_changes: &[ScopeChange],
) -> MtResult<String> {
    let mut result = String::new();
    let mut form_map: HashMap<usize, &ExtractedForms> = HashMap::new();

    // Create a map for fast lookup
    for forms in extracted_forms {
        form_map.insert(forms.ast_index, forms);
    }

    for (idx, node) in source_ast.iter().enumerate() {
        match node {
            AstNode::Text(text) => result.push_str(text),
            AstNode::Placeholder(placeholder) => {
                result.push('$');
                result.push_str(&placeholder.index.to_string());
            }
            AstNode::Transclusion(trans) => {
                let name_upper = trans.name.to_uppercase();

                if let Some(forms) = form_map.get(&idx) {
                    // Reconstruct magic word syntax
                    if forms.forms.is_empty() {
                        return Err(MtError::ReassemblyError(
                            "No forms extracted for magic word".to_string(),
                        ));
                    }

                    result.push_str("{{");
                    result.push_str(&forms.magic_word_type);
                    result.push(':');
                    result.push_str(&trans.param);

                    // Add forms
                    for form in &forms.forms {
                        result.push('|');
                        result.push_str(form);
                    }

                    result.push_str("}}");
                } else if name_upper == "PLURAL" || name_upper == "GENDER" {
                    // Magic word with no extracted forms - copy from source
                    result.push_str("{{");
                    result.push_str(&name_upper);
                    result.push(':');
                    result.push_str(&trans.param);
                    for opt in &trans.options {
                        result.push('|');
                        result.push_str(opt);
                    }
                    result.push_str("}}");
                } else {
                    // Other transclusions - copy as-is
                    result.push_str("{{");
                    result.push_str(&trans.name);
                    result.push(':');
                    result.push_str(&trans.param);
                    for opt in &trans.options {
                        result.push('|');
                        result.push_str(opt);
                    }
                    result.push_str("}}");
                }
            }
            AstNode::InternalLink(link) => {
                result.push_str("[[");
                result.push_str(&link.target);
                if let Some(t) = &link.display_text {
                    result.push('|');
                    result.push_str(t);
                }
                result.push_str("]]");
            }
            AstNode::ExternalLink(link) => {
                result.push('[');
                result.push_str(&link.url);
                if let Some(t) = &link.text {
                    result.push(' ');
                    result.push_str(t);
                }
                result.push(']');
            }
        }
    }

    Ok(result)
}

/// Calculate confidence score for the reassembly
fn calculate_confidence(
    _alignment: &Alignment,
    scope_changes: &[ScopeChange],
    _variants: &[String],
) -> f32 {
    let mut confidence = 1.0_f32;

    // Deduct for each scope change
    confidence -= scope_changes.len() as f32 * 0.1;

    // Ensure confidence stays in [0, 1] range
    confidence.max(0.0).min(1.0)
}

/// Generate user-facing warnings
fn generate_warnings(scope_changes: &[ScopeChange]) -> Vec<String> {
    let mut warnings = Vec::new();

    for change in scope_changes {
        warnings.push(format!(
            "Magic word at position {} expanded scope due to translation agreement: original ({}, {}), expanded ({}, {})",
            change.ast_index,
            change.original_range.0,
            change.original_range.1,
            change.expanded_range.0,
            change.expanded_range.1
        ));
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_stable_parts_single_variant() {
        let variants = vec!["Hello, world".to_string()];
        let alignment = find_stable_and_variable_parts(&variants).unwrap();

        assert_eq!(alignment.stable_parts.len(), 1);
        assert_eq!(alignment.stable_parts[0].text, "Hello, world");
    }

    #[test]
    fn test_find_stable_parts_identical_variants() {
        let variants = vec![
            "There is one item".to_string(),
            "There is one item".to_string(),
        ];
        let alignment = find_stable_and_variable_parts(&variants).unwrap();

        assert_eq!(alignment.stable_parts.len(), 1);
        assert_eq!(alignment.stable_parts[0].text, "There is one item");
    }

    #[test]
    fn test_find_change_ranges_simple() {
        let ranges = find_change_ranges("hello", "hallo");
        assert_eq!(ranges, vec![(1, 2)]);
    }

    #[test]
    fn test_find_change_ranges_multiple() {
        let ranges = find_change_ranges("There is item", "There are items");
        assert!(!ranges.is_empty());
    }

    #[test]
    fn test_confidence_no_scope_changes() {
        let scope_changes = vec![];
        let confidence = calculate_confidence(
            &Alignment {
                stable_parts: vec![],
                variable_parts: vec![],
            },
            &scope_changes,
            &["test".to_string()],
        );
        assert_eq!(confidence, 1.0);
    }

    #[test]
    fn test_confidence_with_scope_changes() {
        let scope_changes = vec![
            ScopeChange {
                ast_index: 0,
                original_range: (0, 5),
                expanded_range: (0, 10),
                reason: "test".to_string(),
            },
            ScopeChange {
                ast_index: 1,
                original_range: (10, 15),
                expanded_range: (10, 20),
                reason: "test".to_string(),
            },
        ];
        let confidence = calculate_confidence(
            &Alignment {
                stable_parts: vec![],
                variable_parts: vec![],
            },
            &scope_changes,
            &["test".to_string()],
        );
        assert_eq!(confidence, 0.8);
    }

    #[test]
    fn test_generate_warnings() {
        let scope_changes = vec![ScopeChange {
            ast_index: 0,
            original_range: (0, 5),
            expanded_range: (0, 10),
            reason: "test expansion".to_string(),
        }];
        let warnings = generate_warnings(&scope_changes);

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("expanded scope"));
    }

    #[test]
    fn test_empty_variants_error() {
        let result = find_stable_and_variable_parts(&[]);
        assert!(result.is_err());
    }
}
