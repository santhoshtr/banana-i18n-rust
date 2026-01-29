//! End-to-End Integration Tests for Machine Translation Pipeline
//!
//! These tests exercise the complete pipeline using the new simplified API design,
//! following the Python reference implementation patterns.
//!
//! # Running Integration Tests
//!
//! ```bash
//! export GOOGLE_TRANSLATE_API_KEY=$(cat .env | grep GOOGLE_TRANSLATE_API_KEY | cut -d= -f2)
//! cargo test --lib mt::integration_tests -- --ignored --nocapture
//! ```

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::parser::Parser;
    use std::time::Instant;

    /// Helper to format timing output
    fn format_duration(duration: std::time::Duration) -> String {
        if duration.as_secs() > 0 {
            format!("{:.2}s", duration.as_secs_f64())
        } else if duration.as_millis() > 0 {
            format!("{}ms", duration.as_millis())
        } else {
            format!("{}µs", duration.as_micros())
        }
    }

    /// Skip test if API key not available
    fn require_api_key() -> bool {
        std::env::var("GOOGLE_TRANSLATE_API_KEY").is_ok()
    }

    // ============================================================================
    // TEST 1: Simple message with a single placeholder (New API)
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_simple_message_new_api() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 1: Simple Message with New API");
        println!("{}", "=".repeat(80));
        println!("Purpose: Validate new simplified API workflow");

        let overall_start = Instant::now();

        // Setup
        let source_message = "Hello, $1!";
        let source_locale = "en";
        let target_locale = "fr";

        println!("\n📝 SOURCE MESSAGE:");
        println!("  Locale: {}", source_locale);
        println!("  Message: \"{}\"", source_message);

        // 1. Parse message
        let parse_start = Instant::now();
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let parse_duration = parse_start.elapsed();
        println!("⏱️  Parse: {}", format_duration(parse_duration));

        // 2. Prepare for translation (NEW API)
        let prepare_start = Instant::now();
        let mut context = prepare_for_translation(&ast, source_locale, "test-message")
            .expect("Failed to prepare for translation");
        let prepare_duration = prepare_start.elapsed();

        println!("\n📦 MESSAGE CONTEXT:");
        println!("  Key: {}", context.original_key);
        println!("  Variables: {:?}", context.variable_types);
        println!("  Variants: {}", context.variant_count());
        for (i, variant) in context.variants.iter().enumerate() {
            println!("    [{}] \"{}\"", i, variant.source_text);
        }
        println!("⏱️  Prepare: {}", format_duration(prepare_duration));

        // 3. Translate using block method for consistency (NEW API)
        let translate_start = Instant::now();
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let source_texts = context.source_texts();
        let translated_texts = provider
            .translate_as_block(&source_texts, source_locale, target_locale)
            .await
            .expect("Block translation failed");
        context.update_translations(translated_texts);
        let translate_duration = translate_start.elapsed();

        println!("\n🌍 TRANSLATED VARIANTS:");
        for (i, variant) in context.variants.iter().enumerate() {
            println!(
                "  [{}] \"{}\" → \"{}\"",
                i, variant.source_text, variant.translated_text
            );
        }
        println!("⏱️  Translate: {}", format_duration(translate_duration));

        // Verify anchor preservation
        for variant in &context.variants {
            assert!(
                variant.translated_text.contains("777001"),
                "Anchor token 777001 should be preserved in: {}",
                variant.translated_text
            );
        }

        // 4. Reassemble using new Reassembler API
        let reassemble_start = Instant::now();
        let reassembler = Reassembler::new(context.variable_types.clone());
        let final_wikitext = reassembler
            .reassemble(context.variants.clone())
            .expect("Reassembly failed");
        let reassemble_duration = reassemble_start.elapsed();

        println!("\n🔧 REASSEMBLED WIKITEXT:");
        println!("  \"{}\"", final_wikitext);
        println!("⏱️  Reassemble: {}", format_duration(reassemble_duration));

        // Verify output contains placeholder
        assert!(
            final_wikitext.contains("$1"),
            "Final wikitext should contain $1 placeholder: {}",
            final_wikitext
        );

        let total_duration = overall_start.elapsed();
        println!("\n⏱️  TOTAL TIME: {}", format_duration(total_duration));
        println!("📊 API CALLS: 1 (block translation)");
        println!("{}", "=".repeat(80));
    }

    // ============================================================================
    // TEST 2: Message with PLURAL magic word
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_plural_expansion_new_api() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 2: PLURAL Magic Word with New API");
        println!("{}", "=".repeat(80));

        let source_message = "There {{PLURAL:$1|is|are}} $1 item";
        let source_locale = "en";
        let target_locale = "fr";

        println!("\n📝 SOURCE MESSAGE: \"{}\"", source_message);

        // Parse and prepare
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let mut context =
            prepare_for_translation(&ast, source_locale, "plural-test").expect("Failed to prepare");

        println!("\n📦 CONTEXT:");
        println!("  Variables: {:?}", context.variable_types);
        println!("  Variants: {}", context.variant_count());
        for (i, variant) in context.variants.iter().enumerate() {
            println!("    [{}] \"{}\"", i, variant.source_text);
        }

        // Translate
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let source_texts = context.source_texts();
        let translated_texts = provider
            .translate_as_block(&source_texts, source_locale, target_locale)
            .await
            .expect("Translation failed");
        context.update_translations(translated_texts);

        println!("\n🌍 TRANSLATED:");
        for variant in &context.variants {
            println!(
                "  \"{}\" → \"{}\"",
                variant.source_text, variant.translated_text
            );
        }

        // Reassemble
        let reassembler = Reassembler::new(context.variable_types.clone());
        let result = reassembler
            .reassemble(context.variants)
            .expect("Reassembly failed");

        println!("\n🔧 RESULT: \"{}\"", result);

        // Should contain PLURAL magic word and placeholder
        assert!(result.contains("PLURAL"));
        assert!(result.contains("$1"));

        println!("{}", "=".repeat(80));
    }

    // ============================================================================
    // TEST 3: Complex message with GENDER and PLURAL
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_gender_and_plural_new_api() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 3: GENDER + PLURAL Complex Message");
        println!("{}", "=".repeat(80));

        let source_message = "{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}";
        let source_locale = "en";
        let target_locale = "fr";

        println!("\n📝 SOURCE: \"{}\"", source_message);

        // Parse and prepare
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let mut context = prepare_for_translation(&ast, source_locale, "complex-test")
            .expect("Failed to prepare");

        println!("\n📦 CONTEXT:");
        println!("  Variables: {:?}", context.variable_types);
        println!(
            "  Variants: {} (should be 6: 3 GENDER × 2 PLURAL)",
            context.variant_count()
        );

        // Verify we have the expected number of variants
        assert_eq!(
            context.variant_count(),
            6,
            "Should have 6 variants (3 GENDER × 2 PLURAL)"
        );

        // Show sample of variants
        for (i, variant) in context.variants.iter().take(3).enumerate() {
            println!("    [{}] \"{}\"", i, variant.source_text);
        }
        if context.variant_count() > 3 {
            println!("    ... {} more variants", context.variant_count() - 3);
        }

        // Translate using block method
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let source_texts = context.source_texts();
        let translated_texts = provider
            .translate_as_block(&source_texts, source_locale, target_locale)
            .await
            .expect("Translation failed");
        context.update_translations(translated_texts);

        println!("\n🌍 SAMPLE TRANSLATIONS:");
        for (i, variant) in context.variants.iter().take(3).enumerate() {
            println!(
                "  [{}] \"{}\" → \"{}\"",
                i, variant.source_text, variant.translated_text
            );
        }

        // Reassemble
        let reassembler = Reassembler::new(context.variable_types.clone());
        let result = reassembler
            .reassemble(context.variants)
            .expect("Reassembly failed");

        println!("\n🔧 RESULT: \"{}\"", result);

        // Verify structure
        assert!(
            result.contains("GENDER"),
            "Should contain GENDER magic word"
        );
        assert!(
            result.contains("PLURAL"),
            "Should contain PLURAL magic word"
        );
        assert!(result.contains("$1"), "Should contain $1 placeholder");
        assert!(result.contains("$2"), "Should contain $2 placeholder");

        println!("✅ Complex message successfully processed!");
        println!("{}", "=".repeat(80));
    }

    // ============================================================================
    // TEST 4: Consistency Checking (Simulated)
    // ============================================================================

    #[test]
    fn test_consistency_checking() {
        println!("\n{}", "=".repeat(80));
        println!("TEST 4: Consistency Checking");
        println!("{}", "=".repeat(80));

        // Test similarity function
        let sim_identical = get_similarity("Hello world", "Hello world");
        assert_eq!(
            sim_identical, 1.0,
            "Identical strings should have 1.0 similarity"
        );

        let sim_similar = get_similarity("He sent a message", "She sent a message");
        println!("Similarity (similar): {:.2}", sim_similar);
        assert!(
            sim_similar > 0.7,
            "Similar strings should have high similarity"
        );

        let sim_different = get_similarity("He sent a message", "Completely different text");
        println!("Similarity (different): {:.2}", sim_different);
        assert!(
            sim_different < 0.7,
            "Different strings should have low similarity"
        );

        // Test consistency error detection
        use std::collections::HashMap;
        let mut var_types = HashMap::new();
        var_types.insert("$1".to_string(), "GENDER".to_string());
        let reassembler = Reassembler::new(var_types);

        // Create mock variants with very different translations
        let variant1 = TranslationVariant::with_translation(
            HashMap::from([("$1".to_string(), 0)]),
            "".to_string(),
            "He sent a message".to_string(),
        );
        let variant2 = TranslationVariant::with_translation(
            HashMap::from([("$1".to_string(), 1)]),
            "".to_string(),
            "Completely unrelated sentence".to_string(),
        );

        let variants = vec![variant1, variant2];
        let result = reassembler.reassemble(variants);

        assert!(result.is_err(), "Should detect consistency error");
        match result {
            Err(MtError::ConsistencyError(_)) => {
                println!("✅ Consistency error properly detected");
            }
            _ => panic!("Expected ConsistencyError"),
        }

        println!("{}", "=".repeat(80));
    }

    // ============================================================================
    // TEST 5: Performance Baseline
    // ============================================================================

    #[test]
    fn test_expansion_performance() {
        println!("\n{}", "=".repeat(80));
        println!("TEST 5: Performance Baseline");
        println!("{}", "=".repeat(80));

        let mut parser =
            Parser::new("{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}");
        let ast = parser.parse();

        let start = Instant::now();
        let variants = expand_to_variants(&ast, "en").expect("Expansion failed");
        let duration = start.elapsed();

        println!("Expansion of 6 variants: {}", format_duration(duration));
        assert_eq!(variants.len(), 6);
        assert!(duration.as_millis() < 100, "Should be fast (< 100ms)");

        // Test larger example
        let large_message =
            "{{PLURAL:$1|a|b}} {{PLURAL:$2|c|d}} {{PLURAL:$3|e|f}} {{PLURAL:$4|g|h}}";
        let mut parser = Parser::new(large_message);
        let ast = parser.parse();

        let start = Instant::now();
        let variants = expand_to_variants(&ast, "en").expect("Large expansion failed");
        let duration = start.elapsed();

        println!(
            "Expansion of {} variants: {}",
            variants.len(),
            format_duration(duration)
        );
        assert_eq!(variants.len(), 16); // 2^4
        assert!(
            duration.as_millis() < 200,
            "Should handle larger cases efficiently"
        );

        println!("✅ Performance within acceptable bounds");
        println!("{}", "=".repeat(80));
    }

    // ============================================================================
    // TEST 6: Full Workflow Demo (matching Python example)
    // ============================================================================

    #[test]
    fn test_full_workflow_demo_with_mock() {
        println!("\n{}", "=".repeat(80));
        println!("TEST 6: Full Workflow Demo (Mock Translation)");
        println!("{}", "=".repeat(80));
        println!("Purpose: Demonstrate complete workflow matching Python reference");

        // This matches the Python example from lines 349-364
        let source_message = "{{GENDER:$1|He|She}} sent {{PLURAL:$2|a message|$2 messages}} to {{GENDER:$3|him|her}}.";

        println!("\n📝 SOURCE: \"{}\"", source_message);

        // 1. Parse message
        let mut parser = crate::parser::Parser::new(source_message);
        let ast = parser.parse();
        println!("✅ Parsed AST ({} nodes)", ast.len());

        // 2. Prepare context (equivalent to Python prepare_for_translation)
        let mut context =
            prepare_for_translation(&ast, "en", "example-message").expect("Failed to prepare");

        println!("\n📦 EXPANSION:");
        println!("  Variables: {:?}", context.variable_types);
        println!("  Expected: {{\"$1\": \"GENDER\", \"$2\": \"PLURAL\", \"$3\": \"GENDER\"}}");
        println!(
            "  Variants: {} (should be 3×2×3 = 18)",
            context.variant_count()
        );

        // Should have 3 GENDER × 2 PLURAL × 3 GENDER = 18 variants
        // (Gender has 3 forms: male/female/unknown, Plural has 2 forms in English)
        assert_eq!(context.variant_count(), 18);
        assert_eq!(context.variable_types.len(), 3);
        assert_eq!(context.get_variable_type("$1"), Some(&"GENDER".to_string()));
        assert_eq!(context.get_variable_type("$2"), Some(&"PLURAL".to_string()));
        assert_eq!(context.get_variable_type("$3"), Some(&"GENDER".to_string()));

        // 3. Mock translation (simulate MT)
        let _mock_translator = crate::mt::MockTranslator::new(crate::mt::MockMode::Suffix);
        let source_texts = context.source_texts();

        // For demonstration, manually simulate translations to show the concept
        // In real usage, we'd call: mock_translator.translate_batch(&source_texts, "en", "fr").await
        let translated_texts: Vec<String> = source_texts
            .iter()
            .map(|text| format!("{}_fr", text)) // Simulate French translation
            .collect();

        context.update_translations(translated_texts);

        println!("\n🌍 MOCK TRANSLATIONS (sample):");
        for (i, variant) in context.variants.iter().take(3).enumerate() {
            println!(
                "  [{}] \"{}\" → \"{}\"",
                i, variant.source_text, variant.translated_text
            );
        }

        // 4. Reassemble (equivalent to Python Reassembler)
        let reassembler = Reassembler::new(context.variable_types.clone());
        let result = reassembler
            .reassemble(context.variants)
            .expect("Reassembly failed");

        println!("\n🔧 FINAL RESULT:");
        println!("  \"{}\"", result);

        // Should contain all magic words and placeholders
        assert!(result.contains("{{GENDER:$1|"));
        assert!(result.contains("{{PLURAL:$2|"));
        assert!(result.contains("{{GENDER:$3|"));
        assert!(result.contains("$2")); // From "$2 messages"

        println!("✅ Full workflow completed successfully!");
        println!("📊 Metrics:");
        println!("  - Source message: {} chars", source_message.len());
        println!("  - Variants expanded: {}", 8);
        println!("  - Final message: {} chars", result.len());
        println!("{}", "=".repeat(80));
    }
}
