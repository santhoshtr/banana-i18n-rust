//! End-to-End Integration Tests for Machine Translation Pipeline
//!
//! These tests exercise the complete pipeline from message parsing through expansion,
//! translation, reassembly, and placeholder recovery using the real Google Translate API.
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
    // TEST 2.1: Simple message with a single placeholder
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_simple_message_with_placeholder() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 2.1: Simple Message with Placeholder");
        println!("{}", "=".repeat(80));
        println!("Purpose: Validate basic placeholder expansion, translation, and recovery");
        println!("Validates: Iterations 1, 5-6, 8");

        let overall_start = Instant::now();

        // Setup
        let source_message = "Hello, $1!";
        let source_locale = "en";
        let target_locale = "fr";

        println!("\n📝 SOURCE MESSAGE:");
        println!("  Locale: {}", source_locale);
        println!("  Message: \"{}\"", source_message);

        // Parse message
        let parse_start = Instant::now();
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let parse_duration = parse_start.elapsed();
        println!("⏱️  Parse: {}", format_duration(parse_duration));

        // Expand variants with anchor tokens (Iteration 1)
        let expand_start = Instant::now();
        let variants = expand_all_variants(&ast, source_locale).expect("Failed to expand variants");
        let expand_duration = expand_start.elapsed();

        println!("\n📦 EXPANDED VARIANTS (with anchors):");
        for (i, variant) in variants.iter().enumerate() {
            println!("  [{}] \"{}\"", i, variant);
        }
        println!("⏱️  Expand: {}", format_duration(expand_duration));
        assert!(!variants.is_empty(), "Should have at least 1 variant");

        // Translate variants (Iterations 5-6)
        let translate_start = Instant::now();
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let translated = provider
            .translate_batch(&variants, source_locale, target_locale)
            .await
            .expect("Translation failed");
        let translate_duration = translate_start.elapsed();

        println!("\n🌍 TRANSLATED VARIANTS:");
        for (i, (src, tgt)) in variants.iter().zip(translated.iter()).enumerate() {
            println!("  [{}] \"{}\" → \"{}\"", i, src, tgt);
        }
        println!("⏱️  Translate: {}", format_duration(translate_duration));

        // Verify anchor preservation
        for trans in &translated {
            assert!(
                trans.contains("_ID1_"),
                "Anchor token _ID1_ should be preserved in translation"
            );
        }

        // Reassemble wikitext (Iteration 7)
        let reassemble_start = Instant::now();
        let reassembly_result =
            reassemble(&ast, &variants, &translated, target_locale).expect("Reassembly failed");
        let reassemble_duration = reassemble_start.elapsed();

        println!("\n🔧 REASSEMBLED WIKITEXT:");
        println!("  \"{}\"", reassembly_result.reconstructed_wikitext);
        println!("  Confidence: {:.2}%", reassembly_result.confidence * 100.0);
        if !reassembly_result.warnings.is_empty() {
            println!("  Warnings: {:?}", reassembly_result.warnings);
        }
        println!("⏱️  Reassemble: {}", format_duration(reassemble_duration));

        // Verify output contains placeholder
        assert!(
            reassembly_result.reconstructed_wikitext.contains("$1"),
            "Final wikitext should contain $1 placeholder"
        );

        let total_duration = overall_start.elapsed();
        println!("\n⏱️  TOTAL TIME: {}", format_duration(total_duration));
        println!("📊 API CALLS: 1 (single batch translation)");
        println!("{}", "=".repeat(80));
        println!();
    }

    // ============================================================================
    // TEST 2.2: Message with PLURAL magic word
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_plural_expansion_and_translation() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 2.2: Message with PLURAL Magic Word");
        println!("{}", "=".repeat(80));
        println!("Purpose: Validate PLURAL expansion, multi-variant translation, and reassembly");
        println!("Validates: Iterations 2, 5-6, 7, 8");

        let overall_start = Instant::now();

        // Setup
        let source_message = "There {{PLURAL:$1|is one item|are $1 items}}";
        let source_locale = "en";
        let target_locale = "fr";

        println!("\n📝 SOURCE MESSAGE:");
        println!("  Locale: {}", source_locale);
        println!("  Message: \"{}\"", source_message);

        // Parse message
        let parse_start = Instant::now();
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let parse_duration = parse_start.elapsed();
        println!("⏱️  Parse: {}", format_duration(parse_duration));

        // Expand variants (Iteration 2)
        let expand_start = Instant::now();
        let variants = expand_all_variants(&ast, source_locale).expect("Failed to expand variants");
        let expand_duration = expand_start.elapsed();

        println!("\n📦 EXPANDED VARIANTS (plural forms):");
        for (i, variant) in variants.iter().enumerate() {
            println!("  [{}] \"{}\"", i, variant);
        }
        println!(
            "⏱️  Expand: {} ({} variants)",
            format_duration(expand_duration),
            variants.len()
        );
        assert_eq!(variants.len(), 2, "English should have 2 plural forms");

        // Translate (Iterations 5-6)
        let translate_start = Instant::now();
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let translated = provider
            .translate_batch(&variants, source_locale, target_locale)
            .await
            .expect("Translation failed");
        let translate_duration = translate_start.elapsed();

        println!("\n🌍 TRANSLATED VARIANTS:");
        for (i, (src, tgt)) in variants.iter().zip(translated.iter()).enumerate() {
            println!("  [{}] \"{}\" → \"{}\"", i, src, tgt);
        }
        println!("⏱️  Translate: {}", format_duration(translate_duration));

        // Reassemble (Iteration 7)
        let reassemble_start = Instant::now();
        let reassembly_result =
            reassemble(&ast, &variants, &translated, target_locale).expect("Reassembly failed");
        let reassemble_duration = reassemble_start.elapsed();

        println!("\n🔧 REASSEMBLED WIKITEXT:");
        println!("  \"{}\"", reassembly_result.reconstructed_wikitext);
        println!("  Confidence: {:.2}%", reassembly_result.confidence * 100.0);
        if !reassembly_result.warnings.is_empty() {
            println!("  Warnings: {:?}", reassembly_result.warnings);
        }
        println!("⏱️  Reassemble: {}", format_duration(reassemble_duration));

        // Verify PLURAL syntax preserved
        assert!(
            reassembly_result.reconstructed_wikitext.contains("PLURAL")
                || reassembly_result.reconstructed_wikitext.contains("$1"),
            "Should preserve PLURAL syntax or contain placeholder"
        );

        let total_duration = overall_start.elapsed();
        println!("\n⏱️  TOTAL TIME: {}", format_duration(total_duration));
        println!(
            "📊 API CALLS: 1 (batch translation for {} variants)",
            variants.len()
        );
        println!("{}", "=".repeat(80));
        println!();
    }

    // ============================================================================
    // TEST 2.3: Message with GENDER magic word
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_gender_expansion_and_translation() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 2.3: Message with GENDER Magic Word");
        println!("{}", "=".repeat(80));
        println!("Purpose: Validate GENDER expansion and agreement in translation");
        println!("Validates: Iterations 3, 5-6, 7, 8");

        let overall_start = Instant::now();

        // Setup
        let source_message = "{{GENDER:$1|He is here|She is here|They are here}}";
        let source_locale = "en";
        let target_locale = "fr";

        println!("\n📝 SOURCE MESSAGE:");
        println!("  Locale: {}", source_locale);
        println!("  Message: \"{}\"", source_message);

        // Parse message
        let parse_start = Instant::now();
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let parse_duration = parse_start.elapsed();
        println!("⏱️  Parse: {}", format_duration(parse_duration));

        // Expand variants (Iteration 3)
        let expand_start = Instant::now();
        let variants = expand_all_variants(&ast, source_locale).expect("Failed to expand variants");
        let expand_duration = expand_start.elapsed();

        println!("\n📦 EXPANDED VARIANTS (gender forms):");
        for (i, variant) in variants.iter().enumerate() {
            let gender = match i {
                0 => "male",
                1 => "female",
                _ => "unknown",
            };
            println!("  [{}] ({}) \"{}\"", i, gender, variant);
        }
        println!(
            "⏱️  Expand: {} ({} variants)",
            format_duration(expand_duration),
            variants.len()
        );
        assert_eq!(variants.len(), 3, "Should have 3 gender variants");

        // Translate (Iterations 5-6)
        let translate_start = Instant::now();
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let translated = provider
            .translate_batch(&variants, source_locale, target_locale)
            .await
            .expect("Translation failed");
        let translate_duration = translate_start.elapsed();

        println!("\n🌍 TRANSLATED VARIANTS:");
        for (i, (src, tgt)) in variants.iter().zip(translated.iter()).enumerate() {
            let gender = match i {
                0 => "male",
                1 => "female",
                _ => "unknown",
            };
            println!("  [{}] ({}) \"{}\" → \"{}\"", i, gender, src, tgt);
        }
        println!("⏱️  Translate: {}", format_duration(translate_duration));

        // Reassemble (Iteration 7)
        let reassemble_start = Instant::now();
        let reassembly_result =
            reassemble(&ast, &variants, &translated, target_locale).expect("Reassembly failed");
        let reassemble_duration = reassemble_start.elapsed();

        println!("\n🔧 REASSEMBLED WIKITEXT:");
        println!("  \"{}\"", reassembly_result.reconstructed_wikitext);
        println!("  Confidence: {:.2}%", reassembly_result.confidence * 100.0);
        if !reassembly_result.warnings.is_empty() {
            println!("  Warnings: {:?}", reassembly_result.warnings);
        }
        println!("⏱️  Reassemble: {}", format_duration(reassemble_duration));

        let total_duration = overall_start.elapsed();
        println!("\n⏱️  TOTAL TIME: {}", format_duration(total_duration));
        println!(
            "📊 API CALLS: 1 (batch translation for {} variants)",
            variants.len()
        );
        println!("{}", "=".repeat(80));
        println!();
    }

    // ============================================================================
    // TEST 2.4: Complex message with PLURAL × GENDER cartesian product
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_plural_and_gender_cartesian_product() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 2.4: PLURAL × GENDER Cartesian Product");
        println!("{}", "=".repeat(80));
        println!("Purpose: Validate complex message with multiple magic words");
        println!("Validates: Iterations 4, 5-6, 7, 8");

        let overall_start = Instant::now();

        // Setup
        let source_message = "{{GENDER:$1|He|She}} sent {{PLURAL:$2|a message|$2 messages}}";
        let source_locale = "en";
        let target_locale = "fr";

        println!("\n📝 SOURCE MESSAGE:");
        println!("  Locale: {}", source_locale);
        println!("  Message: \"{}\"", source_message);

        // Parse message
        let parse_start = Instant::now();
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let parse_duration = parse_start.elapsed();
        println!("⏱️  Parse: {}", format_duration(parse_duration));

        // Expand variants (Iteration 4)
        let expand_start = Instant::now();
        let variants = expand_all_variants(&ast, source_locale).expect("Failed to expand variants");
        let expand_duration = expand_start.elapsed();

        println!("\n📦 EXPANDED VARIANTS (3 gender × 2 plural = 6 total):");
        for (i, variant) in variants.iter().enumerate() {
            println!("  [{}] \"{}\"", i, variant);
        }
        println!(
            "⏱️  Expand: {} ({} variants)",
            format_duration(expand_duration),
            variants.len()
        );
        assert_eq!(
            variants.len(),
            6,
            "Should have 3 gender × 2 plural = 6 variants"
        );

        // Translate (Iterations 5-6)
        let translate_start = Instant::now();
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let translated = provider
            .translate_batch(&variants, source_locale, target_locale)
            .await
            .expect("Translation failed");
        let translate_duration = translate_start.elapsed();

        println!("\n🌍 TRANSLATED VARIANTS:");
        for (i, (src, tgt)) in variants.iter().zip(translated.iter()).enumerate() {
            println!("  [{}] \"{}\" → \"{}\"", i, src, tgt);
        }
        println!("⏱️  Translate: {}", format_duration(translate_duration));

        // Verify anchor tokens preserved
        for trans in &translated {
            assert!(
                trans.contains("_ID1_"),
                "Anchor _ID1_ (for GENDER) should be preserved"
            );
            assert!(
                trans.contains("_ID2_"),
                "Anchor _ID2_ (for PLURAL) should be preserved"
            );
        }

        // Reassemble (Iteration 7)
        let reassemble_start = Instant::now();
        let reassembly_result =
            reassemble(&ast, &variants, &translated, target_locale).expect("Reassembly failed");
        let reassemble_duration = reassemble_start.elapsed();

        println!("\n🔧 REASSEMBLED WIKITEXT:");
        println!("  \"{}\"", reassembly_result.reconstructed_wikitext);
        println!("  Confidence: {:.2}%", reassembly_result.confidence * 100.0);
        println!(
            "  Extracted forms: {} magic words",
            reassembly_result.extracted_forms.len()
        );
        if !reassembly_result.warnings.is_empty() {
            println!("  Warnings: {:?}", reassembly_result.warnings);
        }
        println!("⏱️  Reassemble: {}", format_duration(reassemble_duration));

        // Verify both placeholders present
        assert!(
            reassembly_result.reconstructed_wikitext.contains("$1")
                || reassembly_result.reconstructed_wikitext.contains("GENDER"),
            "Should contain $1 placeholder or GENDER syntax"
        );
        assert!(
            reassembly_result.reconstructed_wikitext.contains("$2")
                || reassembly_result.reconstructed_wikitext.contains("PLURAL"),
            "Should contain $2 placeholder or PLURAL syntax"
        );

        let total_duration = overall_start.elapsed();
        println!("\n⏱️  TOTAL TIME: {}", format_duration(total_duration));
        println!(
            "📊 API CALLS: 1 (batch translation for {} variants)",
            variants.len()
        );
        println!("{}", "=".repeat(80));
        println!();
    }

    // ============================================================================
    // TEST 2.5: Multiple placeholders with real MT
    // ============================================================================

    #[tokio::test]
    #[ignore]
    async fn test_e2e_multiple_placeholders() {
        if !require_api_key() {
            eprintln!("⚠️  Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        println!("\n{}", "=".repeat(80));
        println!("TEST 2.5: Multiple Placeholders");
        println!("{}", "=".repeat(80));
        println!("Purpose: Validate recovery of multiple placeholders");
        println!("Validates: Iterations 1, 5-6, 8");

        let overall_start = Instant::now();

        // Setup
        let source_message = "$1 told $2 about $3";
        let source_locale = "en";
        let target_locale = "de";

        println!("\n📝 SOURCE MESSAGE:");
        println!("  Locale: {}", source_locale);
        println!("  Message: \"{}\"", source_message);

        // Parse and expand
        let parse_start = Instant::now();
        let mut parser = Parser::new(source_message);
        let ast = parser.parse();
        let parse_duration = parse_start.elapsed();

        let expand_start = Instant::now();
        let variants = expand_all_variants(&ast, source_locale).expect("Failed to expand");
        let expand_duration = expand_start.elapsed();

        println!("⏱️  Parse: {}", format_duration(parse_duration));
        println!("⏱️  Expand: {}", format_duration(expand_duration));

        println!("\n📦 EXPANDED VARIANTS:");
        for (i, variant) in variants.iter().enumerate() {
            println!("  [{}] \"{}\"", i, variant);
        }

        // Translate
        let translate_start = Instant::now();
        let provider = GoogleTranslateProvider::from_env().expect("Failed to load provider");
        let translated = provider
            .translate_batch(&variants, source_locale, target_locale)
            .await
            .expect("Translation failed");
        let translate_duration = translate_start.elapsed();

        println!("\n🌍 TRANSLATED VARIANTS:");
        for (i, (src, tgt)) in variants.iter().zip(translated.iter()).enumerate() {
            println!("  [{}] \"{}\" → \"{}\"", i, src, tgt);
        }
        println!("⏱️  Translate: {}", format_duration(translate_duration));

        // Verify all anchors present
        for trans in &translated {
            assert!(trans.contains("_ID1_"), "Should contain _ID1_");
            assert!(trans.contains("_ID2_"), "Should contain _ID2_");
            assert!(trans.contains("_ID3_"), "Should contain _ID3_");
        }

        // Reassemble
        let reassembly_result =
            reassemble(&ast, &variants, &translated, target_locale).expect("Reassembly failed");

        println!("\n🔧 REASSEMBLED WIKITEXT:");
        println!("  \"{}\"", reassembly_result.reconstructed_wikitext);

        // Verify all placeholders recovered
        assert!(
            reassembly_result.reconstructed_wikitext.contains("$1"),
            "Should contain $1"
        );
        assert!(
            reassembly_result.reconstructed_wikitext.contains("$2"),
            "Should contain $2"
        );
        assert!(
            reassembly_result.reconstructed_wikitext.contains("$3"),
            "Should contain $3"
        );

        let total_duration = overall_start.elapsed();
        println!("\n⏱️  TOTAL TIME: {}", format_duration(total_duration));
        println!("📊 API CALLS: 1");
        println!("{}", "=".repeat(80));
        println!();
    }
}
