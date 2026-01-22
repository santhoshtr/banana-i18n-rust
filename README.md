# banana-i18n-rust

A Rust library for internationalization (i18n) with MediaWiki-style message formatting and localization.

## Overview

**banana-i18n-rust** is a robust i18n library designed to handle complex multilingual message formatting with support for:

- **PLURAL magic words** - Automatic plural form selection based on ICU plural rules supporting 56+ languages
- **GENDER magic words** - Gender-based form selection (masculine, feminine, neutral)
- **Placeholder substitution** - Support for numbered placeholders ($1, $2, etc.)
- **Wiki links** - Parsing and handling of wiki markup links
- **External links** - Support for URLs and hyperlinks
- **Locale fallback chains** - Automatic fallback from specific locales to more general ones (e.g., de-at → de → en)
- **Verbosity-controlled logging** - Three levels of logging for debugging localization chains

## Features

### PLURAL Magic Word

Selects the correct plural form based on a number and language rules. Uses ICU CLDR plural rules for accurate localization across different languages.

**Format:** `{{PLURAL:value|singular|plural}}`

**Supported forms** (per language):
- English: 2 forms (one, other)
- Russian: 3 forms (one, few, many)
- Polish: 3 forms
- Arabic: 6 forms
- French: 2 forms with special rules
- Chinese: 1 form (no plural distinction)
- **56+ languages** supported via ICU plural rules

**Examples:**
```
{{PLURAL:$1|There is|There are}} $1 item
→ "There is 1 item" (with value=1)
→ "There are 5 items" (with value=5)

В коробке находится {{PLURAL:$1|предмет|предметов}}
→ "В коробке находится предмет" (Russian, value=1)
→ "В коробке находится предметов" (Russian, value=5)
```

### GENDER Magic Word

Selects the correct form based on gender (male, female, neutral).

**Format:** `{{GENDER:value|masculine|feminine|neutral}}`

**Examples:**
```
{{GENDER:$1|He|She|They}} is here
→ "He is here" (with gender=male)
→ "She is here" (with gender=female)
→ "They is here" (with gender=other)
```

### Placeholder Substitution

Replace numbered placeholders with provided values.

**Format:** `$1`, `$2`, `$3`, etc.

**Examples:**
```
Hello, $1!
→ "Hello, World!" (with $1=World)

$1 sent a message to $2
→ "Alice sent a message to Bob" (with $1=Alice, $2=Bob)
```

### Locale Fallback Chains

Automatically fall back from specific locales to more general ones:

- `de-at` → `de` → `en` (German Austria → German → English)
- `zh-hans` → `zh` → `en` (Simplified Chinese → Chinese → English)
- `fr-ca` → `fr` → `en` (Canadian French → French → English)

Messages are loaded from the first available locale in the fallback chain.

## Installation

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
banana-i18n = { path = "." }  # or from crates.io when published
serde_json = "1.0"
```

### From Source

```bash
git clone <repository>
cd banana-i18n-rust
cargo build --release
```

## Usage

### CLI Tool

The library includes a command-line tool for testing and using localized messages.

#### Basic Usage

```bash
# Build the binary
cargo build --release

# Display help
./target/release/banana-i18n

# Localize a message
./target/release/banana-i18n <locale> <message-key> [param1] [param2] ...
```

#### Examples

```bash
# Simple message with substitution
./target/release/banana-i18n en greeting "World"
# Output: Hello, World!

# Plural forms
./target/release/banana-i18n en plural "1"
# Output: There is 1 item in the box

./target/release/banana-i18n en plural "5"
# Output: There are 5 items in the box

# Gender-based forms
./target/release/banana-i18n en pronoun "male"
# Output: He is here

./target/release/banana-i18n en pronoun "female"
# Output: She is here

# Russian (3-form plurals)
./target/release/banana-i18n ru items "1"
# Output: В коробке находится 1 предмет

./target/release/banana-i18n ru items "5"
# Output: В коробке находится 5 предметов

# Locale fallback
./target/release/banana-i18n de-at greeting "Wien"
# Output: Guten Tag, Wien! (falls back from de-at → de)

