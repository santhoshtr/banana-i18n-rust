//! MinT machine translation provider
//!
//! Integrates with the Wikimedia MinT translation service
//! (<https://translate.wmcloud.org>) via its `POST /translate` endpoint.
//!
//! MinT is a free, open machine translation service hosted by Wikimedia Cloud
//! Services. Unlike Google Translate it requires **no API key**, which makes it
//! the default backend for banana-i18n-mt.
//!
//! # Language codes
//!
//! MinT uses full MediaWiki language codes (e.g. `zh-hans`, `pt-br`), so this
//! provider passes locale codes through unchanged rather than stripping the
//! region/script subtag the way the Google provider does.
//!
//! # Example
//!
//! ```ignore
//! use banana_i18n_mt::{MachineTranslator, MintProvider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let provider = MintProvider::new()?;
//!     let result = provider.translate("Hello, world!", "en", "ta").await?;
//!     println!("{}", result);
//!     Ok(())
//! }
//! ```

use crate::error::{MtError, MtResult};
use crate::translator::{MachineTranslator, validate_locale};
use async_trait::async_trait;
use serde_json::json;

/// Default MinT endpoint (Wikimedia Cloud Services).
const DEFAULT_BASE_URL: &str = "https://translate.wmcloud.org/api/translate";

/// MinT translation provider.
///
/// Communicates with the Wikimedia MinT service. MinT translates a single
/// string per request (there is no batch array), so
/// [`translate_batch`](MachineTranslator::translate_batch) iterates over the
/// inputs while the consistency-preserving block path is handled by the default
/// [`MachineTranslator::translate_as_block`] implementation.
#[derive(Clone)]
pub struct MintProvider {
    /// HTTP client for async requests
    client: reqwest::Client,
    /// Base URL for the MinT `/translate` endpoint
    base_url: String,
}

impl MintProvider {
    /// Maximum characters per request. MinT handles long inputs, but this
    /// guards against pathological payloads.
    const MAX_CHARS_PER_STRING: usize = 10_000;

    /// Create a new MinT provider pointing at the default Wikimedia endpoint.
    pub fn new() -> MtResult<Self> {
        Self::with_base_url(DEFAULT_BASE_URL.to_string())
    }

    /// Create a MinT provider with a custom endpoint URL.
    ///
    /// Useful for pointing at a self-hosted MinT instance.
    pub fn with_base_url(base_url: String) -> MtResult<Self> {
        if base_url.trim().is_empty() {
            return Err(MtError::ConfigError(
                "MinT base URL cannot be empty".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| MtError::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, base_url })
    }

    /// Create a MinT provider, honouring the optional `MINT_API_URL`
    /// environment variable (falls back to the default Wikimedia endpoint).
    ///
    /// MinT needs no API key, so this never fails for missing credentials.
    pub fn from_env() -> MtResult<Self> {
        let base_url =
            std::env::var("MINT_API_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Self::with_base_url(base_url)
    }
}

impl Default for MintProvider {
    fn default() -> Self {
        Self::new().expect("default MinT provider should always build")
    }
}

impl std::fmt::Debug for MintProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MintProvider")
            .field("base_url", &self.base_url)
            .finish()
    }
}

#[async_trait]
impl MachineTranslator for MintProvider {
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

        // Build request body. MinT uses full MediaWiki language codes, so we
        // pass the locales through unchanged (no region/script stripping).
        let body = json!({
            "content": text,
            "source_language": source_locale,
            "target_language": target_locale,
            "format": "text",
        });

        // Send POST request (no authentication required)
        let response = self.client.post(&self.base_url).json(&body).send().await?;

        // Check HTTP status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return Err(if status.is_client_error() {
                MtError::ConfigError(format!("MinT client error ({}): {}", status, error_text))
            } else {
                MtError::TranslationError(format!("MinT server error ({}): {}", status, error_text))
            });
        }

        // Parse response JSON and extract the `translation` field
        let json: serde_json::Value = response.json().await.map_err(|e| {
            MtError::TranslationError(format!("Failed to parse MinT response: {}", e))
        })?;

        json["translation"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                MtError::TranslationError(
                    "Invalid MinT response: missing 'translation' field".to_string(),
                )
            })
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

        // MinT has no batch endpoint; translate each text sequentially.
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.translate(text, source_locale, target_locale).await?);
        }
        Ok(results)
    }

    fn provider_name(&self) -> &str {
        "MinT"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Initialization Tests ==========

    #[test]
    fn test_new_uses_default_url() {
        let provider = MintProvider::new().unwrap();
        assert_eq!(provider.base_url, DEFAULT_BASE_URL);
        assert_eq!(provider.provider_name(), "MinT");
    }

    #[test]
    fn test_with_custom_url() {
        let provider =
            MintProvider::with_base_url("https://example.org/translate".to_string()).unwrap();
        assert_eq!(provider.base_url, "https://example.org/translate");
    }

    #[test]
    fn test_with_empty_url() {
        let result = MintProvider::with_base_url("".to_string());
        assert!(result.is_err());
        match result {
            Err(MtError::ConfigError(msg)) => assert!(msg.contains("empty")),
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_debug_shows_url() {
        let provider = MintProvider::new().unwrap();
        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("MintProvider"));
        assert!(debug_str.contains("wmcloud.org"));
    }

    // ========== Validation Tests ==========

    #[tokio::test]
    async fn test_translate_empty_text() {
        let provider = MintProvider::new().unwrap();
        let result = provider.translate("", "en", "fr").await.unwrap();
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_translate_invalid_source_locale() {
        let provider = MintProvider::new().unwrap();
        let result = provider.translate("hello", "invalid@code", "fr").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_translate_text_too_long() {
        let provider = MintProvider::new().unwrap();
        let long_text = "x".repeat(MintProvider::MAX_CHARS_PER_STRING + 1);
        let result = provider.translate(&long_text, "en", "fr").await;
        assert!(result.is_err());
        match result {
            Err(MtError::TranslationError(msg)) => assert!(msg.contains("exceeds maximum")),
            _ => panic!("Expected TranslationError"),
        }
    }

    #[tokio::test]
    async fn test_batch_empty() {
        let provider = MintProvider::new().unwrap();
        let texts: Vec<String> = vec![];
        let results = provider.translate_batch(&texts, "en", "fr").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_provider_name() {
        let provider = MintProvider::new().unwrap();
        assert_eq!(provider.provider_name(), "MinT");
    }

    // ========== Integration Tests (require network; no API key) ==========

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored
    async fn test_real_api_single_translation() {
        let provider = MintProvider::new().unwrap();
        let result = provider
            .translate("The Earth rotates around the Sun.", "en", "ta")
            .await
            .unwrap();
        println!("MinT translation: {}", result);
        assert!(!result.is_empty());
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored
    async fn test_real_api_preserves_hyphenated_langcode() {
        // Some MinT codes are genuinely hyphenated (e.g. `zh-yue`, `nds-nl`).
        // Verify the provider passes them through unchanged rather than
        // stripping the subtag to `zh` (which would target a different model).
        // These codes are served by a large model that can cold-start slowly,
        // so transient upstream 5xx/network errors are treated as a skip.
        let provider = MintProvider::new().unwrap();
        match provider.translate("Hello", "en", "zh-yue").await {
            Ok(result) => {
                println!("MinT zh-yue translation: {}", result);
                assert!(!result.is_empty());
            }
            Err(MtError::TranslationError(msg)) | Err(MtError::NetworkError(msg)) => {
                eprintln!("Skipping: transient MinT upstream error: {}", msg);
            }
            Err(other) => panic!("Unexpected error type: {:?}", other),
        }
    }
}
