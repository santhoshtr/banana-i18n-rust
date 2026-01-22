use super::anchor;
use super::error::MtResult;
use crate::ast::{AstNode, AstNodeList, Transclusion};

#[cfg(test)]
use crate::ast::Placeholder;

/// Representative test values for gender selection.
/// Standard across all languages: male, female, unknown
#[derive(Debug, Clone, PartialEq)]
pub struct GenderForm {
    pub label: String,      // "male", "female", "unknown"
    pub test_value: String, // For expansion, same as label
}

/// Get all gender forms for expansion.
///
/// Gender expansion is language-independent. All languages use the same 3 forms:
/// - male (masculine)
/// - female (feminine)
/// - unknown (neutral/other)
///
/// # Returns
/// Vec of GenderForm with the 3 standard gender test values
///
/// # Example
/// ```ignore
/// let forms = get_gender_forms();
/// assert_eq!(forms.len(), 3);
/// assert_eq!(forms[0].label, "male");
/// assert_eq!(forms[1].label, "female");
/// assert_eq!(forms[2].label, "unknown");
/// ```
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

/// Find the maximum placeholder index used in the AST
///
/// This scans for both direct placeholders ($1, $2) and GENDER params
/// to determine how many anchor tokens need to be generated.
fn find_max_placeholder_index(ast: &AstNodeList) -> usize {
    let mut max_idx = 0;
    for node in ast {
        match node {
            AstNode::Placeholder(p) => {
                max_idx = max_idx.max(p.index);
            }
            AstNode::Transclusion(trans) => {
                // Check GENDER params like $2 in {{GENDER:$2|...}}
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

/// Expand a message with GENDER magic words into all its plain-text variants.
///
/// This function implements gender variant expansion similar to PLURAL expansion,
/// but with fixed gender categories (male, female, unknown).
///
/// ## Algorithm Workflow
///
/// 1. **Parse AST** - Input wikitext is already parsed to AST
/// 2. **Expand** - Generate all plain-text variants by substituting test values for each gender
/// 3. **Protect** - Replace all placeholders ($1, $2) with anchor tokens (_ID1_, _ID2_) to prevent
///    machine translation from corrupting placeholder values
/// 4. **Ready for MT** - Output variants are ready to send to machine translation system
///
/// ## Example
///
/// ```ignore
/// // Input wikitext with GENDER and placeholders
/// "{{GENDER:$1|He|She|They}} sent $2"
///
/// // Processing
/// 1. Find GENDER nodes: 1 node found
/// 2. Get gender forms: [male, female, unknown]
/// 3. Generate variants by substituting test values:
///    - Male: "He sent $2"
///    - Female: "She sent $2"
///    - Unknown: "They sent $2"
/// 4. Apply anchor tokens to protect placeholders:
///    - Male: "He sent _ID2_"
///    - Female: "She sent _ID2_"
///    - Unknown: "They sent _ID2_"
///
/// // Output ready for MT
/// ["He sent _ID2_", "She sent _ID2_", "They sent _ID2_"]
/// ```
///
/// ## Anchor Tokens
///
/// This function automatically applies anchor tokens to all placeholders to prevent
/// machine translation systems from corrupting placeholder values. For example:
/// - "male" should NOT be translated to "hombre" (Spanish) or "garçon" (French)
/// - Instead, anchor tokens preserve the test value without translation
///
/// After machine translation and reassembly, anchor tokens are replaced back to placeholders.
///
/// # Arguments
/// * `ast` - Abstract syntax tree of the message
///
/// # Returns
/// Vec of expanded plain-text variants (one per gender form combination), with placeholders
/// protected by anchor tokens. Ready to send to machine translation system.
///
/// # Errors
/// Returns an error if:
/// - Anchor token generation fails
/// - Variant rendering fails
///
/// # See Also
/// - `Algorithm.md` - Full algorithm description
/// - `crate::mt::anchor` - Anchor token system
/// - Iteration 4 - Cartesian product (for combining GENDER + PLURAL)
pub fn expand_gender_variants(ast: &AstNodeList) -> MtResult<Vec<String>> {
    // Get all gender forms (always 3: male, female, unknown)
    let gender_forms = get_gender_forms();

    // Find all GENDER nodes in the AST with their indices
    let gender_positions = find_gender_nodes(ast);

    if gender_positions.is_empty() {
        // No GENDER nodes - return single variant with text rendering
        let text = render_ast_to_text(ast)?;
        // Still apply anchor tokens even without GENDERs (for consistency)
        let variants = apply_anchor_tokens_to_variants(vec![text], ast)?;
        return Ok(variants);
    }

    // Generate all combinations of gender forms
    // For N GENDER nodes with varying form counts, we need a cartesian product
    let mut variants = Vec::new();
    generate_gender_combinations(
        &gender_positions,
        &gender_forms,
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

/// Internal representation of a GENDER node position in the AST
#[derive(Debug, Clone)]
struct GenderNodeInfo {
    #[allow(dead_code)]
    ast_index: usize,
    transclusion: Transclusion,
}

/// Find all GENDER nodes in the AST and their indices
fn find_gender_nodes(ast: &AstNodeList) -> Vec<GenderNodeInfo> {
    let mut genders = Vec::new();

    for (idx, node) in ast.iter().enumerate() {
        if let AstNode::Transclusion(trans) = node {
            if trans.name.to_uppercase() == "GENDER" {
                genders.push(GenderNodeInfo {
                    ast_index: idx,
                    transclusion: trans.clone(),
                });
            }
        }
    }

    genders
}

/// Recursively generate all combinations of gender forms
fn generate_gender_combinations(
    gender_positions: &[GenderNodeInfo],
    available_forms: &[GenderForm],
    ast: &AstNodeList,
    current_gender_idx: usize,
    mut current_values: Vec<(String, usize)>, // (test_value, form_index_for_this_gender)
    variants: &mut Vec<String>,
) -> MtResult<()> {
    if current_gender_idx >= gender_positions.len() {
        // We have selected forms for all GENDER nodes - render this variant
        let text = render_ast_with_gender_values(ast, &current_values)?;
        variants.push(text);
        return Ok(());
    }

    // Get the current GENDER node
    let current_gender = &gender_positions[current_gender_idx];

    // Determine how many forms this GENDER node actually has
    let gender_form_count = current_gender.transclusion.options.len();

    // For each possible form in this GENDER, recurse
    for (form_idx, form) in available_forms.iter().enumerate() {
        // Only use this form if the GENDER has enough options
        if form_idx < gender_form_count && gender_form_count > 0 {
            current_values.push((form.test_value.clone(), form_idx));

            generate_gender_combinations(
                gender_positions,
                available_forms,
                ast,
                current_gender_idx + 1,
                current_values.clone(),
                variants,
            )?;

            current_values.pop();
        }
    }

    // If this GENDER has fewer forms than available, pad with last form
    if gender_form_count > 0 && gender_form_count < available_forms.len() {
        for _ in gender_form_count..available_forms.len() {
            current_values.push((
                available_forms[gender_form_count - 1].test_value.clone(),
                gender_form_count - 1,
            ));

            generate_gender_combinations(
                gender_positions,
                available_forms,
                ast,
                current_gender_idx + 1,
                current_values.clone(),
                variants,
            )?;

            current_values.pop();
        }
    } else if gender_form_count == 0 {
        // Empty GENDER - just continue with empty form selection
        generate_gender_combinations(
            gender_positions,
            available_forms,
            ast,
            current_gender_idx + 1,
            current_values.clone(),
            variants,
        )?;
    }

    Ok(())
}

/// Render AST to plain text without any special gender handling
fn render_ast_to_text(ast: &AstNodeList) -> MtResult<String> {
    let mut result = String::new();

    for node in ast {
        match node {
            AstNode::Text(text) => result.push_str(text),
            AstNode::Placeholder(p) => {
                // Render placeholder as $N pattern (will be converted to anchors later)
                result.push('$');
                result.push_str(&p.index.to_string());
            }
            AstNode::Transclusion(trans) => {
                if trans.name.to_uppercase() == "GENDER" {
                    // For GENDER without explicit values, render empty
                    // (This shouldn't happen in normal flow)
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

/// Render AST with specific gender form selections
fn render_ast_with_gender_values(
    ast: &AstNodeList,
    gender_values: &[(String, usize)], // (test_value, form_index) for each GENDER in order
) -> MtResult<String> {
    let mut result = String::new();
    let mut gender_counter = 0;

    for node in ast {
        match node {
            AstNode::Text(text) => result.push_str(text),
            AstNode::Placeholder(p) => {
                // Render placeholder as $N pattern (will be converted to anchors later)
                result.push('$');
                result.push_str(&p.index.to_string());
            }
            AstNode::Transclusion(trans) => {
                if trans.name.to_uppercase() == "GENDER" {
                    // Select the appropriate form for this GENDER
                    if gender_counter < gender_values.len() {
                        let (_, form_idx) = gender_values[gender_counter];
                        if form_idx < trans.options.len() {
                            result.push_str(&trans.options[form_idx]);
                        } else if !trans.options.is_empty() {
                            result.push_str(trans.options.last().unwrap());
                        }
                        gender_counter += 1;
                    }
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
    fn test_get_gender_forms() {
        let forms = get_gender_forms();

        // Should always return 3 forms
        assert_eq!(forms.len(), 3);

        // Check order and values
        assert_eq!(forms[0].label, "male");
        assert_eq!(forms[0].test_value, "male");

        assert_eq!(forms[1].label, "female");
        assert_eq!(forms[1].test_value, "female");

        assert_eq!(forms[2].label, "unknown");
        assert_eq!(forms[2].test_value, "unknown");
    }

    #[test]
    fn test_expand_gender_variants_three_forms() {
        // {{GENDER:$1|He|She|They}} sent message
        let ast = vec![
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$1".to_string(),
                options: vec!["He".to_string(), "She".to_string(), "They".to_string()],
            }),
            AstNode::Text(" sent message".to_string()),
        ];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // Should have 3 variants (one per gender)
        assert_eq!(variants.len(), 3);

        // All should start with the gender form
        assert!(variants[0].starts_with("He"));
        assert!(variants[1].starts_with("She"));
        assert!(variants[2].starts_with("They"));

        // All should contain the static text
        assert!(variants[0].contains("sent message"));
        assert!(variants[1].contains("sent message"));
        assert!(variants[2].contains("sent message"));
    }

    #[test]
    fn test_expand_gender_variants_two_forms() {
        // {{GENDER:$1|he|she}} - should be padded to 3 variants
        let ast = vec![AstNode::Transclusion(Transclusion {
            name: "GENDER".to_string(),
            param: "$1".to_string(),
            options: vec!["he".to_string(), "she".to_string()],
        })];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // Should have 3 variants (padded with last form for unknown)
        assert_eq!(variants.len(), 3);

        assert_eq!(variants[0], "he");
        assert_eq!(variants[1], "she");
        assert_eq!(variants[2], "she"); // Padded with last form
    }

    #[test]
    fn test_expand_gender_variants_single_form() {
        // {{GENDER:$1|person}} - should be padded to 3 variants
        let ast = vec![AstNode::Transclusion(Transclusion {
            name: "GENDER".to_string(),
            param: "$1".to_string(),
            options: vec!["person".to_string()],
        })];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // Should have 3 variants (all using the same form)
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], "person");
        assert_eq!(variants[1], "person");
        assert_eq!(variants[2], "person");
    }

    #[test]
    fn test_expand_gender_variants_zero_forms() {
        // {{GENDER:$1}} - empty options
        let ast = vec![
            AstNode::Text("Items: ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$1".to_string(),
                options: vec![],
            }),
        ];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // Empty GENDER should generate a single variant
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0], "Items: ");
    }

    #[test]
    fn test_expand_gender_variants_no_gender_nodes() {
        // Message with no GENDER nodes
        let ast = vec![
            AstNode::Text("Hello ".to_string()),
            AstNode::Placeholder(Placeholder { index: 1 }),
        ];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // No GENDER nodes, so just one variant
        assert_eq!(variants.len(), 1);
        // Placeholder should be converted to anchor token
        assert_eq!(variants[0], "Hello _ID1_");
    }

    #[test]
    fn test_expand_gender_variants_direct_parameter() {
        // {{GENDER:male|he|she}} - direct parameter, not a placeholder
        // This is static, so it should expand to all 3 forms (as if testing all genders)
        // Actually, the current implementation expands to all available forms because
        // it doesn't distinguish between direct parameters and placeholders during expansion
        // The distinction happens during rendering via the param field
        let ast = vec![
            AstNode::Text("User is ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "male".to_string(),
                options: vec!["he".to_string(), "she".to_string()],
            }),
        ];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // With 2 forms, we generate 2 variants (then pad to 3 total)
        // But the first GENDER node has only 2 options
        // So we generate 2 variants for the 2 forms (form 0 and form 1)
        // Plus padding generates 1 more for the 3rd gender form
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], "User is he");
        assert_eq!(variants[1], "User is she");
        assert_eq!(variants[2], "User is she"); // Padded
    }

    #[test]
    fn test_expand_gender_variants_with_placeholder() {
        // {{GENDER:$1|he|she|they}} and $2 placeholder
        let ast = vec![
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$1".to_string(),
                options: vec!["he".to_string(), "she".to_string(), "they".to_string()],
            }),
            AstNode::Text(" sent ".to_string()),
            AstNode::Placeholder(Placeholder { index: 2 }),
        ];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // Should have 3 variants (one per gender)
        assert_eq!(variants.len(), 3);

        // All should have anchor token for $2
        for variant in &variants {
            assert!(
                variant.contains("_ID2_"),
                "Should have anchor token for $2: {}",
                variant
            );
            assert!(
                !variant.contains("$2"),
                "Should not have raw placeholder: {}",
                variant
            );
        }

        // Verify the gender forms
        assert!(variants[0].starts_with("he"));
        assert!(variants[1].starts_with("she"));
        assert!(variants[2].starts_with("they"));
    }

    #[test]
    fn test_expand_gender_variants_multiple_genders() {
        // {{GENDER:$1|he|she}} and {{GENDER:$2|him|her}}
        // Each has 2 forms, padded to 3
        // So we generate 3 * 3 = 9 variants
        let ast = vec![
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$1".to_string(),
                options: vec!["he".to_string(), "she".to_string()],
            }),
            AstNode::Text(" sent to ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$2".to_string(),
                options: vec!["him".to_string(), "her".to_string()],
            }),
        ];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // 2 genders with 2 forms each, padded to 3 forms each = 3 * 3 = 9 variants
        assert_eq!(variants.len(), 9);

        // First few combinations:
        // (form 0, form 0): he, him
        assert_eq!(variants[0], "he sent to him");
        // (form 0, form 1): he, her
        assert_eq!(variants[1], "he sent to her");
        // (form 0, form 1-padded): he, her (padded)
        assert_eq!(variants[2], "he sent to her");
        // (form 1, form 0): she, him
        assert_eq!(variants[3], "she sent to him");
    }

    #[test]
    fn test_expand_gender_variants_with_links() {
        // {{GENDER:$1|He|She}} said "[[article|this]]"
        let ast = vec![
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$1".to_string(),
                options: vec!["He".to_string(), "She".to_string()],
            }),
            AstNode::Text(" said \"".to_string()),
            AstNode::InternalLink(crate::ast::WikiInternalLink {
                target: "article".to_string(),
                display_text: Some("this".to_string()),
            }),
            AstNode::Text("\"".to_string()),
        ];

        let variants = expand_gender_variants(&ast).expect("should expand");

        // With 2 forms, padded to 3 forms = 3 variants
        assert_eq!(variants.len(), 3);

        // Check all variants render correctly
        assert!(variants[0].contains("He said"));
        assert!(variants[0].contains("this"));

        assert!(variants[1].contains("She said"));
        assert!(variants[1].contains("this"));

        // Third variant is padded with last form (She)
        assert!(variants[2].contains("She said"));
        assert!(variants[2].contains("this"));
    }

    #[test]
    fn test_expand_gender_variants_roundtrip() {
        // Test full workflow: expand → anchor → recover
        use crate::mt::anchor::{generate_anchor_tokens, recover_placeholders_from_anchors};

        let ast = vec![
            AstNode::Placeholder(Placeholder { index: 1 }),
            AstNode::Text(" is ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$1".to_string(),
                options: vec![
                    "male".to_string(),
                    "female".to_string(),
                    "other".to_string(),
                ],
            }),
        ];

        // Step 1: Expand with test values (now includes anchors)
        let expanded = expand_gender_variants(&ast).expect("expansion should succeed");

        // Should have 3 variants (one per gender)
        assert_eq!(expanded.len(), 3);

        // Step 2: Verify anchor tokens are present
        for variant in &expanded {
            assert!(
                variant.contains("_ID1_"),
                "Should have anchor token for $1: {}",
                variant
            );
            // Should NOT have raw placeholder pattern
            assert!(
                !variant.contains("$1"),
                "Should not have raw $1 (should be _ID1_): {}",
                variant
            );
        }

        // Step 3: Verify roundtrip - recover placeholders back from anchors
        let anchors = generate_anchor_tokens(1); // $1
        for variant in &expanded {
            let recovered =
                recover_placeholders_from_anchors(variant, &anchors).expect("recovery should work");
            // Recovered should have placeholders back
            assert!(
                recovered.contains("$1"),
                "Should recover $1 from anchors: {}",
                recovered
            );
            // Should have the gender form text
            assert!(
                recovered.contains(" is "),
                "Should contain static text: {}",
                recovered
            );
        }
    }

    #[test]
    fn test_find_gender_nodes() {
        let ast = vec![
            AstNode::Text("Start ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$1".to_string(),
                options: vec!["he".to_string(), "she".to_string()],
            }),
            AstNode::Text(" middle ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "PLURAL".to_string(),
                param: "$2".to_string(),
                options: vec!["one".to_string(), "many".to_string()],
            }),
            AstNode::Text(" end ".to_string()),
            AstNode::Transclusion(Transclusion {
                name: "GENDER".to_string(),
                param: "$3".to_string(),
                options: vec!["a".to_string(), "b".to_string()],
            }),
        ];

        let genders = find_gender_nodes(&ast);

        // Should find exactly 2 GENDER nodes (indices 1 and 5)
        assert_eq!(genders.len(), 2);
        assert_eq!(genders[0].ast_index, 1);
        assert_eq!(genders[1].ast_index, 5);
    }
}
