# banana-mt: Machine Translation CLI

A command-line tool for translating MediaWiki-style messages using the banana-i18n MT pipeline.

## Usage

```bash
cargo run --bin banana-mt -- [OPTIONS] <message> <target-locale>
```

### Arguments

- `<message>`: Source message to translate (supports MediaWiki magic words)
- `<target-locale>`: Target language code (e.g., fr, es, de)

### Options

- `-s, --source <source-locale>`: Source language code (default: en)
- `-m, --mock`: Use mock translator instead of Google Translate
- `-v, --verbose`: Show detailed translation process
- `-k, --key <key>`: Message key for context (default: auto-generated)
- `-h, --help`: Print help
- `-V, --version`: Print version

## Examples

### Simple Message with Placeholder
```bash
cargo run --bin banana-mt -- --mock "Hello, $1!" fr
# Output: Hello, $1!_fr
```

### PLURAL Magic Word
```bash
cargo run --bin banana-mt -- --mock "There {{PLURAL:$1|is|are}} $1 items" fr
# Output: There {{PLURAL:$1|is|are}} $1 items_fr
```

### GENDER Magic Word
```bash
cargo run --bin banana-mt -- --mock "{{GENDER:$1|He|She|They}} sent a message" fr
# Output: {{GENDER:$1|He|She|They}} sent a message_fr
```

### Complex Message (GENDER + PLURAL)
```bash
cargo run --bin banana-mt -- --mock "{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}" fr
# Output: {{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message_fr|$2 messages_fr}}
```

### Verbose Mode
```bash
cargo run --bin banana-mt -- --mock --verbose "Hello, $1!" fr
```

## Real Translation with Google Translate

Set your API key and use real translation:
```bash
export GOOGLE_TRANSLATE_API_KEY=your_api_key
cargo run --bin banana-mt -- "Hello, $1!" fr
```

## Supported Features

- **Placeholders**: `$1`, `$2`, etc. are protected using anchor tokens (777001, 777002) during translation
- **PLURAL Magic Word**: `{{PLURAL:$1|form1|form2|...}}` with proper plural forms per language
- **GENDER Magic Word**: `{{GENDER:$1|male|female|neutral}}` for gendered languages
- **Wiki Links**: `[[Page]]` and `[[Page|text]]` syntax support
- **External Links**: `[http://url]` and `[http://url text]` syntax support

## Translation Process

1. **Parse**: Convert message to AST using tree-sitter-wikitext
2. **Expand**: Create all variant combinations (cartesian product of magic words)
3. **Translate**: Translate all variants using block translation for consistency
4. **Reassemble**: Reconstruct wikitext with proper magic word syntax

## Environment Variables

- `GOOGLE_TRANSLATE_API_KEY`: Required for real translation (omit when using --mock)

## Error Handling

The CLI handles common errors gracefully:
- Missing API key (with helpful message)
- Parse errors in source message
- Translation API failures
- Reassembly consistency errors