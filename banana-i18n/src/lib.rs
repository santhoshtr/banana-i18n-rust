use std::collections::HashMap;

pub mod ast;
pub mod fallbacks;
pub mod loader;
pub mod parser;

// Re-export AST types for convenient access
pub use ast::{
    AstNode, AstNodeList, Localizable, Placeholder, Transclusion, WikiExternalLink,
    WikiInternalLink,
};
pub use fallbacks::get_fallbacks;
pub use loader::{load_all_messages_from_dir, load_messages_from_file};
pub use parser::Parser;

/// Verbosity level for debug logging during fallback resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerbosityLevel {
    /// No debug logging
    Silent = 0,
    /// Log only when fallbacks are used (default)
    Normal = 1,
    /// Log detailed information about fallback resolution
    Verbose = 2,
}

pub struct LocalizedMessages(pub HashMap<String, String>);
impl LocalizedMessages {
    pub fn new() -> Self {
        LocalizedMessages(HashMap::new())
    }
    pub fn with_message(&mut self, key: &str, message: &str) -> &mut Self {
        self.0.insert(key.to_owned(), message.to_owned());
        self
    }
    pub fn get_message(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }
    pub fn get_messages(&self) -> &HashMap<String, String> {
        &self.0
    }
    pub fn get_messages_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.0
    }
    pub fn get(&self, key: &str) -> String {
        self.0.get(key).unwrap_or(&key.to_string()).to_string()
    }
    pub fn get_or_default(&self, key: &str, default: &str) -> String {
        self.0.get(key).unwrap_or(&default.to_string()).to_string()
    }
}

pub struct I18n {
    // Keyed by locale and then by message key
    // e.g. messages["en"]["greeting"] = "Hello"
    //      messages["fr"]["greeting"] = "Bonjour"
    //      messages["es"]["greeting"] = "Hola"
    //      messages["de"]["greeting"] = "Hallo"
    //      messages["it"]["greeting"] = "Ciao"
    messages: HashMap<String, LocalizedMessages>,
    default_locale: String,
    verbosity: VerbosityLevel,
}

impl I18n {
    pub fn new() -> Self {
        I18n {
            messages: HashMap::new(),
            default_locale: "en".to_string(),
            verbosity: VerbosityLevel::Normal,
        }
    }

    pub fn with_locale(&mut self, locale: &str) -> &mut Self {
        self.default_locale = locale.to_lowercase();
        self
    }

    pub fn get_default_locale(&self) -> &str {
        &self.default_locale
    }

    pub fn with_verbosity(&mut self, verbosity: VerbosityLevel) -> &mut Self {
        self.verbosity = verbosity;
        self
    }

    pub fn get_verbosity(&self) -> VerbosityLevel {
        self.verbosity
    }
    pub fn with_messages_for_locale(
        &mut self,
        locale: &str,
        messages: LocalizedMessages,
    ) -> &mut Self {
        self.messages.insert(locale.to_lowercase(), messages);
        self
    }

    pub fn add_message(&mut self, locale: &str, key: String, message: Vec<String>) {
        let messages: &mut LocalizedMessages = self
            .messages
            .entry(locale.to_string())
            .or_insert_with(LocalizedMessages::new);
        for msg in message {
            messages.with_message(&key, &msg);
        }
    }

    pub fn get_message(&self, locale: &str, key: &str) -> String {
        // Try to get message from requested locale first
        if let Some(messages) = self.messages.get(locale) {
            if let Some(message) = messages.get_message(key) {
                return message.clone();
            }
        }

        // If not found, follow the fallback chain
        let fallback_chain = fallbacks::resolve_locale_chain(locale);

        // Skip the first one since we already tried it
        for fallback_locale in fallback_chain.iter().skip(1) {
            if let Some(messages) = self.messages.get(fallback_locale) {
                if let Some(message) = messages.get_message(key) {
                    if self.verbosity >= VerbosityLevel::Normal {
                        eprintln!(
                            "[i18n] Fallback: Using message '{}' from locale '{}' (requested: '{}')",
                            key, fallback_locale, locale
                        );
                    }
                    if self.verbosity >= VerbosityLevel::Verbose {
                        eprintln!("[i18n] Fallback chain: {}", fallback_chain.join(" -> "));
                    }
                    return message.clone();
                }
            }
        }

        // No message found in any fallback locale, return the key
        if self.verbosity >= VerbosityLevel::Verbose {
            eprintln!(
                "[i18n] No message found for '{}' in locale '{}' or its fallbacks: {}",
                key,
                locale,
                fallback_chain.join(" -> ")
            );
        }
        key.to_string()
    }

    pub fn localize(&self, locale: &str, key: &str, values: &Vec<String>) -> String {
        self.localize_internal(locale, key, values, true)
    }

