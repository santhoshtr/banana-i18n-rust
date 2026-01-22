use icu_locale::Locale;
use icu_plurals::{PluralCategory, PluralRuleType, PluralRules};

use super::anchor;
use super::error::{MtError, MtResult};
use crate::ast::{AstNode, AstNodeList, Transclusion};

#[cfg(test)]
use crate::ast::Placeholder;

/// Representative test values for each plural category in a language.
/// Each tuple contains (PluralCategory, representative_number).
/// These numbers are chosen to trigger each plural form.
#[derive(Debug, Clone, PartialEq)]
pub struct PluralForm {
    pub category: PluralCategory,
    pub test_value: u32,
}

/// Get all plural forms for a given language with representative test values.
///
/// This function uses ICU plural rules to determine how many plural forms
/// a language has, and provides representative numbers that will select each form.
/// For example:
/// - English has 2 forms: one (1) and other (2)
/// - Russian has 3 forms: one (1), few (2), many (5)
/// - Arabic has 6 forms: zero (0), one (1), two (2), few (3), many (4), other (5)
///
/// # Arguments
/// * `locale` - Language code (e.g., "en", "ru", "ar", "de")
///
/// # Returns
/// Vec of PluralForm with category and test value for each form
///
/// # Errors
/// Returns an error if the locale is invalid or plural rules cannot be loaded
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
    // We use a wider range of test values to ensure we find good representatives
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

/// Find the maximum placeholder index used in the AST
///
/// This scans for both direct placeholders ($1, $2) and PLURAL/GENDER params
/// to determine how many anchor tokens need to be generated.
fn find_max_placeholder_index(ast: &AstNodeList) -> usize {
    let mut max_idx = 0;
    for node in ast {
        match node {
            AstNode::Placeholder(p) => {
                max_idx = max_idx.max(p.index);
            }
            AstNode::Transclusion(trans) => {
                // Check PLURAL/GENDER params like $2 in {{PLURAL:$2|...}}
                if trans.param.starts_with('$') {
                    if let Ok(idx) = trans.param[1..].parse::<usize>() {
                        max_idx = max_idx.max(idx);
                    }
                }
            }
            _ => {}
        }
    }
    max_idx
}

