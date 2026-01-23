//! Unified Expansion Engine for Cartesian Product (PLURAL × GENDER)
//!
//! This module generates all combinations of PLURAL and GENDER variants from a message AST.
//! It handles complex messages containing multiple magic words and enforces a limit on the
//! number of variants to prevent combinatorial explosion.
//!
//! # Placeholder Design
//!
//! Placeholders ($1, $2, etc.) in wikitext messages play two distinct roles:
//!
//! 1. **Control Placeholders** (magic word parameters):
//!    - Found inside magic word parameters: `{{PLURAL:$1|...}}`, `{{GENDER:$2|...}}`
//!    - Used to select which form to use during expansion
//!    - **Consumed during expansion** - do NOT appear in expanded variants
//!    - Do NOT receive anchor token protection (they never reach MT system)
//!    - **Example:** `{{GENDER:$1|He|She}}` → variants `"He"`, `"She"` (no $1)
//!
//! 2. **Output Placeholders** (form text):
//!    - Found inside magic word form text: `{{PLURAL:$1|$1 item|$1 items}}`
//!    - These are actual content that appears in the final text
//!    - MUST receive anchor token protection to prevent MT corruption
//!    - **Example:** `{{PLURAL:$1|$1 item|$1 items}}` → variants `"$1 item"`, `"$1 items"` (has $1)
//!
//! # Example
//!
//! ```ignore
//! use banana_i18n::mt::expansion::expand_all_variants;
//! use banana_i18n::parser::Parser;
//!
//! let mut parser = Parser::new("");
//! let ast = parser.parse();
//! let variants = expand_all_variants(&ast, "en")?;
//! // Result: Vec with variants
//! ```

use super::anchor;
use super::error::{MtError, MtResult};
use super::gender_expansion;
use super::plural_expansion;
use crate::ast::{AstNode, AstNodeList, Transclusion};
use std::collections::HashMap;

/// Maximum number of variants allowed. Prevents combinatorial explosion.
const MAX_VARIANTS: usize = 64;

/// Represents a magic word (PLURAL or GENDER) in the AST with its forms.
#[derive(Debug, Clone)]
enum MagicWordType {
    /// PLURAL magic word with its position, transclusion, and available forms
    Plural {
        ast_index: usize,
        #[allow(dead_code)]
        transclusion: Transclusion,
        forms: Vec<plural_expansion::PluralForm>,
    },
    /// GENDER magic word with its position, transclusion, and available forms
    Gender {
        ast_index: usize,
        #[allow(dead_code)]
        transclusion: Transclusion,
        forms: Vec<gender_expansion::GenderForm>,
    },
}

/// Represents a specific selection of forms for all magic words in a message.
#[derive(Debug, Clone)]
struct FormSelection {
    /// Selected form index for each magic word
    selections: Vec<FormChoice>,
}

/// A choice of form for a specific magic word.
#[derive(Debug, Clone)]
enum FormChoice {
    /// Selected plural form with test value and form index
    PluralForm {
        #[allow(dead_code)]
        test_value: u32,
        form_index: usize,
    },
    /// Selected gender form with test value and form index
    GenderForm {
        #[allow(dead_code)]
        test_value: String,
        form_index: usize,
    },
}

/// Expands all combinations of PLURAL and GENDER variants from a message AST.
///
/// # Arguments
///
/// * `ast` - The parsed AST of the message containing magic words
/// * `locale` - The target locale for plural form selection (e.g., "en", "ru")
///
/// # Returns
///
/// * `Ok(Vec<String>)` - All variants with anchor tokens protecting placeholders
/// * `Err(MtError)` - If variant count exceeds MAX_VARIANTS or other expansion errors
///
/// # Example
///
/// ```ignore
/// let variants = expand_all_variants(&ast, "en")?;
/// assert_eq!(variants.len(), 6); // 2 PLURAL × 3 GENDER
/// ```
pub fn expand_all_variants(ast: &AstNodeList, locale: &str) -> MtResult<Vec<String>> {
    // Find all magic words (PLURAL and GENDER) in the AST
    let magic_words = find_all_magic_words(ast, locale)?;

    // Calculate total variant count and check limit
    let variant_count = calculate_variant_count(ast, locale)?;
    if variant_count > MAX_VARIANTS {
        return Err(MtError::ExpansionError(format!(
            "Too many variants ({} > {}): message with {} magic words produces too many combinations",
            variant_count,
            MAX_VARIANTS,
            magic_words.len()
        )));
    }

    // Generate all form selections (cartesian product)
    let selections = generate_all_combinations(&magic_words)?;

    // Render each selection into a variant string
    let mut variants = Vec::new();
    for selection in selections {
        let variant = render_variant(ast, &selection, &magic_words)?;
        variants.push(variant);
    }

    // Apply anchor tokens to protect placeholders from MT corruption
    let variants = apply_anchor_tokens_to_variants(variants, ast)?;

    Ok(variants)
}