# Chinese
./target/release/banana-i18n zh-hans greeting "北京"
# Output: 你好，北京
```

#### Environment Variables

Override the default messages directory:

```bash
I18N_MESSAGES_DIR=/path/to/messages ./target/release/banana-i18n en greeting "World"
```

### Using as a Library

```rust
use banana_i18n::{I18n, loader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load messages from directory
    let mut i18n = I18n::new();
    i18n.load_all_locales("i18n")?;
    
    // Get a message with locale fallback
    let message = i18n.get_message("en", "greeting");
    println!("{}", message); // "Hello, World!" or key name
    
    Ok(())
}
```

## Message Format

Messages are stored in JSON files with locale codes as filenames:

**File:** `en.json`
```json
{
  "@metadata": {
    "authors": ["Your Name"],
    "description": "English messages"
  },
  "greeting": "Hello, $1!",
  "plural": "There {{PLURAL:$1|is|are}} $1 item in the box",
  "pronoun": "{{GENDER:$1|He|She|They}} is here"
}
```

**File:** `ru.json`
```json
{
  "@metadata": {
    "authors": ["Ваше имя"],
    "description": "Russian messages"
  },
  "greeting": "Привет, $1!",
  "items": "В коробке находится {{PLURAL:$1|предмет|предметов|предметов}}"
}
```

### JSON Format Specification

- **@metadata** (optional): Document metadata (skipped during parsing)
  - `authors`: List of translators
  - `description`: Description of message set
  - Other custom metadata fields are ignored

- **Message keys** (required): Message strings with MediaWiki format
  - Supports all magic words (PLURAL, GENDER)
  - Supports placeholders ($1, $2, etc.)
  - Supports wiki links ([[link]], [[link|display]])
  - Supports external links ([http://url], [http://url text])

### Supported Locales

The library includes predefined fallback chains for:

- English (en)
- German (de, de-at, de-ch)
- Russian (ru)
- Polish (pl)
- Arabic (ar)
- French (fr, fr-ca)
- Chinese (zh, zh-hans, zh-hant)
- And 50+ more languages via CLDR

## Development

### Build

```bash
cargo build           # Debug build
cargo build --release # Release build
```

### Tests

```bash
cargo test --verbose         # Run all tests with output
cargo test test_name         # Run a specific test
cargo test -- --nocapture    # Run tests showing println! output
```

### Format & Lint

```bash
cargo fmt              # Format code
cargo clippy           # Lint code
cargo check            # Check for errors without building
```

## Code Style

- **Edition**: Rust 2024
- **Format**: Enforced with `cargo fmt`
- **Linting**: Follows clippy recommendations
- **Imports**: Use absolute paths (prefer `crate::` for internal modules)
- **Naming**:
  - Modules: `lowercase_with_underscores`
  - Types: `PascalCase`
  - Functions: `snake_case`
  - Constants: `SCREAMING_SNAKE_CASE`

## Project Structure

```
banana-i18n-rust/
├── src/
│   ├── lib.rs         # Main library (public API)
│   ├── main.rs        # CLI tool entry point
│   ├── ast.rs         # AST nodes and localization logic
│   ├── parser.rs      # MediaWiki format parser
│   ├── fallbacks.rs   # Locale fallback chain resolution
│   └── loader.rs      # JSON file loading
├── i18n/              # Sample message files
│   ├── en.json        # English messages
│   ├── ru.json        # Russian messages
│   ├── de.json        # German messages
│   ├── fr.json        # French messages
│   └── zh-hans.json   # Simplified Chinese messages
├── Cargo.toml         # Project manifest
├── Cargo.lock         # Dependency lock file
└── README.md          # This file
```

## Architecture

### Core Components

- **I18n struct**: Main entry point managing localized messages by locale
- **LocalizedMessages**: Wrapper around HashMap for key-value message pairs
- **AstNode enum**: Represents different types of wiki markup:
  - `Placeholder` - Numbered variables ($1, $2)
  - `Text` - Plain text content
  - `InternalLink` - Wiki links [[page]] or [[page|text]]
  - `ExternalLink` - URLs [http://example.com text]
  - `Gender` - GENDER magic word
  - `Plural` - PLURAL magic word
- **Parser**: Converts MediaWiki message format strings into AST nodes
- **Localizer**: Applies value substitution and performs localization

### Localization Flow

1. **Parse** - Convert message string to AST using wikitext parser
2. **Localize** - Traverse AST and apply substitutions
   - Replace placeholders with provided values
   - Evaluate PLURAL forms based on language rules
   - Evaluate GENDER forms based on provided gender
3. **Fallback** - If message not found, try fallback locale chain

### Error Handling

The library uses `Option<T>` for fallible operations:

- `get_message(key)` - Returns `Option<&String>` (None if not found)
- `get(key)` - Returns `String` (uses key as fallback if not found)
- Missing messages default to the message key itself

## Dependencies

### Core Dependencies

- **tree-sitter-wikitext** (v0.1.1) - Parses MediaWiki/wikitext format
- **icu_plurals** (v2.1.1) - Provides ICU CLDR plural rules for 56+ languages

### Development Dependencies

- **serde** (v1.0.228) - Serialization framework
- **serde_json** (v1.0.149) - JSON support

## Testing

The project includes 69 comprehensive tests covering:

- **Parser** - MediaWiki format parsing
- **PLURAL** - All language plural rules, edge cases, fallback chains
- **GENDER** - All gender forms and edge cases
- **Fallbacks** - Locale chain resolution and cycle detection
- **Integration** - End-to-end localization workflow

Run tests:
```bash
cargo test --verbose
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes and ensure tests pass: `cargo test`
4. Format code: `cargo fmt`
5. Check with clippy: `cargo clippy`
6. Submit a pull request

## License

MIT License - See LICENSE file for details

## References

- [MediaWiki Message Format Documentation](https://www.mediawiki.org/wiki/Localisation)
- [ICU CLDR Plural Rules](https://cldr.unicode.org/index/cldr-spec/plural-rules)
- [ISO 639-1 Language Codes](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes)

## Support

For issues, questions, or suggestions:

1. Check existing issues on GitHub
2. Create a new issue with:
   - Rust version (`rustc --version`)
   - OS and environment details
   - Minimal reproducible example
   - Expected vs actual behavior

## Acknowledgments

- Built with [tree-sitter](https://tree-sitter.github.io/tree-sitter/) for parsing
- Uses [ICU CLDR data](https://cldr.unicode.org/) for pluralization rules
- Inspired by [MediaWiki's i18n system](https://www.mediawiki.org/wiki/Localisation)
