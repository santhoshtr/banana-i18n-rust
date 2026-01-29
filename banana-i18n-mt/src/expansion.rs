//! Unified Expansion Engine for Cartesian Product (PLURAL × GENDER)
//!
//! This module generates all combinations of PLURAL and GENDER variants from a message AST,
//! closely following the Python reference implementation's approach. It combines the
//! functionality previously split across multiple modules.
//!
//! # Algorithm Overview
//!
//! The expansion follows the Python reference pattern:
//! 1. **Collect Choices** - Find all PLURAL/GENDER magic words and their option counts
//! 2. **Cartesian Product** - Generate all state combinations using itertools-style product
//! 3. **Resolve Variants** - Convert each state to a plain text variant with anchor tokens
//! 4. **Anchor Protection** - Replace $1, $2 with 777001, 777002 to protect from MT corruption
//!
//! # Example
//!
//! ```ignore
//! // Input: "{{GENDER:$1|He|She}} sent {{PLURAL:$2|a message|$2 messages}}"
//! // Output: 6 variants (3 GENDER × 2 PLURAL)
//! // [
//! //   "777001 sent a message",      // He, singular  
//! //   "777001 sent 777002 messages", // He, plural
//! //   "777001 sent a message",      // She, singular
//! //   "777001 sent 777002 messages", // She, plural
//! //   "777001 sent a message",      // They, singular
//! //   "777001 sent 777002 messages"  // They, plural
//! // ]
//! ```

use super::data::{MessageContext, TranslationVariant};
use super::error::{MtError, MtResult};
use banana_i18n::ast::{AstNode, AstNodeList};
use std::collections::HashMap;

// ICU dependencies for plural rules (kept from original implementation)
use icu_locale::Locale;
use icu_plurals::{PluralCategory, PluralRuleType, PluralRules};

/// Maximum number of variants allowed to prevent combinatorial explosion
const MAX_VARIANTS: usize = 64;

/// Information about a magic word found in the AST
#[derive(Debug, Clone)]
struct ChoiceInfo {
    /// Variable ID (e.g., "$1", "$2")
    var_id: String,
    /// Magic word type ("PLURAL" or "GENDER") - stored for consistency but not used in current logic
    #[allow(dead_code)]
    magic_type: String,
    /// Number of options available
    option_count: usize,
}

/// Representative test values for each plural category in a language
#[derive(Debug, Clone, PartialEq)]
pub struct PluralForm {
    pub category: PluralCategory,
    pub test_value: u32,
}

/// Representative test values for gender selection (language-independent)
#[derive(Debug, Clone, PartialEq)]
pub struct GenderForm {
    pub label: String,      // "male", "female", "unknown"
    pub test_value: String, // For expansion, same as label
}

/// Main expansion function: converts AST to all variant combinations
///
/// This function matches the Python `expand_to_variants()` design, creating
/// a cartesian product of all magic word choices and resolving each combination
/// to a plain text string with anchor tokens.
///
/// # Arguments
/// * `ast` - The parsed AST of the message containing magic words
/// * `locale` - The target locale for plural form selection (e.g., "en", "ru")
///
/// # Returns
/// * `Ok(Vec<TranslationVariant>)` - All variants with anchor tokens
/// * `Err(MtError)` - If variant count exceeds MAX_VARIANTS or expansion fails
///
/// # Example
/// ```ignore
/// let variants = expand_to_variants(&ast, "en")?;
/// assert_eq!(variants.len(), 6); // 2 PLURAL × 3 GENDER
/// ```
pub fn expand_to_variants(ast: &AstNodeList, locale: &str) -> MtResult<Vec<TranslationVariant>> {
    // 1. Collect all magic words (PLURAL/GENDER) and their option counts
    let choices = collect_choices(ast, locale)?;

    // Check for empty case
    if choices.is_empty() {
        // No magic words - create single variant with anchor tokens applied
        let text = resolve_ast_with_anchors(ast, &HashMap::new())?;
        return Ok(vec![TranslationVariant::new(HashMap::new(), text)]);
    }

    // 2. Calculate total variant count and check limit
    let variant_count = calculate_total_variants(&choices)?;
    if variant_count > MAX_VARIANTS {
        return Err(MtError::ExpansionError(format!(
            "Too many variants ({} > {}): message with {} magic words produces too many combinations",
            variant_count,
            MAX_VARIANTS,
            choices.len()
        )));
    }

    // 3. Generate all combinations (cartesian product)
    let state_combinations = generate_state_combinations(&choices)?;

    // 4. Resolve each state to a variant with anchor tokens
    let mut variants = Vec::new();
    for state in state_combinations {
        let source_text = resolve_ast_with_anchors(ast, &state)?;
        variants.push(TranslationVariant::new(state, source_text));
    }

    Ok(variants)
}

