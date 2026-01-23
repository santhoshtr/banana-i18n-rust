//! Google Translate API provider for machine translation
//!
//! This module integrates with Google Translate API v2 to provide real
//! machine translation capabilities.
//!
//! # Authentication
//!
//! The provider loads the API key from the `GOOGLE_TRANSLATE_API_KEY`
//! environment variable. Obtain a key from:
//! https://console.cloud.google.com/
//!
//! # Example
//!
//! ```ignore
//! use banana_i18n::mt::{MachineTranslator, GoogleTranslateProvider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load from environment
//!     let provider = GoogleTranslateProvider::from_env()?;
//!
//!     // Translate a single string
//!     let result = provider.translate("Hello, world!", "en", "fr").await?;
//!     println!("{}", result);
//!
//!     // Translate multiple strings
//!     let texts = vec!["Hello".to_string(), "Goodbye".to_string()];
//!     let results = provider.translate_batch(&texts, "en", "fr").await?;
//!     println!("{:?}", results);
//!
//!     Ok(())
//! }
//! ```

use crate::mt::error::{MtError, MtResult};
use crate::mt::translator::{MachineTranslator, normalize_locale, validate_locale};
use async_trait::async_trait;
use serde_json::json;

/// Google Translate API v2 provider
///
/// Communicates with Google's translation API to perform real translations.
/// Supports both single and batch translations with automatic request chunking.
#[derive(Clone)]
pub struct GoogleTranslateProvider {
    /// API key for authentication
    api_key: String,
    /// HTTP client for async requests
    client: reqwest::Client,
    /// Base URL for Google Translate API
    base_url: String,
}

impl GoogleTranslateProvider {
    /// Maximum number of texts per API request
    /// Google Translate v2 API accepts up to 128 texts per request
    const MAX_BATCH_SIZE: usize = 128;

    /// Maximum characters per string (30KB per Google Translate API limits)
    const MAX_CHARS_PER_STRING: usize = 30_000;

    /// Create a new GoogleTranslateProvider with an explicit API key
    ///
    /// # Arguments
    ///
    /// * `api_key` - Google Translate API key
    ///
    /// # Returns
    ///
    /// * `Ok(Self)` - New provider instance
    /// * `Err(MtError)` - If API key is empty or HTTP client creation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = GoogleTranslateProvider::new("your-api-key")?;
    /// ```
    pub fn new(api_key: String) -> MtResult<Self> {
        if api_key.trim().is_empty() {
            return Err(MtError::ConfigError("API key cannot be empty".to_string()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| MtError::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            api_key,
            client,
            base_url: "https://translation.googleapis.com/language/translate/v2".to_string(),
        })
    }

    /// Create a GoogleTranslateProvider from the `GOOGLE_TRANSLATE_API_KEY` environment variable
    ///
    /// # Returns
    ///
    /// * `Ok(Self)` - New provider instance
    /// * `Err(MtError)` - If environment variable is not set or creation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = GoogleTranslateProvider::from_env()?;
    /// ```
    pub fn from_env() -> MtResult<Self> {
        let api_key = std::env::var("GOOGLE_TRANSLATE_API_KEY").map_err(|_| {
            MtError::ConfigError(
                "GOOGLE_TRANSLATE_API_KEY environment variable not set".to_string(),
            )
        })?;

        Self::new(api_key)
    }

    /// Chunk a batch of texts into API-safe sizes
    ///
    /// Google Translate API has a limit of 128 texts per request.
    /// This method chunks large batches transparently.
    ///
    /// # Arguments
    ///
    /// * `texts` - All texts to chunk
    ///
    /// # Returns
    ///
    /// Vector of text slices, each of size ≤ MAX_BATCH_SIZE
    fn chunk_batch<'a>(texts: &'a [String]) -> Vec<&'a [String]> {
        texts.chunks(Self::MAX_BATCH_SIZE).collect()
    }

    /// Translate a single chunk of texts via the API
    ///
    /// # Arguments
    ///
    /// * `texts` - Texts to translate (should be ≤ MAX_BATCH_SIZE)
    /// * `source_locale` - Source language
    /// * `target_locale` - Target language
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - Translated texts
    /// * `Err(MtError)` - If API call fails
    async fn translate_chunk(
        &self,
        texts: &[String],
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<Vec<String>> {
        // Validate inputs
        validate_locale(source_locale)?;
        validate_locale(target_locale)?;

        // Build request URL with API key
        let url = format!("{}?key={}", self.base_url, self.api_key);

        // Build request body
        let body = json!({
            "q": texts,
            "source": normalize_locale(source_locale),
            "target": normalize_locale(target_locale),
            "format": "text"
        });

        // Send POST request
        let response = self.client.post(&url).json(&body).send().await?;

        // Check HTTP status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(if status.is_client_error() {
                MtError::ConfigError(format!("API client error ({}): {}", status, error_text))
            } else {
                MtError::TranslationError(format!("API server error ({}): {}", status, error_text))
            });
        }

        // Parse response JSON
        let json: serde_json::Value = response.json().await.map_err(|e| {
            MtError::TranslationError(format!("Failed to parse API response: {}", e))
        })?;

        // Extract translations from nested response
        let translations = json["data"]["translations"].as_array().ok_or_else(|| {
            MtError::TranslationError(
                "Invalid API response: missing 'data.translations' array".to_string(),
            )
        })?;

        // Extract translatedText from each translation object
        let results: MtResult<Vec<String>> = translations
            .iter()
            .map(|t| {
                t["translatedText"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| {
                        MtError::TranslationError(
                            "Invalid API response: missing 'translatedText' field".to_string(),
                        )
                    })
            })
            .collect();

        results
    }
}

