use banana_i18n::parser::Parser;
use banana_i18n_mt::{
    GoogleTranslateProvider, MachineTranslator, MintProvider, MockMode, MockTranslator,
    Reassembler, prepare_for_translation,
};
use clap::{Arg, Command};
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("banana-mt")
        .version("0.1.0")
        .about("Machine Translation CLI for banana-i18n")
        .arg(
            Arg::new("message")
                .help("Source message to translate")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("target-locale")
                .help("Target language code (e.g., fr, es, de)")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::new("source-locale")
                .long("source")
                .short('s')
                .help("Source language code (default: en)")
                .default_value("en"),
        )
        .arg(
            Arg::new("backend")
                .long("backend")
                .short('b')
                .help("Translation backend: mint (default), google, or mock")
                .value_parser(["mint", "google", "mock"])
                .default_value("mint"),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Show detailed translation process")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("key")
                .long("key")
                .short('k')
                .help("Message key for context (default: auto-generated)"),
        )
        .get_matches();

    let source_message = matches.get_one::<String>("message").unwrap();
    let target_locale = matches.get_one::<String>("target-locale").unwrap();
    let source_locale = matches.get_one::<String>("source-locale").unwrap();
    let backend = matches.get_one::<String>("backend").unwrap();
    let verbose = matches.get_flag("verbose");
    let message_key = matches
        .get_one::<String>("key")
        .map(|s| s.as_str())
        .unwrap_or("cli-message");

    if verbose {
        println!("📝 Source: \"{}\"", source_message);
        println!("🌍 {} → {}", source_locale, target_locale);
        println!("🔑 Key: {}", message_key);
        println!();
    }

    // 1. Parse message
    let mut parser = Parser::new(source_message);
    let ast = parser.parse();

    if verbose {
        println!("✅ Parsed message ({} nodes)", ast.len());
    }

    // 2. Prepare for translation
    let mut context = match prepare_for_translation(&ast, source_locale, message_key) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("❌ Failed to prepare for translation: {}", e);
            return Err(e.into());
        }
    };

    if verbose {
        println!("📦 Expanded to {} variants", context.variant_count());
        println!("   Variables: {:?}", context.variable_types);

        if context.variant_count() <= 10 {
            for (i, variant) in context.variants.iter().enumerate() {
                println!("   [{}] \"{}\"", i, variant.source_text);
            }
        } else {
            for (i, variant) in context.variants.iter().take(5).enumerate() {
                println!("   [{}] \"{}\"", i, variant.source_text);
            }
            println!("   ... {} more variants", context.variant_count() - 5);
        }
        println!();
    }

    // 3. Translate
    let source_texts = context.source_texts();
    let translator: Arc<dyn MachineTranslator> = match backend.as_str() {
        "mock" => Arc::new(MockTranslator::new(MockMode::Suffix)),
        "google" => {
            if env::var("GOOGLE_TRANSLATE_API_KEY").is_err() {
                eprintln!("❌ GOOGLE_TRANSLATE_API_KEY environment variable not set");
                eprintln!("   Set it with: export GOOGLE_TRANSLATE_API_KEY=your_api_key");
                eprintln!("   Or use --backend mint (no API key required)");
                return Err("Missing API key".into());
            }
            Arc::new(GoogleTranslateProvider::from_env()?)
        }
        // "mint" (default) and any future backends
        _ => Arc::new(MintProvider::from_env()?),
    };

    if verbose {
        println!("🔌 Backend: {}", translator.provider_name());
    }

    // Real MT engines translate the variants as one numbered block to keep
    // wording consistent across PLURAL/GENDER forms. The mock translator is
    // deterministic per-string, so a plain batch keeps its output predictable.
    let translated_texts = if backend == "mock" {
        translator
            .translate_batch(&source_texts, source_locale, target_locale)
            .await?
    } else {
        translator
            .translate_as_block(&source_texts, source_locale, target_locale)
            .await?
    };

    context.update_translations(translated_texts);

    if verbose {
        println!("🌍 Translated variants:");
        if context.variant_count() <= 10 {
            for (i, variant) in context.variants.iter().enumerate() {
                println!("   [{}] \"{}\"", i, variant.translated_text);
            }
        } else {
            for (i, variant) in context.variants.iter().take(5).enumerate() {
                println!("   [{}] \"{}\"", i, variant.translated_text);
            }
            println!("   ... {} more variants", context.variant_count() - 5);
        }
        println!();
    }

    // 4. Reassemble
    let reassembler = Reassembler::new(context.variable_types.clone());
    let result = match reassembler.reassemble(context.variants) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("❌ Failed to reassemble: {}", e);
            return Err(e.into());
        }
    };

    if verbose {
        println!("🔧 Reassembled wikitext:");
    }
    println!("{}", result);

    Ok(())
}