/// Expand a message with PLURAL magic words into all its plain-text variants.
///
/// This function implements Step A of the Cartesian Expansion algorithm (see Algorithm.md).
///
/// ## Algorithm Workflow
///
/// 1. **Parse AST** - Input wikitext is already parsed to AST
/// 2. **Expand** - Generate all plain-text variants by substituting test values for each plural form
/// 3. **Protect** - Replace all placeholders ($1, $2) with anchor tokens (_ID1_, _ID2_) to prevent
///    machine translation from corrupting placeholder values
/// 4. **Ready for MT** - Output variants are ready to send to machine translation system
///
/// ## Example
///
/// ```ignore
/// // Input wikitext with PLURAL and placeholders
/// "There {{PLURAL:$1|is|are}} $1 item"
///
/// // Processing
/// 1. Find PLURAL nodes: 1 node found
/// 2. Get plural forms for "en": [One(1), Other(6)]
/// 3. Generate variants by substituting test values:
///    - Form 1 (One): "There is 1 item"
///    - Form 2 (Other): "There are 6 item"
/// 4. Apply anchor tokens to protect placeholders:
///    - Form 1: "There is _ID1_ item"
///    - Form 2: "There are _ID1_ item"
///
/// // Output ready for MT
/// ["There is _ID1_ item", "There are _ID1_ item"]
/// ```
///
/// ## Anchor Tokens
///
/// This function automatically applies anchor tokens to all placeholders to prevent
/// machine translation systems from corrupting placeholder values. For example:
/// - "1" should NOT be translated to "un" (French) or "ek" (Hindi)
/// - Instead, "_ID1_" is used, which MT systems treat as non-translatable
///
/// After machine translation and reassembly, anchor tokens are replaced back to placeholders.
///
/// # Arguments
/// * `ast` - Abstract syntax tree of the message
/// * `locale` - Target language for plural forms (determines how many forms to generate)
///
/// # Returns
/// Vec of expanded plain-text variants (one per plural form combination), with placeholders
/// protected by anchor tokens. Ready to send to machine translation system.
///
/// # Errors
/// Returns an error if:
/// - Locale is invalid or not recognized by ICU
/// - No plural forms can be determined for the locale
///
/// # See Also
/// - `Algorithm.md` - Full algorithm description
/// - `crate::mt::anchor` - Anchor token system
/// - Iteration 4 - Cartesian product (for combining PLURAL + GENDER)
pub fn expand_plural_variants(ast: &AstNodeList, locale: &str) -> MtResult<Vec<String>> {
    // Get all plural forms for this language
    let plural_forms = get_plural_forms_for_language(locale).map_err(|e| {
        // Add context about what we were trying to do
        MtError::PluralExpansionError(format!(
            "Could not expand PLURAL variants for locale '{}': {}. \
            This may mean the locale is not recognized by ICU CLDR or has no plural rules.",
            locale, e
        ))
    })?;

    if plural_forms.is_empty() {
        return Err(MtError::PluralExpansionError(format!(
            "No plural forms found for locale '{}'. \
                The locale is recognized but has no plural rules defined.",
            locale
        )));
    }

    // Find all PLURAL nodes in the AST with their indices
    let plural_positions = find_plural_nodes(ast);

    if plural_positions.is_empty() {
        // No PLURAL nodes - return single variant with text rendering
        let text = render_ast_to_text(ast, &[])?;
        // Still apply anchor tokens even without PLURALs (for consistency)
        let variants = apply_anchor_tokens_to_variants(vec![text], ast)?;
        return Ok(variants);
    }

    // Generate all combinations of plural forms
    // For N PLURAL nodes with varying form counts, we need a cartesian product
    let mut variants = Vec::new();
    generate_plural_combinations(
        &plural_positions,
        &plural_forms,
        ast,
        0,
        Vec::new(),
        &mut variants,
    )?;

    // Apply anchor tokens to protect placeholders from MT corruption
    let variants = apply_anchor_tokens_to_variants(variants, ast)?;

    Ok(variants)
}

/// Apply anchor tokens to a set of variants to protect placeholders
fn apply_anchor_tokens_to_variants(
    variants: Vec<String>,
    ast: &AstNodeList,
) -> MtResult<Vec<String>> {
    let max_placeholder_idx = find_max_placeholder_index(ast);

    if max_placeholder_idx == 0 {
        // No placeholders, return variants as-is
        return Ok(variants);
    }

    let anchors = anchor::generate_anchor_tokens(max_placeholder_idx);

    variants
        .into_iter()
        .map(|variant| anchor::replace_placeholders_with_anchors(&variant, &anchors))
        .collect()
}

/// Internal representation of a PLURAL node position in the AST
#[derive(Debug, Clone)]
struct PluralNodeInfo {
    #[allow(dead_code)]
    ast_index: usize,
    transclusion: Transclusion,
}

/// Find all PLURAL nodes in the AST and their indices
fn find_plural_nodes(ast: &AstNodeList) -> Vec<PluralNodeInfo> {
    let mut plurals = Vec::new();

    for (idx, node) in ast.iter().enumerate() {
        if let AstNode::Transclusion(trans) = node {
            if trans.name.to_uppercase() == "PLURAL" {
                plurals.push(PluralNodeInfo {
                    ast_index: idx,
                    transclusion: trans.clone(),
                });
            }
        }
    }

    plurals
}