/// Calculates the number of variants that would be generated without actually generating them.
///
/// # Arguments
///
/// * `ast` - The parsed AST of the message
/// * `locale` - The target locale for plural form selection
///
/// # Returns
///
/// * `Ok(usize)` - The predicted number of variants (product of all form counts)
/// * `Err(MtError)` - If unable to determine form counts
///
/// # Example
///
/// ```ignore
/// let count = calculate_variant_count(&ast, "en")?;
/// // PLURAL (2 forms) × GENDER (3 forms) = 6
/// assert_eq!(count, 6);
/// ```
pub fn calculate_variant_count(ast: &AstNodeList, locale: &str) -> MtResult<usize> {
    let magic_words = find_all_magic_words(ast, locale)?;

    if magic_words.is_empty() {
        return Ok(1); // No magic words = single variant
    }

    let mut total_count = 1usize;
    for magic_word in &magic_words {
        let form_count = match magic_word {
            MagicWordType::Plural { forms, .. } => forms.len(),
            MagicWordType::Gender { forms, .. } => forms.len(),
        };
        total_count = total_count
            .checked_mul(form_count)
            .ok_or_else(|| MtError::ExpansionError("Variant count overflow".to_string()))?;
    }

    Ok(total_count)
}

/// Finds all PLURAL and GENDER magic words in the AST.
///
/// # Arguments
///
/// * `ast` - The parsed AST to scan
/// * `locale` - The target locale for plural form selection
///
/// # Returns
///
/// * `Ok(Vec<MagicWordType>)` - List of all magic words found, in AST order
/// * `Err(MtError)` - If unable to retrieve forms for a magic word
fn find_all_magic_words(ast: &AstNodeList, locale: &str) -> MtResult<Vec<MagicWordType>> {
    let mut magic_words = Vec::new();

    for (idx, node) in ast.iter().enumerate() {
        if let AstNode::Transclusion(trans) = node {
            let name_upper = trans.name.to_uppercase();

            if name_upper == "PLURAL" {
                // Get plural forms for this locale
                let forms = plural_expansion::get_plural_forms_for_language(locale)?;
                magic_words.push(MagicWordType::Plural {
                    ast_index: idx,
                    transclusion: trans.clone(),
                    forms,
                });
            } else if name_upper == "GENDER" {
                // Get gender forms (always 3)
                let forms = gender_expansion::get_gender_forms();
                magic_words.push(MagicWordType::Gender {
                    ast_index: idx,
                    transclusion: trans.clone(),
                    forms,
                });
            }
        }
    }

    Ok(magic_words)
}

/// Generates all combinations of form selections (cartesian product).
///
/// # Arguments
///
/// * `magic_words` - List of magic words with their available forms
///
/// # Returns
///
/// * `Ok(Vec<FormSelection>)` - All possible form selections
/// * `Err(MtError)` - If generation fails
fn generate_all_combinations(magic_words: &[MagicWordType]) -> MtResult<Vec<FormSelection>> {
    if magic_words.is_empty() {
        return Ok(vec![FormSelection {
            selections: Vec::new(),
        }]);
    }

    let mut combinations: Vec<FormSelection> = Vec::new();
    let mut current_selections: Vec<FormChoice> = Vec::new();

    generate_combinations_recursive(magic_words, 0, &mut current_selections, &mut combinations)?;

    Ok(combinations)
}

