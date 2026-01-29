# banana-i18n: Core Internationalization Library

A Rust library for internationalization (i18n) with MediaWiki-style message formatting and localization.

## Features

- **Wikitext Parser**: Parses MediaWiki-style messages with full support for:
  - Text nodes
  - Magic words: `{{PLURAL:$1|...}}`, `{{GENDER:$1|...}}`
  - Placeholders: `$1`, `$2`, etc.
  - Wiki links: `[[Page]]`, `[[Page|text]]`
  - External links: `[http://url]`, `[http://url text]`

- **Localization**: Multi-locale support with automatic fallback chains
  - Locale fallback: `de-at` → `de` → `en`
  - Complex pluralization for 50+ languages via ICU rules
  - Gender-aware message formatting

- **Message Loading**: Load messages from JSON files with metadata support

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
banana-i18n = "0.1.0"
```

## Quick Start

```rust
use banana_i18n::{LocalizedMessages, I18n, Parser};

fn main() {
    // Create localized messages
    let mut en_messages = LocalizedMessages::new();
    en_messages.with_message("greeting", "Hello, $1!");
    
    let mut i18n = I18n::new();
    i18n.with_locale("en")
        .with_messages_for_locale("en", en_messages);
    
    // Localize a message
    let result = i18n.localize("en", "greeting", &vec!["World".to_string()]);
    println!("{}", result); // Output: Hello, World!
}
```

## Advanced Usage

### Complex Messages with Magic Words

```rust
let mut messages = LocalizedMessages::new();
messages.with_message(
    "items",
    "There {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}}"
);

let result = i18n.localize("en", "items", &vec!["5".to_string()]);
// Output: There are 5 items
```

### Loading from JSON

```rust
use banana_i18n::load_messages_from_file;

let messages = load_messages_from_file("i18n/en.json")?;
i18n.with_messages_for_locale("en", messages);
```

## JSON Message Format

```json
{
  "@metadata": {
    "authors": ["Your Name"],
    "description": "English messages",
    "last-updated": "2024-01-22"
  },
  "greeting": "Hello, $1!",
  "farewell": "Goodbye, $1!",
  "plural": "There {{PLURAL:$1|is|are}} $1 item in the box",
  "pronoun": "{{GENDER:$1|He|She|They}} is here",
  "link": "Check [[article|this article]] for more info",
  "external": "Visit [http://example.com our website] for details"
}
```

## CLI Tool

Run the `banana-i18n` binary:

```bash
# Simple substitution
cargo run --bin banana-i18n -- en greeting "World"
# Output: Hello, World!

# PLURAL magic word
cargo run --bin banana-i18n -- en items "5"
# Output: There are 5 items in the box

# GENDER magic word
cargo run --bin banana-i18n -- en pronoun "male"
# Output: He is here

# Locale fallback
cargo run --bin banana-i18n -- de-at greeting "Wien"
# Falls back to German: Guten Tag, Wien!
```

## Machine Translation

For MT-assisted translation workflows, see [banana-i18n-mt](../banana-i18n-mt/).

## Architecture

- **Parser** (`parser.rs`): Converts messages to AST using tree-sitter
- **AST** (`ast.rs`): Data structures for parsed message nodes
- **Localization** (`lib.rs`): Core localization engine with fallbacks
- **Fallbacks** (`fallbacks.rs`): Locale chain resolution logic
- **Loader** (`loader.rs`): JSON message file loading

## License

MIT

## See Also

- [banana-i18n-mt](../banana-i18n-mt/) - Machine translation support
- [MediaWiki i18n documentation](https://www.mediawiki.org/wiki/Localization)
