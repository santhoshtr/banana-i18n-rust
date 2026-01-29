# banana-i18n-rust: Workspace

A Rust library for internationalization (i18n) with MediaWiki-style message formatting, localization, and machine translation support.

This is a **Cargo workspace** containing three related crates for internationalization and translation workflows.

## 📦 Workspace Structure

### 🌍 [banana-i18n](./banana-i18n/) - Core i18n Library

The core internationalization library providing:

- **Wikitext Parser** - Parse MediaWiki-style messages with magic words
- **Message Localization** - Multi-locale support with automatic fallback chains  
- **PLURAL Magic Word** - Automatic plural form selection (56+ languages via ICU)
- **GENDER Magic Word** - Gender-based form selection
- **Placeholder Substitution** - Support for $1, $2, etc.
- **Wiki & External Links** - Parse and handle wiki markup
- **CLI Tool** - `banana-i18n` binary for quick testing

**Perfect for:** Applications that need MediaWiki-compatible message formatting and localization.

### 🤖 [banana-i18n-mt](./banana-i18n-mt/) - Machine Translation Support

MT-assisted translation workflows for MediaWiki messages:

- **Message Expansion** - Convert magic words to translation variants
- **Block Translation** - Translate related variants together for consistency
- **Google Translate Integration** - Real translation with API key
- **Mock Translator** - Test without API (suffix-based, reorder modes)
- **Consistency Checking** - Detect MT hallucinations
- **Reassembly Engine** - Reconstruct wikitext from translations
- **CLI Tool** - `banana-mt` binary for MT workflows

**Perfect for:** Localizers who need MT-assisted translation of complex MediaWiki messages.

### 🌐 [banana-i18n-mt-web](./banana-i18n-mt-web/) - Web Interface for MT Translations

A user-friendly web interface for translating i18n files with machine translation assistance:

- **File Upload** - Load JSON i18n files (e.g., `en.json`) via file selector
- **Interactive Translation** - Expandable message items with source and translation views
- **AI Suggestions** - Automatic Google Translate suggestions for each message
- **Edit & Review** - Edit machine translations with full wikitext support
- **Export to JSON** - Download translated messages as `<language>.json`
- **Vanilla Frontend** - No frameworks - clean HTML/CSS/JavaScript
- **Axum Backend** - Fast Rust web server with `/api/translate` endpoint
- **Multi-language** - Support for 5+ languages (extensible)

**Perfect for:** Translators and localization teams who prefer a web-based workflow with MT assistance.

## Quick Start

### Using Core i18n Only

```toml
[dependencies]
banana-i18n = { path = "./banana-i18n" }
```

```rust
use banana_i18n::{LocalizedMessages, I18n};

let mut messages = LocalizedMessages::new();
messages.with_message("greeting", "Hello, $1!");

let mut i18n = I18n::new();
i18n.with_messages_for_locale("en", messages);

let result = i18n.localize("en", "greeting", &vec!["World".to_string()]);
println!("{}", result); // Hello, World!
```

### Using Machine Translation

```bash
# With mock translator (no API key needed)
cargo run --bin banana-mt -- --mock "Hello, \$1!" fr

# With Google Translate
export GOOGLE_TRANSLATE_API_KEY=your_key
cargo run --bin banana-mt -- "{{PLURAL:\$1|item|items}}" es
```

### Using the Web Interface

```bash
# Set up Google Translate API key
export GOOGLE_TRANSLATE_API_KEY=your_key

# Start the web server
cargo run --bin banana-mt-web

# Open in browser
# http://127.0.0.1:3000
```

Then:
1. Upload an i18n JSON file (e.g., `en.json`)
2. Select target language (Spanish, French, German, Russian, Chinese, etc.)
3. Review and edit machine translations
4. Export translated file as `<language>.json`

## Building & Testing

```bash
# Build entire workspace
cargo build --workspace

# Build specific crate
cargo build -p banana-i18n
cargo build -p banana-i18n-mt
cargo build -p banana-i18n-mt-web

# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p banana-i18n
cargo test -p banana-i18n-mt

# Run CLI tools
cargo run --bin banana-i18n -- en greeting "World"
cargo run --bin banana-mt -- --mock "Hello, \$1!" fr

# Run web server
cargo run --bin banana-mt-web
```

## Features Overview

### PLURAL Magic Word

Automatic plural form selection based on language rules:

```
{{PLURAL:$1|is|are}} $1 item
→ "is 1 item" (singular)
→ "are 5 items" (plural)
```

Supports 56+ languages with proper ICU plural rules.

### GENDER Magic Word