/// Prepare message for translation by creating a complete MessageContext
///
/// This function matches the Python `prepare_for_translation()` design, creating
/// a MessageContext with variable type information and all expanded variants.
///
/// # Arguments
/// * `ast` - The parsed AST of the message
/// * `locale` - The target locale for plural form selection
/// * `message_key` - Original message key for reference
///
/// # Returns
/// * `Ok(MessageContext)` - Context with all variants and metadata
/// * `Err(MtError)` - If expansion fails
pub fn prepare_for_translation(
    ast: &AstNodeList,
    locale: &str,
    message_key: &str,
) -> MtResult<MessageContext> {
    let mut context = MessageContext::new(message_key.to_string());

    // Analyze AST to extract variable types
    analyze_ast_for_variables(ast, &mut context)?;

    // Generate all variants
    let variants = expand_to_variants(ast, locale)?;
    for variant in variants {
        context.add_variant(variant);
    }

    Ok(context)
}

/// Collect all magic words in AST and determine their option counts
fn collect_choices(ast: &AstNodeList, locale: &str) -> MtResult<Vec<ChoiceInfo>> {
    let mut choices = Vec::new();

    for node in ast.iter() {
        if let AstNode::Transclusion(trans) = node {
            let name_upper = trans.name.to_uppercase();

            if name_upper == "PLURAL" {
                // Get plural forms for this locale using ICU
                let plural_forms = get_plural_forms_for_language(locale)?;
                choices.push(ChoiceInfo {
                    var_id: trans.param.clone(),
                    magic_type: "PLURAL".to_string(),
                    option_count: plural_forms.len(),
                });
            } else if name_upper == "GENDER" {
                // Gender always has 3 forms: male, female, unknown
                choices.push(ChoiceInfo {
                    var_id: trans.param.clone(),
                    magic_type: "GENDER".to_string(),
                    option_count: 3, // Always 3 gender forms
                });
            }
        }
    }

    Ok(choices)
}

/// Calculate total number of variants (product of all option counts)
fn calculate_total_variants(choices: &[ChoiceInfo]) -> MtResult<usize> {
    if choices.is_empty() {
        return Ok(1);
    }

    let mut total = 1usize;
    for choice in choices {
        total = total
            .checked_mul(choice.option_count)
            .ok_or_else(|| MtError::ExpansionError("Variant count overflow".to_string()))?;
    }

    Ok(total)
}

/// Generate all state combinations using cartesian product
fn generate_state_combinations(choices: &[ChoiceInfo]) -> MtResult<Vec<HashMap<String, usize>>> {
    if choices.is_empty() {
        return Ok(vec![HashMap::new()]);
    }

    // Build ranges for each choice
    let ranges: Vec<Vec<usize>> = choices
        .iter()
        .map(|choice| (0..choice.option_count).collect())
        .collect();

    // Generate cartesian product
    let mut combinations = Vec::new();
    cartesian_product_recursive(&ranges, 0, &mut Vec::new(), &mut combinations);

    // Convert index combinations to state maps
    let mut states = Vec::new();
    for combination in combinations {
        let mut state = HashMap::new();
        for (choice_idx, &option_idx) in combination.iter().enumerate() {
            state.insert(choices[choice_idx].var_id.clone(), option_idx);
        }
        states.push(state);
    }

    Ok(states)
}

