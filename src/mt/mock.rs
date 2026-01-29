//! Mock Machine Translator for testing
//!
//! This module provides a deterministic, API-free translator for testing
//! the MT pipeline without requiring API keys or network access.
//!
//! # Example
//!
//! ```ignore
//! use banana_i18n::mt::{MachineTranslator, MockTranslator, MockMode};
//!
//! #[tokio::test]
//! async fn test_translation() {
//!     let mock = MockTranslator::new(MockMode::Suffix);
//!     let result = mock.translate("hello", "en", "fr").await.unwrap();
//!     assert_eq!(result, "hello_fr");
//! }
//! ```

use crate::mt::error::MtResult;
use crate::mt::translator::MachineTranslator;
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

/// Mock translation modes for testing different scenarios
#[derive(Debug, Clone)]
pub enum MockMode {
    /// Append locale suffix: "hello" → "hello_fr"
    /// This preserves anchor tokens perfectly for testing
    Suffix,

    /// Use predefined mappings for realistic translations
    /// (text, target_locale) → translation
    Mappings(HashMap<(String, String), String>),

    /// Simulate word reordering (for testing word-order-changing languages like Japanese)
    /// Reverses the order of words separated by spaces
    Reorder,

    /// Simulate API errors
    Error(String),

    /// No-op: return input unchanged
    NoOp,
}

/// Mock translator that simulates various translation scenarios
///
/// Useful for testing the MT pipeline without external API dependencies.
/// Each mode simulates different translation behaviors.
#[derive(Debug, Clone)]
pub struct MockTranslator {
    mode: MockMode,
    /// Optional simulated network delay (in milliseconds)
    delay_ms: u64,
}

impl MockTranslator {
    /// Create a new MockTranslator with the given mode
    ///
    /// # Arguments
    ///
    /// * `mode` - The translation mode to use
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mock = MockTranslator::new(MockMode::Suffix);
    /// ```
    pub fn new(mode: MockMode) -> Self {
        Self { mode, delay_ms: 0 }
    }

    /// Create a MockTranslator with simulated network delay
    ///
    /// # Arguments
    ///
    /// * `mode` - The translation mode
    /// * `delay_ms` - Simulated delay in milliseconds
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mock = MockTranslator::with_delay(MockMode::Suffix, 50);
    /// // Each translation will have ~50ms delay
    /// ```
    pub fn with_delay(mode: MockMode, delay_ms: u64) -> Self {
        Self { mode, delay_ms }
    }

    /// Internal helper to apply the simulated delay
    async fn apply_delay(&self) {
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
    }

    /// Apply translation logic based on the mode
    fn apply_translation(&self, text: &str, _source: &str, target: &str) -> MtResult<String> {
        use crate::mt::error::MtError;

        match &self.mode {
            MockMode::Suffix => {
                // Simple suffix appending
                Ok(format!("{}_{}", text, target))
            }
            MockMode::Mappings(map) => {
                // Look up in predefined mappings
                let key = (text.to_string(), target.to_string());
                Ok(map
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| format!("{}_{}", text, target)))
            }
            MockMode::Reorder => {
                // Reverse word order (simulates SOV languages)
                let words: Vec<&str> = text.split_whitespace().collect();
                let reversed = words.iter().rev().map(|&w| w).collect::<Vec<_>>().join(" ");
                Ok(reversed)
            }
            MockMode::Error(msg) => Err(MtError::TranslationError(msg.clone())),
            MockMode::NoOp => Ok(text.to_string()),
        }
    }
}

#[async_trait]
impl MachineTranslator for MockTranslator {
    async fn translate(
        &self,
        text: &str,
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<String> {
        // Apply simulated delay
        self.apply_delay().await;

        // Apply translation
        self.apply_translation(text, source_locale, target_locale)
    }

    async fn translate_batch(
        &self,
        texts: &[String],
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<Vec<String>> {
        // Apply simulated delay (per batch, not per string)
        self.apply_delay().await;

        // Translate each text
        let mut results = Vec::new();
        for text in texts {
            let translation = self.apply_translation(text, source_locale, target_locale)?;
            results.push(translation);
        }
        Ok(results)
    }

    fn provider_name(&self) -> &str {
        "Mock Translator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Suffix Mode Tests ==========

    #[tokio::test]
    async fn test_suffix_single_translation() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let result = mock.translate("hello", "en", "fr").await.unwrap();
        assert_eq!(result, "hello_fr");
    }

    #[tokio::test]
    async fn test_suffix_batch_translation() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let texts = vec!["hello".to_string(), "world".to_string()];
        let results = mock.translate_batch(&texts, "en", "fr").await.unwrap();
        assert_eq!(results, vec!["hello_fr", "world_fr"]);
    }

    #[tokio::test]
    async fn test_suffix_preserves_anchor_tokens() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let text = "777001 sent 777002 message";
        let result = mock.translate(text, "en", "fr").await.unwrap();
        assert_eq!(result, "777001 sent 777002 message_fr");
        // Verify anchor tokens are still intact
        assert!(result.contains("777001"));
        assert!(result.contains("777002"));
    }

