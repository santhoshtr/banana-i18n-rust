# banana-i18n-rust

A Rust library for internationalization (i18n) with MediaWiki-style message formatting, localization, and machine translation support.

This is a **Cargo workspace** containing three related crates for internationalization and translation workflows.

## Workspace Modules

### banana-i18n - Core i18n Library

The core internationalization library providing wikitext parsing, message localization with automatic fallback chains, PLURAL and GENDER magic words (56+ languages via ICU CLDR), placeholder substitution, and wiki markup handling. Includes a CLI tool for quick testing.

See [banana-i18n/README.md](./banana-i18n/) for detailed documentation.

### banana-i18n-mt - Machine Translation Support

MT-assisted translation workflows for MediaWiki messages. Implements a 4-phase translation pipeline: message expansion to variants, batch translation via Google Translate API, reassembly using axis-collapsing algorithm, and placeholder recovery. Includes mock translator for testing and a CLI tool for MT workflows.

See [banana-i18n-mt/README.md](./banana-i18n-mt/) for detailed documentation including the comprehensive algorithm explanation.

### banana-i18n-mt-web - Web Interface

A web interface for translating i18n files with machine translation assistance. Built with Axum backend and vanilla JavaScript frontend. Supports file upload, interactive translation editing, AI-assisted suggestions, and JSON export.

See [banana-i18n-mt-web/README.md](./banana-i18n-mt-web/) for detailed documentation.

## Quick Start

### Using Core i18n

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
# With mock translator
cargo run --bin banana-mt -- --mock "Hello, \$1!" fr

# With Google Translate
export GOOGLE_TRANSLATE_API_KEY=your_key
cargo run --bin banana-mt -- "{{PLURAL:\$1|item|items}}" es
```

### Using the Web Interface

```bash
export GOOGLE_TRANSLATE_API_KEY=your_key
cargo run --bin banana-mt-web
# Open http://127.0.0.1:3000
```

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
cargo run --bin banana-mt-web
```

## Features

### PLURAL Magic Word

Automatic plural form selection based on ICU CLDR language rules:

```
{{PLURAL:$1|is|are}} $1 item
→ "is 1 item" (singular)
→ "are 5 items" (plural)
```

Supports 56+ languages with proper plural categories.

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

## Machine Translation Algorithm

The banana-i18n-mt module implements a sophisticated 4-phase translation pipeline:

1. **Expansion** - Generate all variant combinations (PLURAL × GENDER) with anchor token protection
2. **Translation** - Batch translate variants via MT API for consistency
3. **Reassembly** - Reconstruct wikitext using axis-collapsing algorithm with LCP/LCS extraction and word boundary snapping
4. **Recovery** - Restore placeholders from anchor tokens

This approach handles grammatical agreement, vowel elision, and case marking by translating complete sentences rather than word-by-word fragments.

See [banana-i18n-mt/README.md](./banana-i18n-mt/) for the comprehensive algorithm documentation with step-by-step examples.

## Documentation

Each module has detailed documentation in its respective directory:

- [banana-i18n README](./banana-i18n/) - Core library API and examples
- [banana-i18n-mt README](./banana-i18n-mt/) - MT support and algorithm documentation
- [banana-i18n-mt-web README](./banana-i18n-mt-web/) - Web interface usage guide
- [AGENTS.md](./AGENTS.md) - Build and development guidelines

## Example: Machine Translation Workflow

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

## Publishing

Crates can be published separately to crates.io:

```bash
cd banana-i18n && cargo publish
cd ../banana-i18n-mt && cargo publish
```

## Contributing

Refer to [AGENTS.md](./AGENTS.md) for development guidelines and coding standards.

## License

MIT

## Related Links

- [MediaWiki Localization](https://www.mediawiki.org/wiki/Localization)
- [ICU Plural Rules](https://unicode-org.github.io/cldr-json/charts/latest/supplemental/language_plural_rules.html)
- [MediaWiki Magic Words](https://www.mediawiki.org/wiki/Help:Magic_words)
