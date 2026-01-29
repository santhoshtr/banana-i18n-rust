# banana-i18n-mt-web

🍌 Web interface for machine translation-assisted i18n localization using `banana-i18n` and `banana-i18n-mt`.

## Features

- 📁 **Upload i18n files**: Upload JSON files (e.g., `en.json`) containing message definitions
- 🤖 **AI-Assisted Translation**: Get automatic translation suggestions using Google Translate
- ✏️ **Edit translations**: Review and edit machine translations before export
- 🌍 **Multi-language support**: Translate to Spanish, French, German, Russian, Chinese, and more
- 💾 **Export to JSON**: Download translated messages as `<language>.json`
- 🎨 **Vanilla frontend**: No JavaScript frameworks - clean HTML/CSS/JS
- ⚡ **Powered by Rust**: Fast Axum backend with MediaWiki wikitext support

## Architecture

### Backend (Axum)

**Dependencies:**
- `axum` - Web framework
- `tokio` - Async runtime
- `banana-i18n` - Core i18n library
- `banana-i18n-mt` - Machine translation support (Google Translate)

**API Endpoints:**

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/` | Serve HTML interface |
| POST | `/api/translate` | Translate a single message |

**POST /api/translate**

Request:
```json
{
  "message": "Hello, $1!",
  "target_language": "es",
  "key": "greeting"
}
```

Response (200 OK):
```json
{
  "translated": "¡Hola, $1!",
  "source": "Hello, $1!"
}
```

Error Response (400/500):
```json
{
  "error": "Translation failed: invalid language code"
}
```

### Frontend (Vanilla HTML/CSS/JS)

**No frameworks or build tools required** - just vanilla web technologies:

- **HTML5**: Semantic markup with `<details>` elements for expandable message items
- **CSS3**: Modern responsive design with CSS Grid and Flexbox
- **JavaScript (ES6+)**: Client-side state management and API integration

**Key features:**

1. **File Upload**: Load i18n JSON files with file selector
2. **Message List**: Each message displayed as a collapsible `<details>` element
3. **Translation UI**: Source message view + editable textarea for translation
4. **Status Indicators**: Visual feedback for pending/translating/translated/edited states
5. **Export**: Download all translations as JSON with `@metadata` section

## Setup

### Prerequisites

- Rust 1.70+ (for building)
- Google Translate API key (for machine translation)
- Node/npm NOT required (vanilla frontend)

### Installation

1. Clone and navigate to workspace:
```bash
cd /path/to/banana-i18n-rust
```

2. Set up environment:
```bash
cp banana-i18n-mt-web/.env.example .env
# Edit .env and add your GOOGLE_TRANSLATE_API_KEY
```

3. Build all workspace crates:
```bash
cargo build --workspace
```

Or build just the web crate:
```bash
cargo build -p banana-i18n-mt-web
```

### Running

Start the server:

```bash
# Debug mode (verbose logging)
RUST_LOG=info cargo run -p banana-i18n-mt-web

# Release mode (optimized)
cargo run --release -p banana-i18n-mt-web
```

The server will be available at: **http://127.0.0.1:3000**

### Google Translate Setup

1. Create a Google Cloud project: https://cloud.google.com/
2. Enable the Translation API
3. Create a service account and download JSON credentials
4. Set environment variable:
   ```bash
   export GOOGLE_TRANSLATE_API_KEY="$(cat /path/to/credentials.json | jq -r '.private_key')"
   ```

Or add to `.env` file:
```
GOOGLE_TRANSLATE_API_KEY=your_key_here
```

## Usage Workflow

### Step 1: Start the server

```bash
cargo run --release -p banana-i18n-mt-web
```

### Step 2: Open browser

Navigate to `http://127.0.0.1:3000`

### Step 3: Upload i18n file

Click **📁 Upload JSON File** and select your source file (e.g., `en.json`)

**Expected format:**
```json
{
  "@metadata": {
    "authors": ["Your Name"],
    "locale": "en"
  },
  "greeting": "Hello, $1!",
  "plural": "There {{PLURAL:$1|is|are}} $1 item{{PLURAL:$1||s}}",
  "pronoun": "{{GENDER:$1|He|She|They}} arrived"
}
```

### Step 4: Select target language

Choose target language from dropdown (Spanish, French, German, Russian, Chinese Simplified)

### Step 5: Review and translate

- **Click** on a message to expand and view source text
- **Machine translation** is automatically suggested when you open a message
- **Edit** the translation in the textarea as needed
- **Status indicators** show: ⏳ Pending → 🔄 Translating → ✓ Translated → ✏️ Edited

