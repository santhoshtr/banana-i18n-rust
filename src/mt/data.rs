//! Core data structures for MT-assisted localization
//!
//! This module defines the fundamental data types used throughout the MT pipeline,
//! closely matching the Python reference implementation design for simplicity.

use std::collections::HashMap;

/// Represents a single permutation of the message with a specific state
///
/// Each variant corresponds to one specific combination of choices for all
/// magic words in the message. For example, with GENDER($1) and PLURAL($2),
/// there would be 3×2 = 6 variants total.
///
/// # Example
///
/// For message `"{{GENDER:$1|He|She}} sent {{PLURAL:$2|a message|$2 messages}}"`:
///
/// ```ignore
/// TranslationVariant {
///     state: {
///         "$1": 0,  // First choice (He)
///         "$2": 1   // Second choice ($2 messages)
///     },
///     source_text: "_ID1_ sent _ID2_ messages",
///     translated_text: "_ID1_ a envoyé _ID2_ messages"
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TranslationVariant {
    /// State maps the variable ID to the choice index
    /// Example: {"$1": 0, "$2": 1} means first choice for $1, second choice for $2
    pub state: HashMap<String, usize>,

    /// The source string with anchors (e.g., "_ID1_ sent a message.")
    /// Anchors protect placeholders from being translated by MT systems
    pub source_text: String,

    /// The translated string returned by MT (initially empty)
    /// Will be populated during translation phase
    pub translated_text: String,
}

impl TranslationVariant {
    /// Create a new translation variant with the given state and source text
    pub fn new(state: HashMap<String, usize>, source_text: String) -> Self {
        Self {
            state,
            source_text,
            translated_text: String::new(),
        }
    }

    /// Create a variant with translated text
    pub fn with_translation(
        state: HashMap<String, usize>,
        source_text: String,
        translated_text: String,
    ) -> Self {
        Self {
            state,
            source_text,
            translated_text,
        }
    }

    /// Check if this variant has been translated (translated_text is not empty)
    pub fn is_translated(&self) -> bool {
        !self.translated_text.is_empty()
    }
}

/// Holds all variations and metadata needed to rebuild the wikitext
///
/// This structure contains all the information needed to reconstruct the
/// original wikitext structure after translation, including variable types
/// and the complete set of variants.
///
/// # Example
///
/// ```ignore
/// MessageContext {
///     original_key: "user-message",
///     variable_types: {
///         "$1": "GENDER",
///         "$2": "PLURAL"
///     },
///     variants: [
///         TranslationVariant { state: {"$1": 0, "$2": 0}, ... },
///         TranslationVariant { state: {"$1": 0, "$2": 1}, ... },
///         TranslationVariant { state: {"$1": 1, "$2": 0}, ... },
///         TranslationVariant { state: {"$1": 1, "$2": 1}, ... },
///     ]
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MessageContext {
    /// Original message key for reference
    pub original_key: String,

    /// Maps variable IDs to their magic word type for reassembly
    /// Example: {"$1": "GENDER", "$2": "PLURAL"}
    pub variable_types: HashMap<String, String>,

    /// The list of all variants (cartesian product of all choices)
    pub variants: Vec<TranslationVariant>,
}

impl MessageContext {
    /// Create a new empty message context
    pub fn new(original_key: String) -> Self {
        Self {
            original_key,
            variable_types: HashMap::new(),
            variants: Vec::new(),
        }
    }

    /// Add a variable type mapping for reassembly
    ///
    /// # Arguments
    /// * `var_id` - Variable identifier (e.g., "$1", "$2")
    /// * `var_type` - Magic word type ("PLURAL", "GENDER")
    pub fn add_variable(&mut self, var_id: String, var_type: String) {
        self.variable_types.insert(var_id, var_type);
    }

    /// Add a variant to this context
    pub fn add_variant(&mut self, variant: TranslationVariant) {
        self.variants.push(variant);
    }

    /// Get the number of variants
    pub fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// Check if all variants have been translated
    pub fn is_fully_translated(&self) -> bool {
        !self.variants.is_empty() && self.variants.iter().all(|v| v.is_translated())
    }

    /// Get all source texts as a vector (useful for batch translation)
    pub fn source_texts(&self) -> Vec<String> {
        self.variants
            .iter()
            .map(|v| v.source_text.clone())
            .collect()
    }

    /// Update all variants with translated texts
    ///
    /// # Arguments
    /// * `translated_texts` - Translated texts in same order as variants
    ///
    /// # Panics
    /// Panics if the length doesn't match the number of variants
    pub fn update_translations(&mut self, translated_texts: Vec<String>) {
        assert_eq!(
            translated_texts.len(),
            self.variants.len(),
            "Translation count must match variant count"
        );

        for (variant, translated) in self.variants.iter_mut().zip(translated_texts.into_iter()) {
            variant.translated_text = translated;
        }
    }