/// Recursive helper for cartesian product generation.
fn generate_combinations_recursive(
    magic_words: &[MagicWordType],
    current_idx: usize,
    current_selections: &mut Vec<FormChoice>,
    combinations: &mut Vec<FormSelection>,
) -> MtResult<()> {
    // Base case: all magic words have been processed
    if current_idx >= magic_words.len() {
        combinations.push(FormSelection {
            selections: current_selections.clone(),
        });
        return Ok(());
    }

    // Get the current magic word and iterate through its forms
    let magic_word = &magic_words[current_idx];
    match magic_word {
        MagicWordType::Plural { forms, .. } => {
            for (form_idx, form) in forms.iter().enumerate() {
                current_selections.push(FormChoice::PluralForm {
                    test_value: form.test_value,
                    form_index: form_idx,
                });
                generate_combinations_recursive(
                    magic_words,
                    current_idx + 1,
                    current_selections,
                    combinations,
                )?;
                current_selections.pop();
            }
        }
        MagicWordType::Gender { forms, .. } => {
            for (form_idx, form) in forms.iter().enumerate() {
                current_selections.push(FormChoice::GenderForm {
                    test_value: form.test_value.clone(),
                    form_index: form_idx,
                });
                generate_combinations_recursive(
                    magic_words,
                    current_idx + 1,
                    current_selections,
                    combinations,
                )?;
                current_selections.pop();
            }
        }
    }

    Ok(())
}

/// Renders a single variant with all magic words substituted with specific forms.
///
/// # Arguments
///
/// * `ast` - The original AST
/// * `selection` - The form selection for this variant
/// * `magic_words` - The list of magic words with their metadata
///
/// # Returns
///
/// * `Ok(String)` - The rendered variant text
/// * `Err(MtError)` - If rendering fails
fn render_variant(
    ast: &AstNodeList,
    selection: &FormSelection,
    magic_words: &[MagicWordType],
) -> MtResult<String> {
    // Build a map of AST indices to their selected forms
    let mut magic_word_to_choice: HashMap<usize, &FormChoice> = HashMap::new();
    for (magic_idx, choice) in selection.selections.iter().enumerate() {
        if magic_idx < magic_words.len() {
            let ast_index = match &magic_words[magic_idx] {
                MagicWordType::Plural { ast_index, .. } => *ast_index,
                MagicWordType::Gender { ast_index, .. } => *ast_index,
            };
            magic_word_to_choice.insert(ast_index, choice);
        }
    }

    // Render the AST with substitutions
    let mut result = String::new();

    for (idx, node) in ast.iter().enumerate() {
        match node {
            AstNode::Transclusion(trans) => {
                let name_upper = trans.name.to_uppercase();

                if name_upper == "PLURAL" || name_upper == "GENDER" {
                    // Check if this magic word is in our selection map
                    if let Some(choice) = magic_word_to_choice.get(&idx) {
                        match choice {
                            FormChoice::PluralForm { form_index, .. } => {
                                // Extract the forms from options
                                // options[0] is the param, options[1..] are the forms
                                if *form_index < trans.options.len() {
                                    result.push_str(&trans.options[*form_index]);
                                } else if !trans.options.is_empty() {
                                    result.push_str(trans.options.last().unwrap());
                                }
                            }
                            FormChoice::GenderForm { form_index, .. } => {
                                // Extract the forms from options
                                // options[0] is the param, options[1..] are the forms
                                if *form_index < trans.options.len() {
                                    result.push_str(&trans.options[*form_index]);
                                } else if !trans.options.is_empty() {
                                    result.push_str(trans.options.last().unwrap());
                                }
                            }
                        }
                    } else {
                        // Not in our selection map, render as-is
                        result.push_str(&trans.name);
                    }
                } else {
                    // Non-magic transclusion, render as-is
                    result.push_str(&trans.name);
                }
            }
            AstNode::Text(text) => {
                result.push_str(text);
            }
            AstNode::Placeholder(placeholder) => {
                result.push('$');
                result.push_str(&placeholder.index.to_string());
            }
            AstNode::InternalLink(link) => {
                result.push('[');
                result.push('[');
                result.push_str(&link.target);
                if let Some(ref display) = link.display_text {
                    result.push('|');
                    result.push_str(display);
                }
                result.push(']');
                result.push(']');
            }
            AstNode::ExternalLink(link) => {
                result.push('[');
                result.push_str(&link.url);
                if let Some(ref text) = link.text {
                    result.push(' ');
                    result.push_str(text);
                }
                result.push(']');
            }
        }
    }

    Ok(result)
}

