# Machine Translation Module - Implementation Plan

## Overview

This document outlines the test-driven, incremental implementation of the MT (Machine Translation) module for the banana-i18n-rust library. The goal is to provide machine translation suggestions for wikitext i18n messages while preserving PLURAL, GENDER, and placeholder syntax.

## Architecture Summary

The MT module consists of 5 core components:

1. **Expansion Engine** - Converts wikitext with magic words into all possible plain-text variants
2. **MT Trait & Providers** - Generic trait for any MT system, with Google Translate implementation
3. **Reassembly Engine** - Reconstructs wikitext from translated variants using Diff-and-Capture
4. **Consistency Checker** - Validates translated variants for hallucinations
5. **Suggestion Generator** - Orchestrates the full pipeline

## Implementation Iterations

### Iteration 1: Placeholder Expansion & Anchor Tokens (Foundation)

**Goal**: Implement anchor token system to protect placeholders from MT corruption

**Tasks**:
- [ ] Create `AnchorToken` struct: `{ placeholder_index: usize, token: String }`
- [ ] Implement anchor token generator: `generate_anchor_tokens(count) -> Vec<String>`
  - Format: `_ID1_`, `_ID2_`, etc. (non-translatable format)
- [ ] Implement `replace_placeholders_with_anchors(ast: &AstNodeList, anchors: &[String]) -> String`
  - Replaces $1, $2, etc. with anchor tokens in text
- [ ] Implement `recover_placeholders_from_anchors(text: &str, anchors: &[String]) -> String`
  - Reverses the replacement to restore $1, $2

**Tests**:
- [ ] Test anchor token generation (10 tokens, verify uniqueness)
- [ ] Test single placeholder replacement: `"Hello, $1"` → `"Hello, _ID1_"`
- [ ] Test multiple placeholders: `"$1 sent $2"` → `"_ID1_ sent _ID2_"`
- [ ] Test recovery: `"_ID1_ sent _ID2_"` → `"$1 sent $2"`
- [ ] Test MT placeholder reordering: `"_ID2_ は _ID1_ によって"` → restored correctly

**Files**:
- [ ] `src/mt/mod.rs` - Module definition
- [ ] `src/mt/anchor.rs` - Anchor token logic

---

### Iteration 2: Expansion Engine - PLURAL Variants (Core)

**Goal**: Generate all PLURAL form variants for a message in target language

**Tasks**:
- [ ] Analyze AST to find all PLURAL nodes and their target languages
- [ ] Implement `get_plural_forms_for_language(locale: &str) -> Vec<(PluralCategory, u32)>`
  - Returns representative numbers for each plural category (e.g., Russian: 1, 2, 5)
  - Use ICU plural rules from existing code
- [ ] Implement `expand_plural_variants(ast: &AstNodeList, locale: &str) -> Vec<String>`
  - Substitutes test values into each PLURAL choice
  - Returns all plain-text variants
- [ ] Handle partial plural forms (fewer forms than language requires)

**Tests**:
- [ ] English PLURAL: `{{PLURAL:$1|is|are}}` → 2 variants ("is", "are")
- [ ] Russian PLURAL: `{{PLURAL:$1|предмет|предмета|предметов}}` → 3 variants
- [ ] Arabic PLURAL: 6 forms, but only 2 provided → pad and expand to 6 variants
- [ ] Test with placeholder substitution: `"$1 {{PLURAL:$2|is|are}} red"` → multiple variants
- [ ] Test empty PLURAL: `{{PLURAL:$1}}` → empty variant
- [ ] Test direct number: `{{PLURAL:5|item|items}}` → just plural form (no variants)

**Files**:
- [ ] `src/mt/expansion.rs` - Expansion logic
- [ ] `src/mt/plural_expansion.rs` - PLURAL-specific expansion

---

### Iteration 3: Expansion Engine - GENDER Variants (Core) ✅

**Goal**: Generate all GENDER form variants

**Tasks**:
- [x] Implement `expand_gender_variants(ast: &AstNodeList) -> Vec<String>`
  - Substitutes test genders: "male", "female", "unknown"
  - Returns variants for each gender choice
- [x] Handle partial gender forms (fewer than 3 forms provided)

