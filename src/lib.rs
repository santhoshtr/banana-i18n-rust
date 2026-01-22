use std::collections::HashMap;

pub mod ast;
pub mod parser;

// Re-export AST types for convenient access
pub use ast::{
    AstNode, AstNodeList, Localizable, Placeholder, Transclusion, WikiExternalLink,
    WikiInternalLink,
};
pub use parser::Parser;

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
}

impl I18n {
    pub fn new() -> Self {
        I18n {
            messages: HashMap::new(),
            default_locale: "en".to_string(),
        }
    }

    pub fn with_locale(&mut self, locale: &str) -> &mut Self {
        self.default_locale = locale.to_lowercase();
        self
    }

    pub fn get_default_locale(&self) -> &str {
        &self.default_locale
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
        if let Some(messages) = self.messages.get(locale) {
            return messages
                .get_message(key)
                .unwrap_or(&key.to_string())
                .to_string();
        }
        key.to_string()
    }

    pub fn localize(&self, locale: &str, key: &str, values: &Vec<String>) -> String {
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
                    result.push_str(&transclusion.localize(locale, values).as_str());
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
}
