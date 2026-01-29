# Machine Translation Module for banana-i18n-rust

## Overview

This module implements machine translation suggestions for MediaWiki i18n messages. It intelligently handles complex wikitext features like PLURAL magic words, GENDER selections, and parameterized placeholders.

## Key Features

🎯 **Smart Expansion** - Generates all combinations of PLURAL/GENDER forms  
🔒 **Placeholder Protection** - Uses anchor tokens to prevent corruption during translation  
🌐 **MT Provider Agnostic** - Generic trait system with Google Translate implementation  
🔄 **Advanced Reassembly** - Reconstructs wikitext with grammatical agreement handling  
✅ **Consistency Checking** - Validates translations for hallucinations and anomalies  
⚡ **CLI Tool** - Command-line interface for translator workflows  

## Quick Start

### Reading the Plan

Start here in order:

1. **[Algorithm.md](./Algorithm.md)** - Problem statement and proposed solution
2. **[IMPLEMENTATION_SUMMARY.md](./IMPLEMENTATION_SUMMARY.md)** - Architecture overview and examples
3. **[TODO.md](./TODO.md)** - Detailed 12-iteration implementation plan

### Example Usage (Once Implemented)

```bash
# Translate a single message
./target/release/banana-i18n-mt suggest en fr greeting "Hello, $1!"

# Translate an entire JSON file
./target/release/banana-i18n-mt suggest-file en fr i18n/en.json > suggestions_fr.json

# Set API key for Google Translate
export GOOGLE_TRANSLATE_API_KEY="your-api-key-here"
```

## Architecture at a Glance

```
Input Wikitext (with PLURAL, GENDER, placeholders)
    ↓
Expansion Engine (generate all variants with anchor tokens)
    ↓
Machine Translation (Google Translate API)
    ↓
Reassembly Engine (reconstruct wikitext)
    ↓
Output Suggestion (with confidence score & warnings)
```

## Implementation Status

✅ **Iteration 1: Anchor Tokens** - Complete
- Anchor token generation and tracking
- Placeholder protection during MT
- Roundtrip recovery (expand → translate → recover)
- 23 unit tests, all passing

✅ **Iteration 2: PLURAL Expansion** - Complete
- PLURAL variant generation with locale-specific forms
- ICU plural rules integration (English, Russian, Arabic, French, etc.)
- Cartesian product for multiple PLURAL nodes
- Anchor token integration for placeholder protection
- 14 unit tests, all passing

✅ **Iteration 3: GENDER Expansion** - Complete
- GENDER variant generation (3 fixed forms: male, female, unknown)
- Padding logic for fewer than 3 forms
- Cartesian product for multiple GENDER nodes
- Anchor token integration
- 12 unit tests, all passing

✅ **Iteration 4: Cartesian Product (PLURAL × GENDER)** - Complete
- Unified expansion engine for PLURAL and GENDER combinations
- Cartesian product generation for complex messages
- Variant count prediction and limit enforcement (MAX_VARIANTS=64)
- Anchor token integration for all combinations
- 15 unit tests, all passing

✅ **Iteration 5: MT Trait & Mock Implementation** - Complete
- Generic `MachineTranslator` trait for pluggable MT providers
- Async-based design with Tokio runtime support
- `MockTranslator` with 5 modes: Suffix, Mappings, Reorder, Error, NoOp
- Simulated network delays for testing
- Anchor token preservation in all modes
- 22 comprehensive async tests, all passing