    #[tokio::test]
    async fn test_suffix_different_targets() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let text = "hello";
        assert_eq!(mock.translate(text, "en", "fr").await.unwrap(), "hello_fr");
        assert_eq!(mock.translate(text, "en", "ru").await.unwrap(), "hello_ru");
        assert_eq!(mock.translate(text, "en", "de").await.unwrap(), "hello_de");
    }

    #[tokio::test]
    async fn test_suffix_empty_text() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let result = mock.translate("", "en", "fr").await.unwrap();
        assert_eq!(result, "_fr");
    }

    // ========== Mapping Mode Tests ==========

    #[tokio::test]
    async fn test_mapping_single_translation() {
        let mut map = HashMap::new();
        map.insert(
            ("hello".to_string(), "fr".to_string()),
            "bonjour".to_string(),
        );

        let mock = MockTranslator::new(MockMode::Mappings(map));
        let result = mock.translate("hello", "en", "fr").await.unwrap();
        assert_eq!(result, "bonjour");
    }

    #[tokio::test]
    async fn test_mapping_fallback_to_suffix() {
        let map = HashMap::new();
        let mock = MockTranslator::new(MockMode::Mappings(map));

        // Unknown mapping should fall back to suffix mode
        let result = mock.translate("unknown", "en", "fr").await.unwrap();
        assert_eq!(result, "unknown_fr");
    }

    #[tokio::test]
    async fn test_mapping_batch_translation() {
        let mut map = HashMap::new();
        map.insert(
            ("hello".to_string(), "fr".to_string()),
            "bonjour".to_string(),
        );
        map.insert(
            ("goodbye".to_string(), "fr".to_string()),
            "au revoir".to_string(),
        );

        let mock = MockTranslator::new(MockMode::Mappings(map));
        let texts = vec!["hello".to_string(), "goodbye".to_string()];
        let results = mock.translate_batch(&texts, "en", "fr").await.unwrap();
        assert_eq!(results, vec!["bonjour", "au revoir"]);
    }

    // ========== Reorder Mode Tests ==========

    #[tokio::test]
    async fn test_reorder_simple_reversal() {
        let mock = MockTranslator::new(MockMode::Reorder);
        let result = mock.translate("hello world", "en", "ja").await.unwrap();
        assert_eq!(result, "world hello");
    }

    #[tokio::test]
    async fn test_reorder_multiple_words() {
        let mock = MockTranslator::new(MockMode::Reorder);
        let result = mock
            .translate("one two three four", "en", "ja")
            .await
            .unwrap();
        assert_eq!(result, "four three two one");
    }

    #[tokio::test]
    async fn test_reorder_single_word_unchanged() {
        let mock = MockTranslator::new(MockMode::Reorder);
        let result = mock.translate("hello", "en", "ja").await.unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_reorder_preserves_anchor_tokens() {
        let mock = MockTranslator::new(MockMode::Reorder);
        let text = "777001 sent 777002";
        let result = mock.translate(text, "en", "ja").await.unwrap();
        assert_eq!(result, "777002 sent 777001");
        assert!(result.contains("777001"));
        assert!(result.contains("777002"));
    }

    // ========== Error Mode Tests ==========

    #[tokio::test]
    async fn test_error_mode_returns_error() {
        let mock = MockTranslator::new(MockMode::Error("API unavailable".to_string()));
        let result = mock.translate("hello", "en", "fr").await;
        assert!(result.is_err());
        match result {
            Err(crate::mt::error::MtError::TranslationError(msg)) => {
                assert_eq!(msg, "API unavailable");
            }
            _ => panic!("Expected TranslationError"),
        }
    }

    #[tokio::test]
    async fn test_error_mode_batch_fails() {
        let mock = MockTranslator::new(MockMode::Error("Network error".to_string()));
        let texts = vec!["hello".to_string()];
        let result = mock.translate_batch(&texts, "en", "fr").await;
        assert!(result.is_err());
    }

    // ========== NoOp Mode Tests ==========

    #[tokio::test]
    async fn test_noop_returns_unchanged() {
        let mock = MockTranslator::new(MockMode::NoOp);
        let text = "Hello world";
        let result = mock.translate(text, "en", "fr").await.unwrap();
        assert_eq!(result, text);
    }

    #[tokio::test]
    async fn test_noop_batch_returns_unchanged() {
        let mock = MockTranslator::new(MockMode::NoOp);
        let texts = vec!["hello".to_string(), "world".to_string()];
        let results = mock.translate_batch(&texts, "en", "fr").await.unwrap();
        assert_eq!(results, texts);
    }

    // ========== Delay Tests ==========

    #[tokio::test]
    async fn test_delay_adds_latency() {
        let mock = MockTranslator::with_delay(MockMode::Suffix, 50);
        let start = std::time::Instant::now();
        let _ = mock.translate("hello", "en", "fr").await.unwrap();
        let elapsed = start.elapsed();

        // Should have at least 50ms delay
        assert!(elapsed.as_millis() >= 50);
    }

    #[tokio::test]
    async fn test_no_delay_by_default() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let start = std::time::Instant::now();
        let _ = mock.translate("hello", "en", "fr").await.unwrap();
        let elapsed = start.elapsed();

        // Should be fast (< 10ms)
        assert!(elapsed.as_millis() < 10);
    }

    // ========== Provider Name Test ==========

    #[test]
    fn test_provider_name() {
        let mock = MockTranslator::new(MockMode::Suffix);
        assert_eq!(mock.provider_name(), "Mock Translator");
    }

    // ========== Batch Consistency Tests ==========

    #[tokio::test]
    async fn test_batch_preserves_order() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let texts = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
        let results = mock.translate_batch(&texts, "en", "fr").await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0], "first_fr");
        assert_eq!(results[1], "second_fr");
        assert_eq!(results[2], "third_fr");
    }

    #[tokio::test]
    async fn test_batch_handles_empty_input() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let texts: Vec<String> = vec![];
        let results = mock.translate_batch(&texts, "en", "fr").await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_single_item() {
        let mock = MockTranslator::new(MockMode::Suffix);
        let texts = vec!["single".to_string()];
        let results = mock.translate_batch(&texts, "en", "fr").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "single_fr");
    }
}