**Tests**:
- [x] Simple GENDER: `{{GENDER:$1|he|she}}` → 3 variants (padded)
- [x] Three forms: `{{GENDER:$1|he|she|they}}` → 3 variants
- [x] Single form: `{{GENDER:$1|person}}` → padded to 3 variants
- [x] Direct parameter: `{{GENDER:male|...}}` → 3 variants (expansion for all genders)
- [x] Empty GENDER: `{{GENDER:$1}}` → 1 variant (empty)
- [x] Multiple GENDER nodes: generates 3×3=9 variants for 2 nodes with 2 forms each
- [x] With placeholders: anchor tokens applied correctly
- [x] With links: WikiInternalLink and WikiExternalLink rendering
- [x] Roundtrip test: expand → anchor → recover

**Files**:
- [x] `src/mt/gender_expansion.rs` - GENDER-specific expansion (427 LOC)

**Test Results**: 12 tests passing, all edge cases covered

---

### Iteration 4: Expansion Engine - Cartesian Product (Complex)

**Goal**: Generate all combinations of PLURAL × GENDER variants

**Tasks**:
- [ ] Implement `expand_all_variants(ast: &AstNodeList, target_locale: &str, max_variants: usize) -> Result<Vec<String>>`
  - Takes source wikitext and target locale
  - Generates Cartesian product of PLURAL and GENDER
  - Returns vector of all plain-text variants with anchor tokens
  - **Limit**: Max 64 variants, warn if exceeded
- [ ] Implement `calculate_variant_count(ast: &AstNodeList, target_locale: &str) -> usize`
  - Predicts number of variants before expansion

**Tests**:
- [ ] Simple message: `"Hello, $1"` → 1 variant
- [ ] PLURAL only: `{{PLURAL:$1|a|b}}` → 2 variants (English)
- [ ] GENDER only: `{{GENDER:$1|a|b}}` → 2 variants
- [ ] PLURAL × GENDER: Both → 2 × 2 = 4 variants
- [ ] Complex example: `{{GENDER:$1|$1}} sent {{PLURAL:$2|a|b}} to {{GENDER:$3|him|her}}`
  - → 2 × 2 × 2 = 8 variants
- [ ] Russian expansion: 3 PLURAL forms × 2 GENDER forms = 6 variants
- [ ] **Limit exceeded**: Arabic with complex message → 64+ variants
  - Should return `Err("Too many variants (125 > 64)")`

**Files**:
- [ ] `src/mt/cartesian.rs` - Cartesian product logic

---

### Iteration 5: MT Trait & Mock Implementation (Infrastructure)

**Goal**: Define trait and implement mock MT provider for testing

**Tasks**:
- [ ] Define `MachineTranslator` trait:
  ```rust
  pub trait MachineTranslator: Send + Sync {
      fn translate(&self, text: &str, source_locale: &str, target_locale: &str) -> Result<String>;
      fn translate_batch(&self, texts: &[String], source_locale: &str, target_locale: &str) -> Result<Vec<String>>;
  }
  ```
- [ ] Implement `MockTranslator` for testing (deterministic translations)
  - Simple pattern: append locale suffix or use predefined mappings
- [ ] Add trait to lib.rs exports

**Tests**:
- [ ] Mock single translation: `"hello"` → `"hello_fr"`
- [ ] Mock batch translation: 4 variants → 4 translated variants
- [ ] Mock error handling: simulate API failures gracefully
- [ ] Mock word reordering: Japanese variant swaps order
- [ ] Mock plural agreement: French adds "l'" before vowels

**Files**:
- [ ] `src/mt/translator.rs` - Trait definition
- [ ] `src/mt/mock.rs` - Mock implementation

---

### Iteration 6: Google Translate Provider (Real MT)

**Goal**: Implement real Google Translate API integration

**Tasks**:
- [ ] Add `google-cloud-translate` crate dependency
- [ ] Implement `GoogleTranslateProvider` struct:
  - Load API key from environment variable: `GOOGLE_TRANSLATE_API_KEY`
  - Implement `MachineTranslator` trait
  - Batch support (translate up to 128 items per request)
- [ ] Error handling:
  - Invalid API key → clear error message
  - Network timeouts → retry with backoff
  - Rate limits → backoff and retry
  - Invalid locale codes → early validation

