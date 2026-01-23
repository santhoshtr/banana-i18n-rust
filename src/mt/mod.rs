/// Machine Translation Module
///
/// This module provides machine translation suggestion capabilities for the banana-i18n library.
/// It implements the Cartesian Expansion and Diff-and-Capture algorithm to generate high-quality
/// translation suggestions while preserving complex wikitext features like PLURAL, GENDER, and
/// parameterized placeholders.
///
/// # Overview
///
/// The MT module consists of several components working together:
///
/// 1. **Expansion Engine** - Converts wikitext with magic words into all possible plain-text variants
/// 2. **MT Trait & Providers** - Generic trait for MT systems with Google Translate implementation
/// 3. **Reassembly Engine** - Reconstructs wikitext from translated variants using Diff-and-Capture
/// 4. **Consistency Checker** - Validates translations for hallucinations and anomalies
/// 5. **Suggestion Generator** - Orchestrates the full pipeline
///
/// # Example
///
/// ```ignore
/// use banana_i18n::mt::{MachineTranslator, GoogleTranslateProvider, expand_all_variants};
/// use banana_i18n::parser::Parser;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Parse message
///     let mut parser = Parser::new("{{GENDER:$1|He|She}} sent {{PLURAL:$2|a message|$2 messages}}");
///     let ast = parser.parse();
///
///     // Expand variants
///     let variants = expand_all_variants(&ast, "en")?;
///
///     // Translate with provider
///     let provider = GoogleTranslateProvider::from_env()?;
///     let translated = provider.translate_batch(&variants, "en", "fr").await?;
///
///     println!("{:?}", translated);
///     Ok(())
/// }
/// ```
pub mod anchor;
pub mod error;
pub mod expansion;
pub mod gender_expansion;
pub mod google_translate;
pub mod mock;
pub mod plural_expansion;
pub mod translator;

pub use anchor::{
    AnchorToken, generate_anchor_tokens, recover_placeholders_from_anchors,
    replace_placeholders_with_anchors,
};
pub use error::{MtError, MtResult};
pub use expansion::{calculate_variant_count, expand_all_variants};
pub use gender_expansion::{GenderForm, expand_gender_variants, get_gender_forms};
pub use google_translate::GoogleTranslateProvider;
pub use mock::{MockMode, MockTranslator};
pub use plural_expansion::{expand_plural_variants, get_plural_forms_for_language};
pub use translator::MachineTranslator;