/// Recursive helper for cartesian product generation
fn cartesian_product_recursive(
    ranges: &[Vec<usize>],
    depth: usize,
    current: &mut Vec<usize>,
    results: &mut Vec<Vec<usize>>,
) {
    if depth == ranges.len() {
        results.push(current.clone());
        return;
    }

    for &value in &ranges[depth] {
        current.push(value);
        cartesian_product_recursive(ranges, depth + 1, current, results);
        current.pop();
    }
}

/// Resolve AST with specific state to plain text with anchor tokens
fn resolve_ast_with_anchors(ast: &AstNodeList, state: &HashMap<String, usize>) -> MtResult<String> {
    let mut result = String::new();

    for node in ast {
        match node {
            AstNode::Text(text) => {
                result.push_str(text);
            }
            AstNode::Placeholder(placeholder) => {
                // Replace $1, $2, etc. with anchor tokens 777001, 777002 (777000 + index)
                result.push_str(&format!("{}", 777000 + placeholder.index));
            }
            AstNode::Transclusion(trans) => {
                let name_upper = trans.name.to_uppercase();

                if name_upper == "PLURAL" || name_upper == "GENDER" {
                    // Get the selected option index from state
                    let option_idx = state.get(&trans.param).copied().unwrap_or(0);

                    // Use the selected option (or last option if index out of bounds)
                    let actual_idx = option_idx.min(trans.options.len().saturating_sub(1));

                    if let Some(option) = trans.options.get(actual_idx) {
                        // Replace placeholders in the option with anchor tokens
                        let option_with_anchors = replace_placeholders_with_anchors(option)?;
                        result.push_str(&option_with_anchors);
                    }
                } else {
                    // Non-magic transclusion, render as-is
                    result.push_str(&trans.name);
                }
            }
            AstNode::InternalLink(link) => {
                result.push_str("[[");
                result.push_str(&link.target);
                if let Some(ref display_text) = link.display_text {
                    result.push('|');
                    result.push_str(display_text);
                }
                result.push_str("]]");
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

/// Replace placeholders with anchor tokens in a text string
fn replace_placeholders_with_anchors(text: &str) -> MtResult<String> {
    use regex::Regex;

    // Replace $1, $2, etc. with 777001, 777002, etc. (777000 + index)
    // Sort by index in descending order to handle $10 before $1 (avoid conflicts)
    let re = Regex::new(r"\$(\d+)").unwrap();

    // Collect all matches first
    let mut matches: Vec<(usize, usize, usize)> = Vec::new(); // (start, end, placeholder_number)
    for cap in re.captures_iter(text) {
        let full_match = cap.get(0).unwrap();
        let placeholder_num: usize = cap[1].parse().unwrap();
        matches.push((full_match.start(), full_match.end(), placeholder_num));
    }

    // Sort by start position in descending order to replace from right to left
    matches.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = text.to_string();
    for (start, end, num) in matches {
        let anchor = format!("{}", 777000 + num);
        result.replace_range(start..end, &anchor);
    }

    Ok(result)
}

/// Analyze AST to extract variable type information
fn analyze_ast_for_variables(ast: &AstNodeList, context: &mut MessageContext) -> MtResult<()> {
    for node in ast.iter() {
        if let AstNode::Transclusion(trans) = node {
            let name_upper = trans.name.to_uppercase();
            if name_upper == "PLURAL" || name_upper == "GENDER" {
                context.add_variable(trans.param.clone(), name_upper);
            }
        }
    }
    Ok(())
}

/// Get all plural forms for a given language with representative test values
///
/// This function uses ICU plural rules to determine how many plural forms
/// a language has, and provides representative numbers that will select each form.
/// Preserved from the original plural_expansion.rs implementation.
///
/// # Arguments  
/// * `locale_str` - Language code (e.g., "en", "ru", "ar", "de")
///
/// # Returns
/// Vec of PluralForm with category and test value for each form
pub fn get_plural_forms_for_language(locale_str: &str) -> MtResult<Vec<PluralForm>> {
    // Parse the locale
    let locale: Locale = locale_str.parse().map_err(|e| {
        MtError::PluralExpansionError(format!("Failed to parse locale '{}': {}", locale_str, e))
    })?;

    // Create plural rules for the locale (cardinal numbers)
    let pr = PluralRules::try_new(locale.into(), PluralRuleType::Cardinal.into()).map_err(|e| {
        MtError::PluralExpansionError(format!(
            "Failed to create PluralRules for locale '{}': {}",
            locale_str, e
        ))
    })?;

    // Map plural categories to specific test values
    // These test values are chosen to trigger each plural form in various languages
    let test_values_by_category = [
        (PluralCategory::Zero, vec![0u32]),
        (PluralCategory::One, vec![1u32, 21u32, 31u32, 41u32]),
        (PluralCategory::Two, vec![2u32, 22u32, 32u32]),
        (PluralCategory::Few, vec![3u32, 4u32, 23u32, 24u32]),
        (PluralCategory::Many, vec![5u32, 11u32, 101u32]),
        (
            PluralCategory::Other,
            vec![6u32, 7u32, 8u32, 9u32, 10u32, 25u32, 100u32, 1000u32],
        ),
    ];

    // Collect the categories that are actually used in this language
    let mut forms = Vec::new();

    for (expected_category, test_values) in test_values_by_category.iter() {
        for &test_value in test_values {
            let actual_category = pr.category_for(test_value as usize);
            if actual_category == *expected_category {
                forms.push(PluralForm {
                    category: *expected_category,
                    test_value,
                });
                break; // Found a good test value for this category, move to next
            }
        }
    }

    Ok(forms)
}

/// Get all gender forms for expansion
///
/// Gender expansion is language-independent. All languages use the same 3 forms:
/// - male (masculine)
/// - female (feminine)
/// - unknown (neutral/other)
pub fn get_gender_forms() -> Vec<GenderForm> {
    vec![
        GenderForm {
            label: "male".to_string(),
            test_value: "male".to_string(),
        },
        GenderForm {
            label: "female".to_string(),
            test_value: "female".to_string(),
        },
        GenderForm {
            label: "unknown".to_string(),
            test_value: "unknown".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use banana_i18n::parser::Parser;

    fn parse(text: &str) -> AstNodeList {
        let mut parser = Parser::new(text);
        parser.parse()
    }

    // ========== Baseline Tests ==========

    #[test]
    fn test_expand_no_magic_words() {
        let ast = parse("Hello, $1!");
        let variants = expand_to_variants(&ast, "en").unwrap();
        assert_eq!(variants.len(), 1);
        assert!(variants[0].source_text.contains("777001"));
    }

    #[test]
    fn test_expand_plain_text() {
        let ast = parse("Plain text message");
        let variants = expand_to_variants(&ast, "en").unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].source_text, "Plain text message");
    }

    #[test]
    fn test_prepare_for_translation() {
        let ast = parse("Hello, $1!");
        let context = prepare_for_translation(&ast, "en", "test-message").unwrap();

        assert_eq!(context.original_key, "test-message");
        assert_eq!(context.variant_count(), 1);
        assert!(context.variants[0].source_text.contains("777001"));
    }

    // ========== Single Magic Word Tests ==========

    #[test]
    fn test_expand_plural_only_english() {
        let ast = parse("There {{PLURAL:$1|is|are}} $1 item");
        let variants = expand_to_variants(&ast, "en").unwrap();
        // English has 2 plural forms
        assert_eq!(variants.len(), 2);
    }

    #[test]
    fn test_expand_gender_only() {
        let ast = parse("{{GENDER:$1|He|She|They}} is here");
        let variants = expand_to_variants(&ast, "en").unwrap();
        // Gender always has 3 forms
        assert_eq!(variants.len(), 3);
    }

    // ========== Cartesian Product Tests ==========

    #[test]
    fn test_expand_plural_and_gender_english() {
        let ast = parse("{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}");
        let variants = expand_to_variants(&ast, "en").unwrap();
        // English: 3 GENDER × 2 PLURAL = 6 variants
        assert_eq!(variants.len(), 6);

        // Check that variants have anchor tokens where expected
        // Note: $1 is only used as GENDER control parameter, so no 777001 expected
        // $2 appears in the plural form "$2 messages", so 777002 should appear

        // Variants with singular form should not have 777002
        assert!(variants[0].source_text == "He sent a message");
        assert!(variants[2].source_text == "She sent a message");
        assert!(variants[4].source_text == "They sent a message");

        // Variants with plural form should have 777002 from "$2 messages"
        assert!(variants[1].source_text.contains("777002"));
        assert!(variants[3].source_text.contains("777002"));
        assert!(variants[5].source_text.contains("777002"));
    }

    #[test]
    fn test_expand_plural_and_gender_russian() {
        let ast = parse("{{GENDER:$1|Он|Она|Они}} {{PLURAL:$2|письмо|письма|писем}}");
        let variants = expand_to_variants(&ast, "ru").unwrap();
        // Russian: 3 GENDER × 3 PLURAL = 9 variants
        assert_eq!(variants.len(), 9);
    }

    // ========== Variant Count Calculation Tests ==========

    #[test]
    fn test_cartesian_product_generation() {
        let choices = vec![
            ChoiceInfo {
                var_id: "$1".to_string(),
                magic_type: "GENDER".to_string(),
                option_count: 3,
            },
            ChoiceInfo {
                var_id: "$2".to_string(),
                magic_type: "PLURAL".to_string(),
                option_count: 2,
            },
        ];

        let states = generate_state_combinations(&choices).unwrap();
        assert_eq!(states.len(), 6); // 3 × 2

        // Check that each state has both variables
        for state in &states {
            assert!(state.contains_key("$1"));
            assert!(state.contains_key("$2"));
            assert!(*state.get("$1").unwrap() < 3);
            assert!(*state.get("$2").unwrap() < 2);
        }
    }

    // ========== Limit Enforcement Tests ==========

    #[test]
    fn test_limit_at_max_variants() {
        // Create a message with exactly 64 variants
        // 2^6 = 64 (6 binary choices)
        let message = "{{PLURAL:$1|a|b}} {{PLURAL:$2|a|b}} {{PLURAL:$3|a|b}} {{PLURAL:$4|a|b}} {{PLURAL:$5|a|b}} {{PLURAL:$6|a|b}}";
        let ast = parse(message);
        let variants = expand_to_variants(&ast, "en").unwrap();
        assert_eq!(variants.len(), 64);
    }

    #[test]
    fn test_limit_exceeds_max_variants() {
        // Create a message with 128 variants (exceeds MAX_VARIANTS of 64)
        let message = "{{PLURAL:$1|a|b}} {{PLURAL:$2|a|b}} {{PLURAL:$3|a|b}} {{PLURAL:$4|a|b}} {{PLURAL:$5|a|b}} {{PLURAL:$6|a|b}} {{PLURAL:$7|a|b}}";
        let ast = parse(message);
        let result = expand_to_variants(&ast, "en");
        assert!(result.is_err());

        match result {
            Err(MtError::ExpansionError(msg)) => {
                assert!(msg.contains("Too many variants"));
            }
            _ => panic!("Expected ExpansionError"),
        }
    }

    // ========== Anchor Token Tests ==========

    #[test]
    fn test_placeholder_replacement_with_anchors() {
        let text = "$1 sent $2 to $3";
        let result = replace_placeholders_with_anchors(text).unwrap();
        assert_eq!(result, "777001 sent 777002 to 777003");
    }

    #[test]
    fn test_placeholder_replacement_order() {
        // Test that $10 is replaced before $1 to avoid conflicts
        let text = "$1 and $10 are different";
        let result = replace_placeholders_with_anchors(text).unwrap();
        assert!(result.contains("777001"));
        assert!(result.contains("777010"));
        assert!(!result.contains("7770010"));
    }

    #[test]
    fn test_no_placeholder_replacement() {
        let text = "Hello, World!";
        let result = replace_placeholders_with_anchors(text).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    // ========== ICU Plural Form Tests ==========

    #[test]
    fn test_get_plural_forms_english() {
        let forms = get_plural_forms_for_language("en").unwrap();

        // English typically has 2 forms: one and other
        assert!(forms.len() >= 2);
        assert!(forms.iter().any(|f| f.test_value == 1)); // one

        // Check that we have an "other" category (test value varies by implementation)
        assert!(
            forms
                .iter()
                .any(|f| f.category == icu_plurals::PluralCategory::Other)
        );

        // Specifically, we should have categories One and Other
        let categories: std::collections::HashSet<_> = forms.iter().map(|f| f.category).collect();
        assert!(categories.contains(&icu_plurals::PluralCategory::One));
        assert!(categories.contains(&icu_plurals::PluralCategory::Other));
    }

    #[test]
    fn test_get_plural_forms_russian() {
        let forms = get_plural_forms_for_language("ru").unwrap();
        // Russian has 3 forms: one, few, many
        assert!(forms.len() >= 3);
        assert!(forms.iter().any(|f| f.test_value == 1)); // one
        assert!(forms.iter().any(|f| f.test_value == 2 || f.test_value == 3)); // few
        assert!(forms.iter().any(|f| f.test_value == 5)); // many
    }

    #[test]
    fn test_get_plural_forms_invalid_locale() {
        let result = get_plural_forms_for_language("invalid-locale");
        assert!(result.is_err());
    }

    // ========== Gender Form Tests ==========

    #[test]
    fn test_get_gender_forms() {
        let forms = get_gender_forms();
        assert_eq!(forms.len(), 3);
        assert_eq!(forms[0].label, "male");
        assert_eq!(forms[1].label, "female");
        assert_eq!(forms[2].label, "unknown");
    }

    // ========== Complex Integration Tests ==========

    #[test]
    fn test_complex_message_with_links() {
        let ast = parse(
            "{{GENDER:$1|He|She|They}} sent [[article]] and {{PLURAL:$2|a message|$2 messages}}",
        );
        let variants = expand_to_variants(&ast, "en").unwrap();
        assert_eq!(variants.len(), 6); // 3 GENDER × 2 PLURAL

        for variant in &variants {
            assert!(variant.source_text.contains("article"));
            // Note: $1 is used as GENDER control, no 777001 expected
            // Only 777002 appears in plural forms
        }

        // Check that plural variants have 777002
        assert!(variants[1].source_text.contains("777002"));
        assert!(variants[3].source_text.contains("777002"));
        assert!(variants[5].source_text.contains("777002"));
    }

    #[test]
    fn test_analyze_ast_for_variables() {
        let ast = parse("{{GENDER:$1|He|She}} sent {{PLURAL:$2|one|many}}");
        let mut context = MessageContext::new("test".to_string());
        analyze_ast_for_variables(&ast, &mut context).unwrap();

        assert_eq!(context.get_variable_type("$1"), Some(&"GENDER".to_string()));
        assert_eq!(context.get_variable_type("$2"), Some(&"PLURAL".to_string()));
        assert_eq!(context.get_variable_type("$3"), None);
    }

    #[test]
    fn test_empty_choices_collection() {
        let ast = parse("Plain message with $1");
        let choices = collect_choices(&ast, "en").unwrap();
        assert!(choices.is_empty());
    }
}