    fn localize_internal(
        &self,
        locale: &str,
        key: &str,
        values: &Vec<String>,
        _log_fallback: bool,
    ) -> String {
        let message = self.get_message(locale, key);
        let mut parser = parser::Parser::new(&message);
        let ast: AstNodeList = parser.parse();
        let mut result = String::new();

        for node in ast {
            match node {
                AstNode::Text(text) => result.push_str(&text),
                AstNode::Placeholder(placeholder) => {
                    result.push_str(&placeholder.localize(locale, values).as_str());
                }
                AstNode::Transclusion(transclusion) => {
                    // For transclusions, pass verbosity via context
                    result.push_str(
                        transclusion
                            .localize_with_context(locale, values, self.verbosity)
                            .as_str(),
                    );
                }
                AstNode::InternalLink(link) => {
                    result.push_str(&link.to_string());
                }
                AstNode::ExternalLink(link) => {
                    result.push_str(&link.to_string());
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localization() {
        let mut en_messages: LocalizedMessages = LocalizedMessages::new();
        en_messages.with_message("greeting", "Hello, $1!");
        en_messages.with_message("farewell", "Goodbye, $1!");
        en_messages.with_message(
            "plural",
            "There {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}} in the box",
        );

        let mut i18n = I18n::new();
        i18n.with_locale("en")
            .with_messages_for_locale("en", en_messages);

        assert_eq!(
            i18n.localize("en", "greeting", &vec!["World".to_string()]),
            "Hello, World!"
        );
        assert_eq!(
            i18n.localize("en", "farewell", &vec!["World".to_string()]),
            "Goodbye, World!"
        );
        assert_eq!(
            i18n.localize("en", "plural", &vec!["2".to_string()]),
            "There are 2 items in the box"
        );
        assert_eq!(
            i18n.localize("en", "plural", &vec!["1".to_string()]),
            "There is 1 item in the box"
        );
    }

    #[test]
    fn test_default_locale() {
        let mut i18n = I18n::new();
        assert_eq!(i18n.get_default_locale(), "en");

        i18n.with_locale("fr");
        assert_eq!(i18n.get_default_locale(), "fr");

        i18n.with_locale("ES");
        assert_eq!(i18n.get_default_locale(), "es");
    }

    #[test]
    fn test_message_fallback_simple() {
        // Test fallback from de-at to de to en
        let mut de_messages: LocalizedMessages = LocalizedMessages::new();
        de_messages.with_message("greeting", "Guten Tag, $1!");

        let mut en_messages: LocalizedMessages = LocalizedMessages::new();
        en_messages.with_message("greeting", "Hello, $1!");
        en_messages.with_message("farewell", "Goodbye, $1!");

        let mut i18n = I18n::new();
        i18n.with_locale("en")
            .with_messages_for_locale("en", en_messages)
            .with_messages_for_locale("de", de_messages)
            .with_verbosity(VerbosityLevel::Silent);

        // Message exists for de-at's fallback (de)
        assert_eq!(
            i18n.localize("de-at", "greeting", &vec!["Welt".to_string()]),
            "Guten Tag, Welt!"
        );

        // Message doesn't exist for de-at, but exists in en fallback
        assert_eq!(
            i18n.localize("de-at", "farewell", &vec!["Welt".to_string()]),
            "Goodbye, Welt!"
        );
    }

    #[test]
    fn test_message_fallback_missing_all() {
        // Test when message is missing from all locales
        let mut en_messages: LocalizedMessages = LocalizedMessages::new();
        en_messages.with_message("greeting", "Hello, $1!");

        let mut i18n = I18n::new();
        i18n.with_locale("en")
            .with_messages_for_locale("en", en_messages)
            .with_verbosity(VerbosityLevel::Silent);

        // Non-existent message should return the key
        assert_eq!(i18n.localize("en", "nonexistent", &vec![]), "nonexistent");
    }

    #[test]
    fn test_message_fallback_complex_chain() {
        // Test with a complex fallback chain like zh-cn
        let mut zh_hant_messages: LocalizedMessages = LocalizedMessages::new();
        zh_hant_messages.with_message("title", "繁體");

        let mut zh_hans_messages: LocalizedMessages = LocalizedMessages::new();
        zh_hans_messages.with_message("title", "简体");
        zh_hans_messages.with_message("greeting", "你好 $1");

        let mut en_messages: LocalizedMessages = LocalizedMessages::new();
        en_messages.with_message("greeting", "Hello $1");

        let mut i18n = I18n::new();
        i18n.with_locale("en")
            .with_messages_for_locale("en", en_messages)
            .with_messages_for_locale("zh-hans", zh_hans_messages)
            .with_messages_for_locale("zh-hant", zh_hant_messages)
            .with_verbosity(VerbosityLevel::Silent);

        // zh-cn should fall back to zh-hans
        assert_eq!(
            i18n.localize("zh-cn", "greeting", &vec!["世界".to_string()]),
            "你好 世界"
        );

        // zh-cn should eventually reach en for missing messages
        assert_eq!(i18n.get_message("zh-cn", "title"), "简体");
    }

    #[test]
    fn test_verbosity_levels() {
        // Test that verbosity level is properly set and retrieved
        let mut i18n = I18n::new();

        assert_eq!(i18n.get_verbosity(), VerbosityLevel::Normal);

        i18n.with_verbosity(VerbosityLevel::Silent);
        assert_eq!(i18n.get_verbosity(), VerbosityLevel::Silent);

        i18n.with_verbosity(VerbosityLevel::Verbose);
        assert_eq!(i18n.get_verbosity(), VerbosityLevel::Verbose);
    }
}
