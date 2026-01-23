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

use crate::mt::error::MtResult;
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
        return Err(crate::mt::error::MtError::InvalidLocale(
            "Locale code is empty".to_string(),
        ));
    }

    // Check that locale contains only valid characters
    if !locale
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(crate::mt::error::MtError::InvalidLocale(format!(
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
        use crate::mt::error::MtError;
        match validate_locale("en@US") {
            Err(MtError::InvalidLocale(msg)) => {
                assert!(msg.contains("Invalid characters"));
            }
            _ => panic!("Expected InvalidLocale error"),
        }
    }
}