**Tests**:
- [ ] Unit test: verify API key loading from ENV
- [ ] Unit test: batch request construction (verify JSON format)
- [ ] Unit test: response parsing (verify output structure)
- [ ] Integration test (requires real API key):
  - English → French: `"hello"` → `"bonjour"`
  - Batch translation: multiple strings in one request
  - Error handling: invalid API key → returns error

**Files**:
- [ ] `src/mt/google_translate.rs` - Google Translate provider

**Dependencies to add**:
- [ ] `google-cloud-translate` crate (or direct HTTP library)

---

### Iteration 7: Reassembly Engine - Structural Alignment (Foundation)

**Goal**: Extract diffs from translated variants to identify variable parts

**Tasks**:
- [ ] Implement `find_stable_parts(variants: &[String]) -> Vec<(usize, String)>`
  - Compares all variants to find common text segments
  - Returns positions and text of stable parts
  - Example: All variants start with "L" and end with "rouge" → stable
- [ ] Implement `extract_variable_parts(variants: &[String], stable: &[(usize, String)]) -> Vec<String>`
  - Extracts the differing portions at each PLURAL/GENDER position
- [ ] Implement `align_with_source_ast(source_ast: &AstNodeList, translated_variants: &[String]) -> Vec<(usize, Vec<String>)>`
  - Maps translated variants back to original AST node indices

**Tests**:
- [ ] Simple case: 2 English PLURAL variants
  - Source: `"There {{PLURAL:$1|is|are}} $1 item"`
  - Translated (en): `"There is 1 item"` / `"There are 5 items"`
  - Expected alignment: position 6 is "is"/"are", rest stable
- [ ] French case with scope expansion:
  - Source: `"The $1 apple {{PLURAL:$2|is|are}} red"`
  - Variants: `"La 1 pomme est rouge"` / `"Les 5 pommes sont rouges"`
  - Note: scope changed from just "is/are" to "La/Les" and "est/sont"
- [ ] Complex case: 4 variants from PLURAL × GENDER

**Files**:
- [ ] `src/mt/reassembly.rs` - Reassembly core logic

---

### Iteration 8: Reassembly Engine - Scope Widening (Advanced)

**Goal**: Detect and expand scope when MT changes words outside magic words

**Tasks**:
- [ ] Implement `detect_scope_changes(source_variants: &[String], translated_variants: &[String]) -> Vec<(usize, usize)>`
  - Compares positions to find unexpected changes
  - Returns ranges that should be widened
- [ ] Implement `expand_wikitext_scope(ast: &AstNodeList, change_ranges: &[(usize, usize)]) -> AstNodeList`
  - Expands PLURAL/GENDER magic words to include nearby text
  - Example: `"The {{PLURAL:$1|apple|apples}}"` → `"{{PLURAL:$1|The apple|The apples}}"`
- [ ] Implement `generate_scope_warnings(changes: &[(usize, usize)]) -> Vec<String>`
  - Reports to user which parts of the original message were changed

**Tests**:
- [ ] French scope change: article + adjective agreement
  - Source: `"The {{PLURAL:$1|apple|apples}}"`
  - Detected change: "La/Les" before magic word
  - Widened scope: `"{{PLURAL:$1|The apple|The apples}}"`
  - Warning: "Scope expanded to include article"
- [ ] German case agreement
- [ ] Russian adjective agreement
- [ ] No scope change: English → German (1:1 mapping)

**Files**:
- [ ] `src/mt/scope_widening.rs` - Scope detection and expansion

---

### Iteration 9: Reassembly Engine - Placeholder Recovery (Complex)

**Goal**: Handle word-order changes and placeholder position recovery

**Tasks**:
- [ ] Implement `locate_anchors_in_text(text: &str, anchors: &[String]) -> Vec<(String, usize)>`
  - Finds all anchor tokens and their positions
  - Handles reordering (e.g., Japanese SOV → anchor positions differ)
- [ ] Implement `map_anchors_to_placeholders(anchor_positions: &[(String, usize)]) -> Vec<(usize, usize)>`
  - Maps anchor token positions to original $1, $2, $3... indices
  - Handles reordering: anchor `_ID2_` at pos 5 → `$2` at pos 5 in output
