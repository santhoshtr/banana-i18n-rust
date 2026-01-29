use banana_i18n::{I18n, VerbosityLevel, load_all_messages_from_dir};
use std::env;
use std::path::PathBuf;

fn main() {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    // Get messages directory from environment variable or use default
    let messages_dir = env::var("I18N_MESSAGES_DIR").unwrap_or_else(|_| "i18n".to_string());
    let messages_dir = PathBuf::from(messages_dir);

    // Parse positional arguments: <locale> <message-key> [params...]
    if args.len() < 3 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    let locale = args[1].clone();
    let message_key = args[2].clone();
    let params: Vec<String> = args[3..].to_vec();

    // Load all messages from the messages directory
    let messages_map = match load_all_messages_from_dir(&messages_dir) {
        Ok(map) => {
            if map.is_empty() {
                eprintln!(
                    "Error: No message files found in '{}'",
                    messages_dir.display()
                );
                std::process::exit(1);
            }
            map
        }
        Err(e) => {
            eprintln!("Error loading messages: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize I18n with all loaded locales
    let mut i18n = I18n::new();

    // Set default locale to English if available, otherwise first available locale
    let default_locale = if messages_map.contains_key("en") {
        "en"
    } else {
        messages_map
            .keys()
            .next()
            .map(|s| s.as_str())
            .unwrap_or("en")
    };

    i18n.with_locale(default_locale)
        .with_verbosity(VerbosityLevel::Silent);

    // Add all loaded message locales to i18n
    for (loc, messages) in messages_map {
        i18n.with_messages_for_locale(&loc, messages);
    }

    // Localize and print the result
    let result = i18n.localize(&locale, &message_key, &params);
    println!("{}", result);
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <locale> <message-key> [params...]", program);
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} en greeting \"World\"", program);
    eprintln!("  {} ru items \"5\"", program);
    eprintln!("  {} en plural \"2\"", program);
    eprintln!("  {} en pronoun male", program);
    eprintln!();
    eprintln!("Environment Variables:");
    eprintln!("  I18N_MESSAGES_DIR  Directory containing JSON message files (default: i18n/)");
    eprintln!();
    eprintln!("Message files should be in JSON format with locale code as filename:");
    eprintln!("  en.json, ru.json, fr.json, de.json, zh-hans.json, etc.");
}