Gender-based form selection:

```
{{GENDER:$1|He|She|They}} is here
→ "He is here" (male)
→ "She is here" (female)  
→ "They is here" (neutral)
```

### Locale Fallback

Automatic fallback chains for missing messages:

```
de-at → de → en
zh-cn → zh-hans → zh → en
```

### Wikitext Parsing

Full support for MediaWiki message syntax:

```
Hello [[User:$1|$1]]!
Visit [http://example.com our site] for more.
```

## Architecture

### banana-i18n

- `parser.rs` - Wikitext parser using tree-sitter
- `ast.rs` - AST node definitions
- `lib.rs` - Core localization engine
- `fallbacks.rs` - Locale fallback logic
- `loader.rs` - JSON message file loading

### banana-i18n-mt

- `expansion.rs` - Message expansion to variants
- `google_translate.rs` - Google Translate integration
- `mock.rs` - Mock translator for testing
- `reassembly.rs` - Reconstruct wikitext from translations
- `data.rs` - Core MT data structures
- `translator.rs` - Translator trait and utilities
- `error.rs` - Error types

### banana-i18n-mt-web

- `main.rs` - Axum web server and routing
- `static/index.html` - Interactive web interface
- `static/app.js` - Vanilla JavaScript (no frameworks)
- `static/style.css` - Responsive CSS styling
- `/api/translate` - Backend translation API endpoint

## Dependencies

### banana-i18n

```
icu_locale = "2.1"
icu_plurals = "2.1.1"
tree-sitter = "0.26"
tree-sitter-wikitext = "0.1.1"
serde = "1.0"
serde_json = "1.0"
```

### banana-i18n-mt

```
banana-i18n = { path = "../banana-i18n" }
tokio = "1" (async runtime)
reqwest = "0.13" (HTTP client for Google Translate)
async-trait = "0.1" (async traits)
regex = "1.10" (text processing)
clap = "4.0" (CLI argument parsing)
icu_plurals = "2.1.1" (plural rules)
```

### banana-i18n-mt-web

```
axum = "0.8" (web framework)
tokio = "1" (async runtime)
tower-http = "0.6" (HTTP utilities)
serde/serde_json = "1.0" (JSON handling)
banana-i18n = { path = "../banana-i18n" }
banana-i18n-mt = { path = "../banana-i18n-mt" }
```

## Publishing

Both crates are designed to be published separately to crates.io:

```bash
# Publish core library first
cd banana-i18n
cargo publish

# Then publish MT support
cd ../banana-i18n-mt
cargo publish
```

## Documentation

- **[banana-i18n README](./banana-i18n/README.md)** - Core library documentation
- **[banana-i18n-mt README](./banana-i18n-mt/README.md)** - MT support documentation
- **[banana-i18n-mt-web README](./banana-i18n-mt-web/README.md)** - Web interface documentation
- **[banana-i18n-mt Algorithm](./banana-i18n-mt/Algorithm.md)** - Detailed MT algorithm explanation
- **[AGENTS.md](./AGENTS.md)** - Build and development guidelines

## Examples

### Core i18n: Locale Fallback

```rust
let mut i18n = I18n::new();
i18n.with_messages_for_locale("en", en_messages)
    .with_messages_for_locale("de", de_messages)
    .with_verbosity(VerbosityLevel::Silent);

// Falls back: de-at → de → en
let msg = i18n.localize("de-at", "key", &vec![]);
```

### Machine Translation: Full Workflow

```rust
use banana_i18n_mt::{prepare_for_translation, Reassembler, GoogleTranslateProvider, MachineTranslator};
use banana_i18n::parser::Parser;

let mut parser = Parser::new("{{GENDER:$1|He|She}} sent $1 items");
let ast = parser.parse();

let mut context = prepare_for_translation(&ast, "en", "msg")?;

let provider = GoogleTranslateProvider::from_env()?;
let translations = provider.translate_as_block(
    &context.source_texts(),
    "en", "fr"
).await?;
context.update_translations(translations);

let reassembler = Reassembler::new(context.variable_types);
let result = reassembler.reassemble(context.variants)?;
println!("{}", result);
```

## Contributing

Please refer to [AGENTS.md](./AGENTS.md) for development guidelines and coding standards.

## License

MIT

## Related Links

- [MediaWiki Localization](https://www.mediawiki.org/wiki/Localization)
- [ICU Plural Rules](https://unicode-org.github.io/cldr-json/charts/latest/supplemental/language_plural_rules.html)
- [MediaWiki Magic Words](https://www.mediawiki.org/wiki/Help:Magic_words)