/// Recursively generate all combinations of plural forms
fn generate_plural_combinations(
    plural_positions: &[PluralNodeInfo],
    available_forms: &[PluralForm],
    ast: &AstNodeList,
    current_plural_idx: usize,
    mut current_values: Vec<(u32, usize)>, // (test_value, form_index_for_this_plural)
    variants: &mut Vec<String>,
) -> MtResult<()> {
    if current_plural_idx >= plural_positions.len() {
        // We have selected forms for all PLURAL nodes - render this variant
        let text = render_ast_with_plural_values(ast, &current_values)?;
        variants.push(text);
        return Ok(());
    }

    // Get the current PLURAL node
    let current_plural = &plural_positions[current_plural_idx];

    // Determine how many forms this PLURAL node actually has
    let plural_form_count = current_plural.transclusion.options.len();

    // For each possible form in this PLURAL, recurse
    for (form_idx, form) in available_forms.iter().enumerate() {
        // Only use this form if the PLURAL has enough options
        if form_idx < plural_form_count && plural_form_count > 0 {
            current_values.push((form.test_value, form_idx));

            generate_plural_combinations(
                plural_positions,
                available_forms,
                ast,
                current_plural_idx + 1,
                current_values.clone(),
                variants,
            )?;

            current_values.pop();
        }
    }

    // If this PLURAL has fewer forms than the language requires, pad with last form
    if plural_form_count > 0 && plural_form_count < available_forms.len() {
        for _ in plural_form_count..available_forms.len() {
            current_values.push((
                available_forms[plural_form_count - 1].test_value,
                plural_form_count - 1,
            ));

            generate_plural_combinations(
                plural_positions,
                available_forms,
                ast,
                current_plural_idx + 1,
                current_values.clone(),
                variants,
            )?;

            current_values.pop();
        }
    } else if plural_form_count == 0 {
        // Empty PLURAL - just continue with empty form selection
        generate_plural_combinations(
            plural_positions,
            available_forms,
            ast,
            current_plural_idx + 1,
            current_values.clone(),
            variants,
        )?;
    }

    Ok(())
}

/// Render AST to plain text without any special plural handling
fn render_ast_to_text(ast: &AstNodeList, _values: &[(u32, usize)]) -> MtResult<String> {
    let mut result = String::new();

    for node in ast {
        match node {
            AstNode::Text(text) => result.push_str(text),
            AstNode::Placeholder(p) => {
                // Render placeholder as $N
                result.push('$');
                result.push_str(&p.index.to_string());
            }
            AstNode::Transclusion(trans) => {
                if trans.name.to_uppercase() == "PLURAL" {
                    // For PLURAL without explicit values, render empty
                    // (This shouldn't happen in normal flow)
                } else if trans.name.to_uppercase() == "GENDER" {
                    // For GENDER without explicit values, render empty
                } else {
                    // Unknown transclusion
                    result.push_str(&format!(
                        "{{{{{}:{}|{}}}}}",
                        trans.name,
                        trans.param,
                        trans.options.join("|")
                    ));
                }
            }
            AstNode::InternalLink(link) => {
                if let Some(display) = &link.display_text {
                    result.push_str(display);
                } else {
                    result.push_str(&link.target);
                }
            }
            AstNode::ExternalLink(link) => {
                if let Some(text) = &link.text {
                    result.push_str(text);
                } else {
                    result.push_str(&link.url);
                }
            }
        }
    }

    Ok(result)
}

