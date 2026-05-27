//! Machine Translation trait and utilities
//!
//! This module defines the `MachineTranslator` trait for provider abstraction,
//! enabling support for different MT backends (Google Translate, mock, etc.)
//! without coupling the library to any specific implementation.
//!
//! # Example
//!
//! ```ignore
//! use banana_i18n::mt::{MachineTranslator, GoogleTranslateProvider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a provider
//!     let provider = GoogleTranslateProvider::from_env()?;
//!
//!     // Translate a single string
//!     let result = provider.translate("Hello, world!", "en", "fr").await?;
//!     println!("{}", result); // "Bonjour, le monde!"
//!
//!     // Translate multiple strings in a batch
//!     let texts = vec!["Hello".to_string(), "Goodbye".to_string()];
//!     let results = provider.translate_batch(&texts, "en", "fr").await?;
//!     println!("{:?}", results);
//!
//!     Ok(())
//! }
//! ```

use crate::error::MtResult;
use async_trait::async_trait;

/// Generic trait for machine translation providers
///
/// Implementations of this trait handle the actual translation work,
/// whether through an API (Google Translate) or deterministic logic (Mock).
///
/// All methods are async to support I/O-bound operations like network requests.
#[async_trait]
pub trait MachineTranslator: Send + Sync {
    /// Translate a single text string from source to target locale
    ///
    /// # Arguments
    ///
    /// * `text` - The text to translate
    /// * `source_locale` - Source language code (e.g., "en", "en-US")
    /// * `target_locale` - Target language code (e.g., "fr", "fr-FR")
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The translated text
    /// * `Err(MtError)` - If translation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = provider.translate("Hello", "en", "fr").await?;
    /// assert_eq!(result, "Bonjour");
    /// ```
    async fn translate(
        &self,
        text: &str,
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<String>;

    /// Translate multiple strings in a single batch operation
    ///
    /// Batch translation is more efficient than individual translations,
    /// especially for providers with per-request overhead (like API calls).
    /// Implementations may chunk large batches internally.
    ///
    /// # Arguments
    ///
    /// * `texts` - Vector of strings to translate
    /// * `source_locale` - Source language code
    /// * `target_locale` - Target language code
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - Translated strings in the same order as input
    /// * `Err(MtError)` - If translation fails
    ///
    /// # Guarantees
    ///
    /// - Output order matches input order
    /// - Output length equals input length
    /// - Each translation is independent
    ///
    /// # Example
    ///
    /// ```ignore
    /// let texts = vec!["Hello".to_string(), "Goodbye".to_string()];
    /// let results = provider.translate_batch(&texts, "en", "fr").await?;
    /// assert_eq!(results.len(), 2);
    /// ```
    async fn translate_batch(
        &self,
        texts: &[String],
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<Vec<String>>;

    /// Get the name of this translation provider
    ///
    /// Used for logging and debugging to identify which provider handled a translation.
    ///
    /// # Returns
    ///
    /// A string identifying the provider (e.g., "Google Translate", "Mock Translator")
    fn provider_name(&self) -> &str;

    /// Translate multiple variants as a single numbered block.
    ///
    /// Joins all variants with numbered prefixes (`1. ...`, `2. ...`) and
    /// translates them in **one** `translate` call, so the MT engine sees the
    /// related variants together and keeps wording consistent across them
    /// (important for PLURAL/GENDER forms). The block is then split back into
    /// individual translations.
    ///
    /// This default implementation is provider-agnostic — it relies only on
    /// [`translate`](Self::translate). Providers may override it if they need
    /// different behaviour.
    ///
    /// # Guarantees
    ///
    /// - Output order matches input order
    /// - Output length equals input length
    async fn translate_as_block(
        &self,
        variants: &[String],
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<Vec<String>> {
        // Handle empty case
        if variants.is_empty() {
            return Ok(Vec::new());
        }

        // Handle single variant case — no numbering needed
        if variants.len() == 1 {
            let result = self
                .translate(&variants[0], source_locale, target_locale)
                .await?;
            return Ok(vec![result]);
        }

        // 1. Join with numbered prefixes
        let input_block: String = variants
            .iter()
            .enumerate()
            .map(|(i, variant)| format!("{}. {}", i + 1, variant))
            .collect::<Vec<_>>()
            .join("\n");

        // 2. Translate the entire block in a single call
        let translated_block = self
            .translate(&input_block, source_locale, target_locale)
            .await?;

        // 3. Split back using the numbered prefixes
        use regex::Regex;
        let re = Regex::new(r"\n?\d+\.\s").unwrap();
        let lines: Vec<String> = re
            .split(translated_block.trim())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // 4. Safety check: same count
        if lines.len() != variants.len() {
            return Err(crate::error::MtError::TranslationError(format!(
                "Block translation count mismatch: expected {}, got {}. Block: '{}'",
                variants.len(),
                lines.len(),
                translated_block
            )));
        }

        // 5. Clean up anchor token mangling
        let cleaned: Vec<String> = lines.iter().map(|line| clean_anchor_mangling(line)).collect();

        Ok(cleaned)
    }
}

/// Undo the one anchor-token mangling we can safely repair: whitespace MT
/// inserted *inside* the anchor digits, e.g. `"777 001"` → `"777001"`.
///
/// Spaces *around* a complete anchor are deliberately left alone — a space
/// before the anchor is a word separator (e.g. Hindi `"निम्नलिखित 777001 फ़ाइलें"`,
/// "the following $1 files"); stripping it would glue the placeholder to the
/// preceding word and yield `"निम्नलिखित$1"`. See
/// docs/mt_assisted_localization.md §9.5.
pub(crate) fn clean_anchor_mangling(line: &str) -> String {
    line.replace("777 ", "777")
}

/// Normalize a locale code by stripping region information
///
/// Converts locale codes from BCP 47 format to ISO 639-1 format:
/// - `en-US` → `en`
/// - `zh-Hans` → `zh`
/// - `fr-FR` → `fr`
/// - `en` → `en` (unchanged)
///
/// # Arguments
///
/// * `locale` - The locale code to normalize
///
/// # Returns
///
/// The normalized locale code (base language only)
///
/// # Example
///
/// ```ignore
/// assert_eq!(normalize_locale("en-US"), "en");
/// assert_eq!(normalize_locale("zh-Hans"), "zh");
/// ```
pub fn normalize_locale(locale: &str) -> String {
    // Split on hyphen and take the first part (language code)
    locale.split('-').next().unwrap_or(locale).to_lowercase()
}

/// Validate that a locale code is in acceptable format
///
/// Checks that the locale code contains only alphanumeric characters,
/// hyphens, and underscores (following ISO 639 conventions).
///
/// # Arguments
///
/// * `locale` - The locale code to validate
///
/// # Returns
///
/// * `Ok(())` - If the locale is valid
/// * `Err(MtError)` - If the locale is invalid
///
/// # Example
///
/// ```ignore
/// validate_locale("en")?; // OK
/// validate_locale("en-US")?; // OK
/// validate_locale("invalid@code").unwrap_err(); // Error
/// ```
pub fn validate_locale(locale: &str) -> MtResult<()> {
    if locale.is_empty() {
        return Err(crate::error::MtError::InvalidLocale(
            "Locale code is empty".to_string(),
        ));
    }

    // Check that locale contains only valid characters
    if !locale
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(crate::error::MtError::InvalidLocale(format!(
            "Invalid characters in locale code: {}",
            locale
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_locale_with_region() {
        assert_eq!(normalize_locale("en-US"), "en");
        assert_eq!(normalize_locale("en-GB"), "en");
        assert_eq!(normalize_locale("fr-FR"), "fr");
    }

    #[test]
    fn test_normalize_locale_with_script() {
        assert_eq!(normalize_locale("zh-Hans"), "zh");
        assert_eq!(normalize_locale("zh-Hant"), "zh");
        assert_eq!(normalize_locale("sr-Latn"), "sr");
    }

    #[test]
    fn test_normalize_locale_complex() {
        // Even complex codes get normalized to language only
        assert_eq!(normalize_locale("de-AT-1996"), "de");
    }

    #[test]
    fn test_normalize_locale_already_simple() {
        assert_eq!(normalize_locale("en"), "en");
        assert_eq!(normalize_locale("fr"), "fr");
        assert_eq!(normalize_locale("ru"), "ru");
    }

    #[test]
    fn test_normalize_locale_case_insensitive() {
        // Ensures lowercase output
        assert_eq!(normalize_locale("EN"), "en");
        assert_eq!(normalize_locale("EN-US"), "en");
    }

    #[test]
    fn test_validate_locale_valid_codes() {
        assert!(validate_locale("en").is_ok());
        assert!(validate_locale("en-US").is_ok());
        assert!(validate_locale("zh-Hans").is_ok());
        assert!(validate_locale("de_DE").is_ok());
    }

    #[test]
    fn test_validate_locale_invalid_codes() {
        assert!(validate_locale("").is_err());
        assert!(validate_locale("en@invalid").is_err());
        assert!(validate_locale("fr#bad").is_err());
        assert!(validate_locale("es!error").is_err());
    }

    #[test]
    fn test_validate_locale_error_messages() {
        use crate::error::MtError;
        match validate_locale("en@US") {
            Err(MtError::InvalidLocale(msg)) => {
                assert!(msg.contains("Invalid characters"));
            }
            _ => panic!("Expected InvalidLocale error"),
        }
    }

    // ========== Anchor Cleanup Tests ==========

    #[test]
    fn test_clean_anchor_preserves_space_before_anchor() {
        // Regression for the reported bug: the space between निम्नलिखित
        // ("following") and the anchor 777001 ($1) must be preserved.
        // Previously `.replace(" 777", "777")` glued them into निम्नलिखित777001.
        let input = "निम्नलिखित 777001 फ़ाइलें वर्तमान श्रेणी में हैं।";
        assert_eq!(clean_anchor_mangling(input), input);
    }

    #[test]
    fn test_clean_anchor_rejoins_split_anchor() {
        // The one mangling we DO repair: a space MT inserted inside the digits.
        assert_eq!(
            clean_anchor_mangling("The following 777 001 files"),
            "The following 777001 files"
        );
    }

    #[test]
    fn test_clean_anchor_intact_unchanged() {
        assert_eq!(clean_anchor_mangling("777001 फ़ाइलें"), "777001 फ़ाइलें");
    }

    #[test]
    fn test_clean_anchor_ascii_unchanged() {
        assert_eq!(
            clean_anchor_mangling("He sent 777001 messages"),
            "He sent 777001 messages"
        );
    }
}