    /// Get variables used in this message context
    pub fn variable_ids(&self) -> Vec<String> {
        self.variable_types.keys().cloned().collect()
    }

    /// Get the magic word type for a variable
    pub fn get_variable_type(&self, var_id: &str) -> Option<&String> {
        self.variable_types.get(var_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translation_variant_creation() {
        let mut state = HashMap::new();
        state.insert("$1".to_string(), 0);
        state.insert("$2".to_string(), 1);

        let variant = TranslationVariant::new(state.clone(), "Hello _ID1_!".to_string());

        assert_eq!(variant.state, state);
        assert_eq!(variant.source_text, "Hello _ID1_!");
        assert_eq!(variant.translated_text, "");
        assert!(!variant.is_translated());
    }

    #[test]
    fn test_translation_variant_with_translation() {
        let mut state = HashMap::new();
        state.insert("$1".to_string(), 0);

        let variant = TranslationVariant::with_translation(
            state.clone(),
            "Hello _ID1_!".to_string(),
            "Bonjour _ID1_!".to_string(),
        );

        assert_eq!(variant.state, state);
        assert_eq!(variant.source_text, "Hello _ID1_!");
        assert_eq!(variant.translated_text, "Bonjour _ID1_!");
        assert!(variant.is_translated());
    }

    #[test]
    fn test_message_context_creation() {
        let context = MessageContext::new("test-message".to_string());

        assert_eq!(context.original_key, "test-message");
        assert!(context.variable_types.is_empty());
        assert!(context.variants.is_empty());
        assert_eq!(context.variant_count(), 0);
        assert!(!context.is_fully_translated());
    }

    #[test]
    fn test_message_context_add_variable() {
        let mut context = MessageContext::new("test".to_string());

        context.add_variable("$1".to_string(), "GENDER".to_string());
        context.add_variable("$2".to_string(), "PLURAL".to_string());

        assert_eq!(context.variable_types.len(), 2);
        assert_eq!(context.get_variable_type("$1"), Some(&"GENDER".to_string()));
        assert_eq!(context.get_variable_type("$2"), Some(&"PLURAL".to_string()));
        assert_eq!(context.get_variable_type("$3"), None);
    }

    #[test]
    fn test_message_context_variants() {
        let mut context = MessageContext::new("test".to_string());

        let mut state1 = HashMap::new();
        state1.insert("$1".to_string(), 0);
        let variant1 = TranslationVariant::new(state1, "He sent".to_string());

        let mut state2 = HashMap::new();
        state2.insert("$1".to_string(), 1);
        let variant2 = TranslationVariant::new(state2, "She sent".to_string());

        context.add_variant(variant1);
        context.add_variant(variant2);

        assert_eq!(context.variant_count(), 2);
        assert!(!context.is_fully_translated());

        let source_texts = context.source_texts();
        assert_eq!(source_texts, vec!["He sent", "She sent"]);
    }

    #[test]
    fn test_message_context_update_translations() {
        let mut context = MessageContext::new("test".to_string());

        let mut state = HashMap::new();
        state.insert("$1".to_string(), 0);
        let variant = TranslationVariant::new(state, "Hello".to_string());

        context.add_variant(variant);

        let translations = vec!["Bonjour".to_string()];
        context.update_translations(translations);

        assert!(context.is_fully_translated());
        assert_eq!(context.variants[0].translated_text, "Bonjour");
    }

    #[test]
    #[should_panic(expected = "Translation count must match variant count")]
    fn test_update_translations_count_mismatch() {
        let mut context = MessageContext::new("test".to_string());

        let mut state = HashMap::new();
        state.insert("$1".to_string(), 0);
        let variant = TranslationVariant::new(state, "Hello".to_string());
        context.add_variant(variant);

        // Wrong count - should panic
        let translations = vec!["Bonjour".to_string(), "Hola".to_string()];
        context.update_translations(translations);
    }

    #[test]
    fn test_variable_ids() {
        let mut context = MessageContext::new("test".to_string());
        context.add_variable("$2".to_string(), "PLURAL".to_string());
        context.add_variable("$1".to_string(), "GENDER".to_string());

        let mut var_ids = context.variable_ids();
        var_ids.sort(); // HashMap iteration order is not guaranteed

        assert_eq!(var_ids, vec!["$1", "$2"]);
    }

    #[test]
    fn test_empty_context_source_texts() {
        let context = MessageContext::new("test".to_string());
        assert_eq!(context.source_texts(), Vec::<String>::new());
    }

    #[test]
    fn test_translation_variant_equality() {
        let mut state = HashMap::new();
        state.insert("$1".to_string(), 0);

        let variant1 = TranslationVariant::new(state.clone(), "Hello".to_string());
        let variant2 = TranslationVariant::new(state.clone(), "Hello".to_string());
        let variant3 = TranslationVariant::new(state, "Goodbye".to_string());

        assert_eq!(variant1, variant2);
        assert_ne!(variant1, variant3);
    }
}