- [ ] Implement `reconstruct_with_placeholders(text: &str, anchor_map: &[(usize, usize)]) -> String`
  - Replaces anchors with proper $N placeholders in their new positions

**Tests**:
- [ ] Identity mapping: Japanese → English (no reordering)
  - Anchors in same order: `_ID1_ _ID2_` → `$1 $2`
- [ ] Reordering: English SOV → Japanese SVO
  - Source: `"$1 sent $2"` → `"_ID1_ sent _ID2_"`
  - Translated: `"_ID2_ は _ID1_ によって送信"`
  - Recovered: `"$2 は $1 によって送信"`
- [ ] Missing anchor: translated text missing an anchor token
  - Should fall back to original position
  - Warning: "Placeholder $N not found in translation"

**Files**:
- [ ] `src/mt/placeholder_recovery.rs` - Placeholder mapping logic

---

### Iteration 10: Consistency Checking (QA)

**Goal**: Validate translated variants for hallucinations and inconsistencies

**Tasks**:
- [ ] Implement `check_consistency(source_variants: &[String], translated_variants: &[String]) -> Vec<ConsistencyWarning>`
  - Detects if the same English words are translated differently across variants
  - Returns warnings for high variance
- [ ] Implement `check_anchor_preservation(variants: &[String], anchors: &[String]) -> Result<(), String>`
  - Ensures all anchor tokens are present in all variants
  - Detects missing or extra anchors
- [ ] Implement `check_scope_stability(variants: &[String]) -> Vec<ScopeWarning>`
  - Verifies that stable parts didn't change between variants
  - Example: If all variants should start with "La", but one starts with "Le" → warning

**Tests**:
- [ ] Consistency check: MT translates "apple" as "pomme" and "pomme" inconsistently
  - Should detect and warn
- [ ] Anchor preservation: all 4 variants have `_ID1_` and `_ID2_`
  - Should pass
- [ ] Missing anchor: one variant is missing `_ID2_`
  - Should fail with error
- [ ] Scope stability: French articles match across variants
  - Should pass

**Files**:
- [ ] `src/mt/consistency.rs` - Consistency validation

---

### Iteration 11: Suggestion Generator - Pipeline Orchestration (Integration)

**Goal**: Wire together all components into a complete pipeline

**Tasks**:
- [ ] Implement `TranslationSuggestion` struct:
  ```rust
  pub struct TranslationSuggestion {
      pub source_key: String,
      pub source_message: String,
      pub target_locale: String,
      pub suggested_wikitext: String,
      pub confidence: f32, // 0.0 to 1.0
      pub warnings: Vec<String>,
      pub variants_generated: usize,
      pub variants_translated: usize,
  }
  ```
- [ ] Implement `generate_suggestion(
    source_locale: &str,
    target_locale: &str,
    message_key: &str,
    message: &str,
    translator: &dyn MachineTranslator,
  ) -> Result<TranslationSuggestion>`
  - Orchestrates full pipeline:
    1. Parse wikitext → AST
    2. Expand variants with anchor tokens
    3. Check combinatorial explosion
    4. Translate all variants
    5. Check consistency
    6. Reassemble with scope widening
    7. Calculate confidence score
    8. Return suggestion with warnings
- [ ] Implement `generate_suggestions_batch(
    source_locale: &str,
    target_locale: &str,
    messages: &HashMap<String, String>,
    translator: &dyn MachineTranslator,
  ) -> Vec<Result<TranslationSuggestion>>`
  - Processes multiple messages efficiently

**Tests**:
- [ ] End-to-end: English greeting → French
  - `"Hello, $1!"` → `"Bonjour, $1!"`
  - Confidence: high
  - Warnings: none
- [ ] End-to-end: English with PLURAL → Russian
  - Complex expansion, multiple variants
  - Verify all variants translated
  - Verify reassembly correct
  - Verify confidence score lower due to complexity
- [ ] End-to-end: Message with scope expansion needed
  - Verify warnings generated
  - Verify suggestion still provided
- [ ] Error case: Too many variants (>64)
  - Should return error, not suggestion
- [ ] Error case: Consistency check fails
  - Should return error or low-confidence suggestion

