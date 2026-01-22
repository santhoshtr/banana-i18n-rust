use banana_i18n::{I18n, LocalizedMessages};

fn main() {
    // Example 1: English
    println!("=== English Examples ===");
    let mut en_messages: LocalizedMessages = LocalizedMessages::new();
    en_messages.with_message("greeting", "Hello, $1!");
    en_messages.with_message("farewell", "Goodbye, $1!");
    en_messages.with_message(
        "plural",
        "There {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}} in the box",
    );
    en_messages.with_message(
        "plural_with_link",
        "There {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}} in the [[box]]",
    );

    let mut i18n = I18n::new();
    let i18n = i18n
        .with_locale("en")
        .with_messages_for_locale("en", en_messages);

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
    println!(
        "Localized: {}",
        i18n.localize("en", "plural_with_link", &vec!["1".to_string()])
    );

    // Example 2: Russian - Demonstrate multi-form plurals
    println!("\n=== Russian Examples (3 plural forms) ===");
    let mut ru_messages: LocalizedMessages = LocalizedMessages::new();
    ru_messages.with_message(
        "items",
        "В коробке {{PLURAL:$1|находится|находятся|находится}} $1 {{PLURAL:$1|предмет|предмета|предметов}}",
    );

    let mut i18n_ru = I18n::new();
    let i18n_ru = i18n_ru
        .with_locale("ru")
        .with_messages_for_locale("ru", ru_messages);

    println!(
        "Russian (1): {}",
        i18n_ru.localize("ru", "items", &vec!["1".to_string()])
    );
    println!(
        "Russian (2): {}",
        i18n_ru.localize("ru", "items", &vec!["2".to_string()])
    );
    println!(
        "Russian (5): {}",
        i18n_ru.localize("ru", "items", &vec!["5".to_string()])
    );
    println!(
        "Russian (21): {}",
        i18n_ru.localize("ru", "items", &vec!["21".to_string()])
    );

    // Example 3: French - Similar to English but with different message
    println!("\n=== French Examples ===");
    let mut fr_messages: LocalizedMessages = LocalizedMessages::new();
    fr_messages.with_message(
        "articles",
        "Il y a $1 {{PLURAL:$1|article|articles}} dans la boîte",
    );

    let mut i18n_fr = I18n::new();
    let i18n_fr = i18n_fr
        .with_locale("fr")
        .with_messages_for_locale("fr", fr_messages);

    println!(
        "French (1): {}",
        i18n_fr.localize("fr", "articles", &vec!["1".to_string()])
    );
    println!(
        "French (5): {}",
        i18n_fr.localize("fr", "articles", &vec!["5".to_string()])
    );
}