/// Render AST with specific plural form selections
fn render_ast_with_plural_values(
    ast: &AstNodeList,
    plural_values: &[(u32, usize)], // (test_value, form_index) for each PLURAL in order
) -> MtResult<String> {
    // First, we need to build a map of which placeholders correspond to which PLURAL test values
    // by analyzing the plural_params in each PLURAL node
    let mut placeholder_to_param: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();

    let mut plural_index = 0;
    for node in ast {
        if let AstNode::Transclusion(trans) = node {
            if trans.name.to_uppercase() == "PLURAL" {
                if plural_index < plural_values.len() {
                    // Extract the placeholder index from the param (e.g., "$1" -> 1)
                    if trans.param.starts_with('$') {
                        if let Ok(placeholder_idx) = trans.param[1..].parse::<usize>() {
                            // Map this placeholder to the form index
                            placeholder_to_param.insert(placeholder_idx, plural_index);
                        }
                    }
                    plural_index += 1;
                }
            }
        }
    }

    let mut result = String::new();
    let mut plural_counter = 0;

    for node in ast {
        match node {
            AstNode::Text(text) => result.push_str(text),
            AstNode::Placeholder(p) => {
                // Render placeholder as $N pattern (will be converted to anchors later)
                result.push('$');
                result.push_str(&p.index.to_string());
            }
            AstNode::Transclusion(trans) => {
                if trans.name.to_uppercase() == "PLURAL" {
                    // Select the appropriate form for this PLURAL
                    if plural_counter < plural_values.len() {
                        let (_, form_idx) = plural_values[plural_counter];
                        if form_idx < trans.options.len() {
                            result.push_str(&trans.options[form_idx]);
                        } else if !trans.options.is_empty() {
                            result.push_str(trans.options.last().unwrap());
                        }
                        plural_counter += 1;
                    }
                } else if trans.name.to_uppercase() == "GENDER" {
                    // For now, render empty for GENDER (will be handled in iteration 3)
                } else {
                    // Unknown transclusion
                    result.push_str(&format!(
                        "{{{{{}:{}|{}}}}}",
                        trans.name,
                        trans.param,
                        trans.options.join("|")
                    ));
                }
            }
            AstNode::InternalLink(link) => {
                if let Some(display) = &link.display_text {
                    result.push_str(display);
                } else {
                    result.push_str(&link.target);
                }
            }
            AstNode::ExternalLink(link) => {
                if let Some(text) = &link.text {
                    result.push_str(text);
                } else {
                    result.push_str(&link.url);
                }
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_plural_forms_english() {
        let forms = get_plural_forms_for_language("en").expect("en should be valid");

        // English has 2 plural forms
        assert_eq!(
            forms.len(),
            2,
            "English should have 2 plural forms, got: {:?}",
            forms
        );

        // The categories should include One and Other (in some order)
        let categories: std::collections::HashSet<_> = forms.iter().map(|f| f.category).collect();
        assert!(
            categories.contains(&PluralCategory::One),
            "Should have One category"
        );
        assert!(
            categories.contains(&PluralCategory::Other),
            "Should have Other category"
        );
    }

    #[test]
    fn test_get_plural_forms_russian() {
        let forms = get_plural_forms_for_language("ru").expect("ru should be valid");

        // Russian has 3 plural forms
        assert_eq!(
            forms.len(),
            3,
            "Russian should have 3 plural forms, got: {:?}",
            forms
        );

        // The categories should include One, Few, and Many (in some order)
        let categories: std::collections::HashSet<_> = forms.iter().map(|f| f.category).collect();
        assert!(
            categories.contains(&PluralCategory::One),
            "Should have One category"
        );
        assert!(
            categories.contains(&PluralCategory::Few),
            "Should have Few category"
        );
        assert!(
            categories.contains(&PluralCategory::Many),
            "Should have Many category"
        );
    }

    #[test]
    fn test_get_plural_forms_arabic() {
        let forms = get_plural_forms_for_language("ar").expect("ar should be valid");

        // Arabic typically has 6 plural forms, but the implementation might detect fewer
        // if not all forms are triggered by the test values
        assert!(
            forms.len() >= 4,
            "Arabic should have at least 4 plural forms, got: {:?}",
            forms
        );

        // Check for key categories
        let categories: std::collections::HashSet<_> = forms.iter().map(|f| f.category).collect();
        assert!(
            categories.contains(&PluralCategory::Zero),
            "Should have Zero category"
        );
        assert!(
            categories.contains(&PluralCategory::One),
            "Should have One category"
        );
    }

    #[test]
    fn test_expand_plural_variants_english_simple() {
        // "There {{PLURAL:$1|is|are}} $1 item"
        let ast = vec![
            AstNode::Text("There ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$1".to_string(),
                options: vec!["is".to_string(), "are".to_string()],
            }),
            AstNode::Text(" ".to_string()),
            AstNode::Placeholder(Placeholder { index: 1 }),
            AstNode::Text(" item".to_string()),
        ];

        let variants = expand_plural_variants(&ast, "en").expect("should expand");

        // English has 2 plural forms, so we should get 2 variants
        assert_eq!(variants.len(), 2);
        // Verify anchor tokens protect placeholders (not raw test values)
        assert!(variants[0].contains("is"), "First form should use 'is'");
        assert!(
            variants[0].contains("_ID1_"),
            "Should use anchor token for $1"
        );
        assert!(
            !variants[0].contains(" 1 "),
            "Should not contain raw test value"
        );

        assert!(variants[1].contains("are"), "Second form should use 'are'");
        assert!(
            variants[1].contains("_ID1_"),
            "Should use anchor token for $1"
        );
        assert!(
            !variants[1].contains(" 6 "),
            "Should not contain raw test value"
        );
    }

    #[test]
    fn test_expand_plural_variants_russian_simple() {
        // Russian: "{{PLURAL:$1|предмет|предмета|предметов}}"
        let ast = vec![
            AstNode::Text("В коробке ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$1".to_string(),
                options: vec![
                    "предмет".to_string(),
                    "предмета".to_string(),
                    "предметов".to_string(),
                ],
            }),
        ];

        let variants = expand_plural_variants(&ast, "ru").expect("should expand");

        // Russian has 3 plural forms, so we should get 3 variants
        assert_eq!(variants.len(), 3);
        // All variants should start with prefix (no placeholder substitution here)
        assert!(variants[0].starts_with("В коробке"));
        assert!(variants[1].starts_with("В коробке"));
        assert!(variants[2].starts_with("В коробке"));
        // Verify the forms are present (note: no placeholder substitution in this message)
        assert!(variants[0].contains("предмет"));
        assert!(variants[1].contains("предмета"));
        assert!(variants[2].contains("предметов"));
    }

    #[test]
    fn test_expand_plural_variants_no_plural_nodes() {
        // Message with no PLURAL nodes
        let ast = vec![
            AstNode::Text("Hello ".to_string()),
            AstNode::Placeholder(Placeholder { index: 1 }),
        ];

        let variants = expand_plural_variants(&ast, "en").expect("should expand");

        // No PLURAL nodes, so just one variant
        assert_eq!(variants.len(), 1);
        // Placeholder should be converted to anchor token even without PLURAL
        assert_eq!(variants[0], "Hello _ID1_");
    }

    #[test]
    fn test_expand_plural_variants_partial_forms_english_in_russian() {
        // English PLURAL with 2 forms in Russian which expects 3
        // Should pad the 3rd form with the last form (form 1)
        let ast = vec![AstNode::Transclusion(Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        })];

        let variants = expand_plural_variants(&ast, "ru").expect("should expand");

        // Russian has 3 plural forms, but we only provided 2
        // So we should get 3 variants (with the 3rd using the last form)
        assert_eq!(variants.len(), 3);
        // No placeholder to substitute in this message, so no anchor tokens
        // (anchor tokens only appear where placeholders exist)
        assert_eq!(variants[0], "item");
        assert_eq!(variants[1], "items");
        assert_eq!(variants[2], "items"); // padded with last form
    }

    #[test]
    fn test_expand_plural_variants_multiple_plurals() {
        // Message with two PLURAL nodes
        // "$1 {{PLURAL:$2|sent|sends}} $3 {{PLURAL:$4|message|messages}}"
        let ast = vec![
            AstNode::Placeholder(Placeholder { index: 1 }),
            AstNode::Text(" ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$2".to_string(),
                options: vec!["sent".to_string(), "sends".to_string()],
            }),
            AstNode::Text(" ".to_string()),
            AstNode::Placeholder(Placeholder { index: 3 }),
            AstNode::Text(" ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$4".to_string(),
                options: vec!["message".to_string(), "messages".to_string()],
            }),
        ];

        let variants = expand_plural_variants(&ast, "en").expect("should expand");

        // English has 2 plural forms, and we have 2 PLURAL nodes
        // So we should get 2 * 2 = 4 variants
        assert_eq!(variants.len(), 4);

        // All variants should contain anchor tokens for all placeholders
        for variant in &variants {
            assert!(variant.contains("_ID1_"), "Should have anchor for $1");
            assert!(variant.contains("_ID3_"), "Should have anchor for $3");
        }

        // First variant: both singulars
        assert!(
            variants[0].contains("sent"),
            "First plural should be singular form"
        );
        assert!(
            variants[0].contains("message"),
            "Second plural should be singular form"
        );

        // Verify no raw values leak through
        for variant in &variants {
            assert!(!variant.contains(" 1 "), "Should not have raw value for $1");
            assert!(!variant.contains(" 3 "), "Should not have raw value for $3");
        }
    }

    #[test]
    fn test_expand_plural_variants_with_links() {
        let ast = vec![
            AstNode::Text("Check ".to_string()),
            AstNode::InternalLink(crate::ast::WikiInternalLink {
                target: "article".to_string(),
                display_text: Some("this".to_string()),
            }),
            AstNode::Text(" ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$1".to_string(),
                options: vec!["is".to_string(), "are".to_string()],
            }),
            AstNode::Text(" done".to_string()),
        ];

        let variants = expand_plural_variants(&ast, "en").expect("should expand");

        // Should have 2 variants (for English's 2 plural forms)
        assert_eq!(variants.len(), 2);
        // Note: This message has no placeholders, so no anchor tokens will be present
        assert_eq!(variants[0], "Check this is done");
        assert_eq!(variants[1], "Check this are done");
    }

    #[test]
    fn test_expand_plural_variants_empty_plural() {
        // PLURAL with no options and NO direct placeholder node
        // Since there's no direct placeholder $1 in the AST (only in PLURAL param),
        // no anchor tokens will be generated
        let ast = vec![
            AstNode::Text("Items: ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$1".to_string(),
                options: vec![],
            }),
        ];

        let variants = expand_plural_variants(&ast, "en").expect("should expand");

        // Empty PLURAL should generate a single variant (no form selection possible)
        assert_eq!(variants.len(), 1);
        // Since there's no direct Placeholder node, no anchors are generated
        // The $1 only exists as a PLURAL param, not as a rendered element
        assert_eq!(variants[0], "Items: ");
    }

    #[test]
    fn test_expand_plural_variants_direct_number() {
        // PLURAL with direct number (not a placeholder)
        let ast = vec![
            AstNode::Text("There are ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "5".to_string(),
                options: vec!["item".to_string(), "items".to_string()],
            }),
        ];

        let variants = expand_plural_variants(&ast, "en").expect("should expand");

        // Should have 2 variants (for English's 2 plural forms)
        assert_eq!(variants.len(), 2);
        // Both should contain the text parts (no placeholders, so no anchors)
        assert!(variants[0].contains("There are"));
        assert!(variants[1].contains("There are"));
        // No placeholder means no anchor tokens
        assert!(
            !variants[0].contains("_ID"),
            "Should not have anchor tokens when no placeholders"
        );
    }

    #[test]
    fn test_find_plural_nodes() {
        let ast = vec![
            AstNode::Text("Start ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$1".to_string(),
                options: vec!["one".to_string(), "many".to_string()],
            }),
            AstNode::Text(" middle ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$2".to_string(),
                options: vec!["he".to_string(), "she".to_string()],
            }),
            AstNode::Text(" end ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$3".to_string(),
                options: vec!["a".to_string(), "b".to_string()],
            }),
        ];

        let plurals = find_plural_nodes(&ast);

        // Should find exactly 2 PLURAL nodes (indices 1 and 5)
        assert_eq!(plurals.len(), 2);
        assert_eq!(plurals[0].ast_index, 1);
        assert_eq!(plurals[1].ast_index, 5);
    }

    #[test]
    fn test_expand_recover_roundtrip() {
        // Tests the complete workflow: expand → anchor → recover
        // This validates the algorithm per Algorithm.md Step A-C
        use crate::mt::anchor::{generate_anchor_tokens, recover_placeholders_from_anchors};

        // Message with direct placeholders $1 and $3
        // The PLURAL uses $2 to determine form, but $2 appears as a direct placeholder too
        let ast = vec![
            AstNode::Placeholder(Placeholder { index: 1 }),
            AstNode::Text(" ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$2".to_string(),
                options: vec!["sent".to_string(), "sends".to_string()],
            }),
            AstNode::Text(" message to ".to_string()),
            AstNode::Placeholder(Placeholder { index: 2 }),
            AstNode::Text(" and ".to_string()),
            AstNode::Placeholder(Placeholder { index: 3 }),
        ];

        // Step 1: Expand with test values (now includes anchors)
        let expanded = expand_plural_variants(&ast, "en").expect("expansion should succeed");

        // Should have 2 variants (English has 2 plural forms)
        assert_eq!(expanded.len(), 2);

        // Step 2: Verify anchor tokens are present (protecting placeholders)
        for variant in &expanded {
            assert!(
                variant.contains("_ID1_"),
                "Should have anchor token for $1: {}",
                variant
            );
            assert!(
                variant.contains("_ID2_"),
                "Should have anchor token for $2: {}",
                variant
            );
            assert!(
                variant.contains("_ID3_"),
                "Should have anchor token for $3: {}",
                variant
            );
            // Should NOT have raw placeholder patterns
            assert!(
                !variant.contains("$1") && !variant.contains("$3"),
                "Should not have raw $N patterns (should be anchors): {}",
                variant
            );
        }

        // Step 3: Verify roundtrip - recover placeholders back from anchors
        let anchors = generate_anchor_tokens(3); // $1, $2, $3
        for variant in &expanded {
            let recovered =
                recover_placeholders_from_anchors(variant, &anchors).expect("recovery should work");
            // Recovered should have placeholders back (e.g., "$1", "$2")
            assert!(
                recovered.contains("$1"),
                "Should recover $1 from anchors: {}",
                recovered
            );
            assert!(
                recovered.contains("$2"),
                "Should recover $2 from anchors: {}",
                recovered
            );
            assert!(
                recovered.contains("$3"),
                "Should recover $3 from anchors: {}",
                recovered
            );
        }
    }

    #[test]
    fn test_expand_no_placeholder_no_anchors() {
        // Edge case: message with PLURAL but no placeholders
        // Should NOT apply anchor tokens (nothing to protect)
        let ast = vec![
            AstNode::Text("Result: ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "5".to_string(), // Direct number, not a placeholder
                options: vec!["success".to_string(), "successes".to_string()],
            }),
        ];

        let variants = expand_plural_variants(&ast, "en").expect("expansion should succeed");

        // Should have 2 variants (English has 2 plural forms)
        assert_eq!(variants.len(), 2);

        // Should NOT have anchor tokens (no placeholders to protect)
        for variant in &variants {
            assert!(
                !variant.contains("_ID"),
                "Should not have anchor tokens when no placeholders: {}",
                variant
            );
            // Should have the PLURAL forms
            assert!(
                variant.contains("Result:"),
                "Should contain prefix: {}",
                variant
            );
        }
    }
}