impl std::fmt::Debug for GoogleTranslateProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleTranslateProvider")
            .field("api_key", &"***")
            .field("base_url", &self.base_url)
            .finish()
    }
}

#[async_trait]
impl MachineTranslator for GoogleTranslateProvider {
    async fn translate(
        &self,
        text: &str,
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<String> {
        // Validate inputs
        validate_locale(source_locale)?;
        validate_locale(target_locale)?;

        if text.is_empty() {
            return Ok(String::new());
        }

        // Check character limit
        if text.len() > Self::MAX_CHARS_PER_STRING {
            return Err(MtError::TranslationError(format!(
                "Text exceeds maximum length of {} characters",
                Self::MAX_CHARS_PER_STRING
            )));
        }

        // Translate as a single-item batch
        let results = self
            .translate_chunk(&[text.to_string()], source_locale, target_locale)
            .await?;

        Ok(results.into_iter().next().unwrap_or_default())
    }

    async fn translate_batch(
        &self,
        texts: &[String],
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<Vec<String>> {
        // Validate inputs
        validate_locale(source_locale)?;
        validate_locale(target_locale)?;

        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Validate each text
        for (i, text) in texts.iter().enumerate() {
            if text.len() > Self::MAX_CHARS_PER_STRING {
                return Err(MtError::TranslationError(format!(
                    "Text at index {} exceeds maximum length of {} characters",
                    i,
                    Self::MAX_CHARS_PER_STRING
                )));
            }
        }

        // Chunk texts for API limits
        let chunks = Self::chunk_batch(texts);
        let mut all_results = Vec::new();

        // Process each chunk sequentially
        for chunk in chunks {
            let chunk_results = self
                .translate_chunk(chunk, source_locale, target_locale)
                .await?;
            all_results.extend(chunk_results);
        }

        // Verify output length matches input length
        assert_eq!(
            all_results.len(),
            texts.len(),
            "Output length must match input length"
        );

        Ok(all_results)
    }

    fn provider_name(&self) -> &str {
        "Google Translate"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Initialization Tests ==========

    #[test]
    fn test_new_with_valid_key() {
        let provider = GoogleTranslateProvider::new("test-api-key".to_string());
        assert!(provider.is_ok());
        assert_eq!(provider.unwrap().provider_name(), "Google Translate");
    }

    #[test]
    fn test_new_with_empty_key() {
        let result = GoogleTranslateProvider::new("".to_string());
        assert!(result.is_err());
        match result {
            Err(MtError::ConfigError(msg)) => assert!(msg.contains("empty")),
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_new_with_whitespace_key() {
        let result = GoogleTranslateProvider::new("   ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_from_env_without_key() {
        // Ensure env var is not set for this test
        unsafe {
            std::env::remove_var("GOOGLE_TRANSLATE_API_KEY");
        }
        let result = GoogleTranslateProvider::from_env();
        assert!(result.is_err());
        match result {
            Err(MtError::ConfigError(msg)) => assert!(msg.contains("not set")),
            _ => panic!("Expected ConfigError"),
        }
    }

    // ========== Chunking Tests ==========

    #[test]
    fn test_chunk_under_limit() {
        let texts = vec!["hello".to_string(), "world".to_string()];
        let chunks = GoogleTranslateProvider::chunk_batch(&texts);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 2);
    }

    #[test]
    fn test_chunk_at_limit() {
        let texts = (0..128).map(|i| format!("text{}", i)).collect::<Vec<_>>();
        let chunks = GoogleTranslateProvider::chunk_batch(&texts);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 128);
    }

    #[test]
    fn test_chunk_over_limit() {
        let texts = (0..256).map(|i| format!("text{}", i)).collect::<Vec<_>>();
        let chunks = GoogleTranslateProvider::chunk_batch(&texts);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 128);
        assert_eq!(chunks[1].len(), 128);
    }

    #[test]
    fn test_chunk_partial_chunk() {
        let texts = (0..200).map(|i| format!("text{}", i)).collect::<Vec<_>>();
        let chunks = GoogleTranslateProvider::chunk_batch(&texts);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 128);
        assert_eq!(chunks[1].len(), 72);
    }

    #[test]
    fn test_chunk_empty() {
        let texts: Vec<String> = vec![];
        let chunks = GoogleTranslateProvider::chunk_batch(&texts);
        assert_eq!(chunks.len(), 0);
    }

    // ========== Validation Tests ==========

    #[tokio::test]
    async fn test_translate_empty_text() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        let result = provider.translate("", "en", "fr").await.unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_translate_invalid_source_locale() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        let result = provider.translate("hello", "invalid@code", "fr").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_translate_invalid_target_locale() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        let result = provider.translate("hello", "en", "invalid#code").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_translate_text_too_long() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        let long_text = "x".repeat(GoogleTranslateProvider::MAX_CHARS_PER_STRING + 1);
        let result = provider.translate(&long_text, "en", "fr").await;
        assert!(result.is_err());
        match result {
            Err(MtError::TranslationError(msg)) => assert!(msg.contains("exceeds maximum")),
            _ => panic!("Expected TranslationError"),
        }
    }

    #[tokio::test]
    async fn test_batch_empty() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        let texts: Vec<String> = vec![];
        let results = provider.translate_batch(&texts, "en", "fr").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_text_too_long() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        let long_text = "x".repeat(GoogleTranslateProvider::MAX_CHARS_PER_STRING + 1);
        let texts = vec![long_text];
        let result = provider.translate_batch(&texts, "en", "fr").await;
        assert!(result.is_err());
    }

