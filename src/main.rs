use banana_i18n::{I18n, LocalizedMessages, VerbosityLevel};

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
        .with_messages_for_locale("en", en_messages)
        .with_verbosity(VerbosityLevel::Silent);

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
        .with_messages_for_locale("ru", ru_messages)
        .with_verbosity(VerbosityLevel::Silent);

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
        .with_messages_for_locale("fr", fr_messages)
        .with_verbosity(VerbosityLevel::Silent);

    println!(
        "French (1): {}",
        i18n_fr.localize("fr", "articles", &vec!["1".to_string()])
    );
    println!(
        "French (5): {}",
        i18n_fr.localize("fr", "articles", &vec!["5".to_string()])
    );

    // Example 4: Fallback chain demonstration - de-at to de
    println!("\n=== Fallback Chain Examples (de-at -> de -> en) ===");
    let mut de_messages: LocalizedMessages = LocalizedMessages::new();
    de_messages.with_message("greeting", "Guten Tag, $1!");
    de_messages.with_message(
        "items",
        "Es {{PLURAL:$1|ist|sind}} $1 {{PLURAL:$1|Element|Elemente}} in der Kiste",
    );

    let mut en_messages_copy: LocalizedMessages = LocalizedMessages::new();
    en_messages_copy.with_message("greeting", "Hello, $1!");
    en_messages_copy.with_message("farewell", "Goodbye, $1!");

    let mut i18n_de = I18n::new();
    let i18n_de = i18n_de
        .with_locale("en")
        .with_messages_for_locale("en", en_messages_copy)
        .with_messages_for_locale("de", de_messages)
        .with_verbosity(VerbosityLevel::Normal);

    println!("\nRequesting de-at locale (has fallbacks to de and en):");
    println!(
        "de-at greeting (fallback to de): {}",
        i18n_de.localize("de-at", "greeting", &vec!["Welt".to_string()])
    );

    println!(
        "de-at plural (fallback to de): {}",
        i18n_de.localize("de-at", "items", &vec!["1".to_string()])
    );

    println!(
        "de-at plural (fallback to de): {}",
        i18n_de.localize("de-at", "items", &vec!["5".to_string()])
    );

    // Message only in English (fallback through de)
    println!(
        "de-at farewell (fallback to en): {}",
        i18n_de.localize("de-at", "farewell", &vec!["Welt".to_string()])
    );

    // Example 5: Complex Chinese fallback chain
    println!("\n=== Complex Fallback Chain (zh-cn -> zh-hans -> zh -> zh-hant -> en) ===");
    let mut zh_hans_messages: LocalizedMessages = LocalizedMessages::new();
    zh_hans_messages.with_message("greeting", "你好，$1");
    zh_hans_messages.with_message("books", "有 {{PLURAL:$1|一|}} $1 {{PLURAL:$1|本书|本书}}");

    let mut en_messages_copy2: LocalizedMessages = LocalizedMessages::new();
    en_messages_copy2.with_message("greeting", "Hello, $1!");
    en_messages_copy2.with_message("farewell", "Goodbye, $1!");

    let mut i18n_zh = I18n::new();
    let i18n_zh = i18n_zh
        .with_locale("en")
        .with_messages_for_locale("en", en_messages_copy2)
        .with_messages_for_locale("zh-hans", zh_hans_messages)
        .with_verbosity(VerbosityLevel::Normal);

    println!("\nRequesting zh-cn locale (fallback chain active):");
    println!(
        "zh-cn greeting (via zh-hans): {}",
        i18n_zh.localize("zh-cn", "greeting", &vec!["世界".to_string()])
    );

    println!(
        "zh-cn books (1 book): {}",
        i18n_zh.localize("zh-cn", "books", &vec!["1".to_string()])
    );

    println!(
        "zh-cn books (5 books): {}",
        i18n_zh.localize("zh-cn", "books", &vec!["5".to_string()])
    );

    println!(
        "zh-cn farewell (fallback to en): {}",
        i18n_zh.localize("zh-cn", "farewell", &vec!["世界".to_string()])
    );

    // Example 6: Verbose logging to show fallback chain resolution
    println!("\n=== Verbose Logging Example ===");
    let mut en_messages_copy3: LocalizedMessages = LocalizedMessages::new();
    en_messages_copy3.with_message("greeting", "Hello, $1!");
    en_messages_copy3.with_message("farewell", "Goodbye, $1!");

    let mut i18n_verbose = I18n::new();
    let i18n_verbose = i18n_verbose
        .with_locale("en")
        .with_messages_for_locale("en", en_messages_copy3)
        .with_verbosity(VerbosityLevel::Verbose);

    println!("With Verbose logging (STDERR shows fallback info):");
    println!(
        "Result: {}",
        i18n_verbose.localize("de-at", "farewell", &vec!["Welt".to_string()])
    );
}