### Step 6: Export translations

Click **💾 Export Translation** to download `<language>.json` with all your translations

**Exported format:**
```json
{
  "@metadata": {
    "authors": ["Machine Translation"],
    "last-updated": "2026-01-29",
    "locale": "es"
  },
  "greeting": "¡Hola, $1!",
  "plural": "Hay {{PLURAL:$1|es|son}} $1 element{{PLURAL:$1||s}}"
}
```

## Message Format Support

The interface supports MediaWiki wikitext message syntax:

| Syntax | Example | Notes |
|--------|---------|-------|
| **Placeholder** | `Hello, $1!` | Variable substitution |
| **PLURAL** | `{{PLURAL:$1\|item\|items}}` | Language-aware pluralization |
| **GENDER** | `{{GENDER:$1\|He\|She\|They}}` | Gender-aware pronouns |
| **Wiki Links** | `[[Article]]` or `[[Article\|text]]` | Internal links |
| **External Links** | `[http://example.com text]` | External links |

**Examples:**

```json
{
  "greeting": "Hello, $1!",
  "items": "There {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}}",
  "pronoun": "{{GENDER:$1|He|She|They}} sent a message",
  "link": "Visit [[Help]] for more information"
}
```

## Project Structure

```
banana-i18n-mt-web/
├── Cargo.toml              # Dependencies and metadata
├── .env.example            # Environment configuration template
├── README.md               # This file
└── src/
    ├── main.rs             # Axum server, routing, API handlers
    └── static/
        ├── index.html      # Web interface markup
        ├── style.css       # Vanilla CSS styling
        └── app.js          # Client-side JavaScript
```

## API Error Handling

The API provides clear error messages:

| Status | Error | Cause |
|--------|-------|-------|
| 400 | Invalid language code | Target language not supported |
| 400 | Failed to prepare message | Malformed wikitext syntax |
| 500 | Translation service error | Google Translate API error |
| 500 | Failed to reassemble message | Error reconstructing wikitext |

## Development Notes

### Adding New Languages

Edit `index.html` to add options to the `<select id="targetLang">`:

```html
<option value="pt">Portuguese (pt)</option>
<option value="ja">Japanese (ja)</option>
```

Supported language codes: [ISO 639-1 codes](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes)

### Styling

All styling is in `src/static/style.css`. No CSS frameworks - pure CSS3 with:
- CSS Grid for layouts
- Flexbox for alignment
- CSS custom properties for colors (optional enhancement)
- Mobile-responsive design

### JavaScript

The `app.js` file uses vanilla JavaScript with:
- Fetch API for HTTP requests
- DOM manipulation with vanilla methods (no jQuery)
- LocalStorage for potential future enhancements
- ES6+ features (arrow functions, async/await, template literals)

## Testing

### Manual Testing

1. **Simple message**:
   ```json
   { "greeting": "Hello, $1!" }
   ```
   Upload and translate to verify basic workflow

2. **Complex message with PLURAL**:
   ```json
   { "items": "There {{PLURAL:$1|is|are}} $1 item{{PLURAL:$1||s}}" }
   ```

3. **Multiple messages**:
   ```json
   {
     "greeting": "Hello, $1!",
     "farewell": "Goodbye, $1!",
     "pronoun": "{{GENDER:$1|He|She|They}} is here"
   }
   ```

### Troubleshooting

**"Failed to initialize translator: ..."**
- Check that `GOOGLE_TRANSLATE_API_KEY` is set and valid
- Verify Google Translate API is enabled in your GCP project

**"Translation service error"**
- Network connectivity issue with Google Translate API
- API rate limit exceeded
- Invalid target language code

**File upload fails**
- Ensure JSON is valid (use `jq . file.json` to validate)
- Check file doesn't exceed browser's paste limit

## Performance Notes

- **Backend**: Fast Rust implementation with async/await
- **Frontend**: No framework overhead - minimal JavaScript
- **Caching**: Consider enabling Redis for repeated translations
- **API calls**: Each message translates as a separate request for user control

## Future Enhancements

- Batch translation for faster processing
- Translation memory/caching
- Language auto-detection
- Glossary support for consistent terminology
- Collaborative editing with multiple users
- History/undo functionality
- Syntax validation with live preview

## License

MIT - See LICENSE file in root workspace

## Contributing

Contributions welcome! This is part of the banana-i18n project.

## Related Documentation

- [banana-i18n README](../banana-i18n/README.md) - Core i18n library
- [banana-i18n-mt README](../banana-i18n-mt/README.md) - MT library with algorithm details
- [Root README](../README.md) - Workspace overview