**Files**:
- [ ] `src/mt/suggestion.rs` - Suggestion generator
- [ ] Update `src/lib.rs` - Export MT API

---

### Iteration 12: CLI Tool (User Interface)

**Goal**: Provide command-line interface for translators

**Tasks**:
- [ ] Create `src/bin/banana-i18n-mt.rs` - Separate binary
- [ ] Implement commands:
  - `banana-i18n-mt suggest <source-locale> <target-locale> <message-key> [message-json]`
  - `banana-i18n-mt suggest-file <source-locale> <target-locale> <json-file>`
  - `banana-i18n-mt batch <source-locale> <target-locale> <input-dir> <output-dir>`
- [ ] Features:
  - Load messages from JSON files (i18n directory)
  - Display suggestions with confidence scores
  - Show warnings (scope changes, high variance)
  - Optional: interactive mode to accept/reject/edit suggestions
  - Export as JSON or MediaWiki format
- [ ] Error handling:
  - Missing API key → clear error message
  - Invalid locale codes → suggest nearby locales
  - Network errors → helpful diagnostics

**Tests**:
- [ ] CLI help: `banana-i18n-mt --help` → shows usage
- [ ] Single message: `banana-i18n-mt suggest en fr greeting "Hello, $1!"`
  - → Displays suggestion with confidence and warnings
- [ ] File input: `banana-i18n-mt suggest-file en fr i18n/en.json`
  - → Processes all messages, outputs suggestions
- [ ] Error handling:
  - Missing `GOOGLE_TRANSLATE_API_KEY` → error message
  - Invalid locale → error message
  - Network timeout → error with retry suggestion

**Files**:
- [ ] `src/bin/banana-i18n-mt.rs` - CLI binary
- [ ] `src/mt/cli.rs` - CLI utilities (optional)

**Build**:
- [ ] Test: `cargo build --bin banana-i18n-mt`
- [ ] Run: `./target/debug/banana-i18n-mt --help`

---

## Cross-Cutting Concerns

### Error Handling Strategy
- Use `Result<T>` for all fallible operations
- Custom error type: `pub enum MtError { ParsingError, ExpansionError, TranslationError, ReassemblyError }`
- Graceful degradation: return best-effort suggestion with warnings rather than failing completely

### Testing Strategy
- **Unit tests** for each component (expansion, reassembly, consistency, etc.)
- **Integration tests** for full pipelines
- **Mock translator** for fast, deterministic testing
- **Optional integration tests** with real Google Translate (requires API key, skipped in CI)

### Performance Considerations
- Lazy parsing of wikitext (parse only when needed)
- Batch translation to reduce API calls (up to 128 items per batch)
- Cache expanded variants to avoid re-computation
- Parallel translation for multiple messages (optional, future)

### Documentation
- Add doc comments to all public types and functions
- Include examples in CLI help and Rust docs
- Document the Diff-and-Capture algorithm in detail
- Document anchor token design and tradeoffs

---

## Success Criteria

✅ **Iteration 1**: Anchor tokens work correctly for placeholder protection  
✅ **Iteration 2**: All PLURAL variants generated for English, Russian, French, Arabic  
✅ **Iteration 3**: All GENDER variants generated correctly  
✅ **Iteration 4**: Cartesian product respects 64-variant limit with warnings  
✅ **Iteration 5**: Mock translator enables fast testing  
✅ **Iteration 6**: Google Translate integration works (with real API testing)  
✅ **Iteration 7**: Structural alignment extracts diffs accurately  
✅ **Iteration 8**: Scope widening detects and handles agreement changes  
✅ **Iteration 9**: Placeholder recovery handles word order reordering  
✅ **Iteration 10**: Consistency checks detect hallucinations and anomalies  
✅ **Iteration 11**: End-to-end pipeline produces valid suggestions  
✅ **Iteration 12**: CLI tool is user-friendly and helpful  

---

## Future Enhancements (Out of Scope)

- [ ] Support for GRAMMAR magic word
- [ ] Real-time translation preview in UI
- [ ] Multiple MT provider support (AWS Translate, Azure, etc.)
- [ ] Machine learning model to detect scope changes automatically
- [ ] Parallel batch processing for large message sets
- [ ] Web UI for translator collaboration
- [ ] Integration with MediaWiki translation workflow