/// Applies anchor tokens to variants to protect placeholders from MT corruption.
///
/// # Arguments
///
/// * `variants` - The rendered variants
/// * `ast` - The original AST (to get placeholder count)
///
/// # Returns
///
/// * `Ok(Vec<String>)` - Variants with anchor tokens applied
/// * `Err(MtError)` - If anchor token application fails
fn apply_anchor_tokens_to_variants(
    variants: Vec<String>,
    ast: &AstNodeList,
) -> MtResult<Vec<String>> {
    // Count placeholders in the original AST
    let placeholder_count = count_placeholders(ast);

    if placeholder_count == 0 {
        return Ok(variants); // No placeholders, no anchors needed
    }

    // Generate anchor tokens
    let anchors = anchor::generate_anchor_tokens(placeholder_count);

    // Apply anchors to each variant
    let mut anchored_variants = Vec::new();
    for variant in variants {
        let anchored = anchor::replace_placeholders_with_anchors(&variant, &anchors)?;
        anchored_variants.push(anchored);
    }

    Ok(anchored_variants)
}

/// Counts the number of placeholders in the AST.
fn count_placeholders(ast: &AstNodeList) -> usize {
    let mut max_placeholder = 0;
    for node in ast.iter() {
        if let AstNode::Placeholder(p) = node {
            max_placeholder = max_placeholder.max(p.index);
        }
    }
    max_placeholder
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse(text: &str) -> MtResult<AstNodeList> {
        let mut parser = Parser::new(text);
        let ast = parser.parse();
        Ok(ast)
    }

    // ========== Baseline Tests ==========

    #[test]
    fn test_expand_no_magic_words() -> MtResult<()> {
        let ast = parse("Hello, $1!")?;
        let variants = expand_all_variants(&ast, "en")?;
        assert_eq!(variants.len(), 1);
        Ok(())
    }

    #[test]
    fn test_expand_plain_text() -> MtResult<()> {
        let ast = parse("Plain text message")?;
        let variants = expand_all_variants(&ast, "en")?;
        assert_eq!(variants.len(), 1);
        Ok(())
    }

    #[test]
    fn test_calculate_no_magic_words() -> MtResult<()> {
        let ast = parse("Hello, $1!")?;
        let count = calculate_variant_count(&ast, "en")?;
        assert_eq!(count, 1);
        Ok(())
    }

    // ========== Single Magic Word Delegation Tests ==========

    #[test]
    fn test_expand_plural_only_english() -> MtResult<()> {
        let ast = parse("There {{PLURAL:$1|is|are}} $1 item")?;
        let variants = expand_all_variants(&ast, "en")?;
        // English has 2 plural forms
        assert_eq!(variants.len(), 2);
        Ok(())
    }

    #[test]
    fn test_expand_gender_only() -> MtResult<()> {
        let ast = parse("{{GENDER:$1|He|She|They}} is here")?;
        let variants = expand_all_variants(&ast, "en")?;
        // Gender always has 3 forms
        assert_eq!(variants.len(), 3);
        Ok(())
    }

    // ========== Cartesian Product Core Tests ==========

    #[test]
    fn test_expand_plural_and_gender_english() -> MtResult<()> {
        let ast = parse("{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}")?;
        let variants = expand_all_variants(&ast, "en")?;
        // English: 3 GENDER × 2 PLURAL = 6 variants
        assert_eq!(variants.len(), 6);
        Ok(())
    }

    #[test]
    fn test_expand_plural_and_gender_russian() -> MtResult<()> {
        let ast = parse("{{GENDER:$1|Он|Она|Они}} {{PLURAL:$2|письмо|письма|писем}}")?;
        let variants = expand_all_variants(&ast, "ru")?;
        // Russian: 3 GENDER × 3 PLURAL = 9 variants
        assert_eq!(variants.len(), 9);
        Ok(())
    }

    #[test]
    fn test_calculate_variant_count_plural_gender() -> MtResult<()> {
        let ast = parse("{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}")?;
        let count = calculate_variant_count(&ast, "en")?;
        assert_eq!(count, 6);
        Ok(())
    }

    #[test]
    fn test_calculate_variant_count_russian_multiple() -> MtResult<()> {
        let ast = parse("{{GENDER:$1|Он|Она|Они}} {{PLURAL:$2|письмо|письма|писем}}")?;
        let count = calculate_variant_count(&ast, "ru")?;
        assert_eq!(count, 9);
        Ok(())
    }

    // ========== Variant Count Calculation Tests ==========

    #[test]
    fn test_variant_count_matches_expansion_english() -> MtResult<()> {
        let ast = parse("{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}")?;
        let predicted_count = calculate_variant_count(&ast, "en")?;
        let variants = expand_all_variants(&ast, "en")?;
        assert_eq!(predicted_count, variants.len());
        Ok(())
    }

    #[test]
    fn test_variant_count_matches_expansion_russian() -> MtResult<()> {
        let ast = parse("{{GENDER:$1|Он|Она|Они}} {{PLURAL:$2|письмо|письма|писем}}")?;
        let predicted_count = calculate_variant_count(&ast, "ru")?;
        let variants = expand_all_variants(&ast, "ru")?;
        assert_eq!(predicted_count, variants.len());
        Ok(())
    }

    // ========== Limit Enforcement Tests ==========

    #[test]
    fn test_limit_at_max_variants() -> MtResult<()> {
        // Create a message with exactly 64 variants
        // 2 PLURAL × 2 PLURAL × 2 PLURAL × 2 PLURAL × 2 PLURAL × 2 PLURAL = 64
        let ast = parse(
            "{{PLURAL:$1|a|b}} {{PLURAL:$2|a|b}} {{PLURAL:$3|a|b}} \
             {{PLURAL:$4|a|b}} {{PLURAL:$5|a|b}} {{PLURAL:$6|a|b}}",
        )?;
        let variants = expand_all_variants(&ast, "en")?;
        assert_eq!(variants.len(), 64);
        Ok(())
    }

    #[test]
    fn test_limit_exceeds_max_variants() -> MtResult<()> {
        // Create a message with 128 variants (exceeds MAX_VARIANTS of 64)
        // 2 PLURAL × 2 PLURAL × 2 PLURAL × 2 PLURAL × 2 PLURAL × 2 PLURAL × 2 PLURAL = 128
        let ast = parse(
            "{{PLURAL:$1|a|b}} {{PLURAL:$2|a|b}} {{PLURAL:$3|a|b}} \
             {{PLURAL:$4|a|b}} {{PLURAL:$5|a|b}} {{PLURAL:$6|a|b}} {{PLURAL:$7|a|b}}",
        )?;
        let result = expand_all_variants(&ast, "en");
        assert!(result.is_err());
        match result {
            Err(MtError::ExpansionError(msg)) => {
                assert!(msg.contains("Too many variants"));
            }
            _ => panic!("Expected ExpansionError with too many variants"),
        }
        Ok(())
    }

    // ========== Anchor Token Integration Tests ==========

    #[test]
    fn test_expand_with_anchor_tokens() -> MtResult<()> {
        let ast = parse("Hello, $1! You sent {{PLURAL:$2|a message|$2 messages}}")?;
        let variants = expand_all_variants(&ast, "en")?;
        assert_eq!(variants.len(), 2);
        // Check that anchor tokens are present
        for variant in &variants {
            assert!(variant.contains("_ID"));
        }
        Ok(())
    }

    #[test]
    fn test_expand_complex_with_links() -> MtResult<()> {
        let ast = parse(
            "{{GENDER:$1|He|She|They}} sent [[article]] and \
             {{PLURAL:$2|a message|$2 messages}}",
        )?;
        let variants = expand_all_variants(&ast, "en")?;
        assert_eq!(variants.len(), 6); // 3 GENDER × 2 PLURAL
        for variant in &variants {
            assert!(variant.contains("article"));
        }
        Ok(())
    }
}