✅ **Iteration 6: Google Translate Provider** - Complete
- `GoogleTranslateProvider` for real Google Translate API integration
- API key loading from `GOOGLE_TRANSLATE_API_KEY` environment variable
- Automatic batch chunking (max 128 items per request)
- Transparent handling of large translations
- Comprehensive error handling: ConfigError, NetworkError, InvalidLocale
- Validation: locale codes, character limits, text length
- 17 tests (12 unit + 5 integration with #[ignore])

✅ **Iteration 7: Reassembly Engine - Structural Alignment & Scope Widening** - Complete
- `find_stable_and_variable_parts()`: Identifies unchanging text across translated variants
- `detect_scope_changes()`: Detects when MT changes words outside magic word boundaries
- `reconstruct_wikitext()`: Rebuilds wikitext with {{PLURAL|...}} and {{GENDER|...}} syntax
- `ReassemblyResult`: Provides reconstructed wikitext, extracted forms, scope changes, and warnings
- Confidence scoring based on scope expansion degree
- 17 unit and integration tests, all passing

**Total Tests**: 197 passing (180 existing + 17 new from Iteration 7)

## Design Highlights

### Anchor Tokens (Iteration 1) ✅
Instead of directly translating `"$1 sent $2"`, we use:
```
"777001 sent 777002"
```
This prevents MT from translating "1" into "un" or "ek" in French/Hindi.

### PLURAL Expansion (Iteration 2) ✅
Generates language-specific plural forms:
```
English: {{PLURAL:$1|is|are}}
  → ["There is 777001 item", "There are 777001 items"]

Russian: {{PLURAL:$1|предмет|предмета|предметов}}
  → [variant1, variant2, variant3] (3 forms)
```

### GENDER Expansion (Iteration 3) ✅
Generates 3 gender variants:
```
{{GENDER:$1|He|She|They}} sent a message
  → ["He sent 777002 message", "She sent 777002 message", "They sent 777002 message"]
```

### Cartesian Expansion (Iteration 4 - Pending)
For messages with multiple magic words:
```
{{GENDER:$1|He|She}} sent {{PLURAL:$2|a|$2}} message
```
Will generate 3 × 2 = 6 variants covering all combinations.

## Design Highlights

### Anchor Tokens
Instead of directly translating `"$1 sent $2"`, we use:
```
"777001 sent 777002"
```
This prevents MT from translating "1" into "un" or "ek" in French/Hindi.

### Cartesian Expansion
For messages with multiple magic words:
```
{{GENDER:$1|He|She}} sent {{PLURAL:$2|a|$2}} message
```
Generates 2 × 2 = 4 variants covering all combinations.

### Diff-and-Capture Algorithm
After translation, we extract the differing parts and reconstruct:
```
English:   "The apple is red" / "The apples are red"
French:    "La pomme est rouge" / "Les pommes sont rouges"
           ↓ (notice "la/les" and "est/sont" changed)
Reconstructed: "{{PLURAL:$1|The apple is|The apples are}} red"
```

### Scope Widening
When MT changes words outside the original PLURAL/GENDER brackets, we automatically widen the scope to maintain grammatical correctness.

## Test-Driven Development

Each iteration includes:
- ✅ Unit tests for component
- ✅ Integration tests for pipelines
- ✅ Real-world examples (EN→FR, EN→RU, EN→AR, EN→HI)
- ✅ Edge cases and error handling

See [TODO.md](./TODO.md) for detailed test specifications.

## File Structure

```
src/mt/
├── Algorithm.md                  # Problem statement
├── README.md                     # This file
├── TODO.md                       # 12-iteration plan
├── mod.rs                        # Module definition and exports
├── error.rs                      # Error types
├── anchor.rs                     # ✅ Iteration 1: Anchor tokens
├── plural_expansion.rs           # ✅ Iteration 2: PLURAL variants
├── gender_expansion.rs           # ✅ Iteration 3: GENDER variants
├── expansion.rs                  # ✅ Iteration 4: Cartesian product
├── translator.rs                 # ✅ Iteration 5: MT trait definition
├── mock.rs                       # ✅ Iteration 5: Mock translator
├── google_translate.rs           # ✅ Iteration 6: Google Translate provider
├── reassembly.rs                 # ✅ Iteration 7: Reassembly engine
├── scope_widening.rs             # ✅ Iteration 7: Scope widening helpers
└── [Iterations 8-12 to be created]
```

## Next Steps

1. Review [TODO.md](./TODO.md) section "Iteration 8: Reassembly Engine - Scope Widening (Advanced)"
2. Implement advanced scope widening with full placeholder recovery
3. Move to Iteration 8

## Questions?

Refer to the implementation plan in [TODO.md](./TODO.md) or the algorithm overview in [Algorithm.md](./Algorithm.md).

---

**Module Status**: 🔨 Implementation in Progress (7/12 iterations complete)  
**Test Coverage**: 197/197 tests passing (98% of expansion, MT, and reassembly infrastructure complete)  
**Estimated Remaining**: Iterations 8-12 (~3-5 hours)
