use banana_i18n::{I18n, LocalizedMessages};

fn main() {
    // Example usage: Plural
    // "There {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}} in the box
    let mut en_messages: LocalizedMessages = LocalizedMessages::new();
    en_messages.with_message("greeting", "Hello, $1!");
    en_messages.with_message("farewell", "Goodbye, $1!");
    en_messages.with_message(
        "plural",
        "There {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}} in the box",
    );

    let mut i18n = I18n::new();
    let i18n = i18n.with_messages_for_locale("en", en_messages);

    println!(
        "Localized: {}",
        i18n.localize("en", "greeting", &vec!["World".to_string()])
    );
    println!(
        "Localized: {}",
        i18n.localize("en", "farewell", &vec!["World".to_string()])
    );
    println!(
        "Localized: {}",
        i18n.localize("en", "plural", &vec!["2".to_string()])
    );
    println!(
        "Localized: {}",
        i18n.localize("en", "plural", &vec!["1".to_string()])
    );
}