    // ========== Provider Name Test ==========

    #[test]
    fn test_provider_name() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        assert_eq!(provider.provider_name(), "Google Translate");
    }

    // ========== Debug Implementation Test ==========

    #[test]
    fn test_debug_output() {
        let provider = GoogleTranslateProvider::new("test-key".to_string()).unwrap();
        let debug_str = format!("{:?}", provider);
        // API key should be masked
        assert!(debug_str.contains("***"));
        assert!(!debug_str.contains("test-key"));
    }

    // ========== Integration Tests (require real API key) ==========

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored
    async fn test_real_api_single_translation() {
        if std::env::var("GOOGLE_TRANSLATE_API_KEY").is_err() {
            eprintln!("Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        let provider = GoogleTranslateProvider::from_env().unwrap();
        let result = provider.translate("Hello", "en", "fr").await.unwrap();
        println!("Translation: {} → {}", "Hello", result);

        // Should contain a valid French translation
        assert!(!result.is_empty());
        assert!(result.len() > 0);
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored
    async fn test_real_api_batch_translation() {
        if std::env::var("GOOGLE_TRANSLATE_API_KEY").is_err() {
            eprintln!("Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        let provider = GoogleTranslateProvider::from_env().unwrap();
        let texts = vec!["Hello".to_string(), "Goodbye".to_string()];
        let results = provider.translate_batch(&texts, "en", "fr").await.unwrap();

        assert_eq!(results.len(), 2);
        for (input, output) in texts.iter().zip(results.iter()) {
            println!("Translation: {} → {}", input, output);
            assert!(!output.is_empty());
        }
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored
    async fn test_real_api_preserves_anchor_tokens() {
        if std::env::var("GOOGLE_TRANSLATE_API_KEY").is_err() {
            eprintln!("Skipping: GOOGLE_TRANSLATE_API_KEY not set");
            return;
        }

        let provider = GoogleTranslateProvider::from_env().unwrap();
        let text = "_ID1_ sent _ID2_ message";
        let result = provider.translate(text, "en", "fr").await.unwrap();

        println!("Original: {}", text);
        println!("Translated: {}", result);

        // Anchor tokens should be preserved
        assert!(result.contains("_ID1_"));
        assert!(result.contains("_ID2_"));
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored
    async fn test_real_api_invalid_key() {
        let provider = GoogleTranslateProvider::new("invalid-key-xyz".to_string()).unwrap();
        let result = provider.translate("hello", "en", "fr").await;

        // Should fail with client error (401 Unauthorized)
        assert!(result.is_err());
        match result {
            Err(MtError::ConfigError(_)) | Err(MtError::TranslationError(_)) => {
                // Expected
            }
            _ => panic!("Expected error from invalid API key"),
        }
    }
}
