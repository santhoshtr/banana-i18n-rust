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

### Iteration 4: Expansion Engine - Cartesian Product (Complex) ✅ COMPLETE

**Goal**: Generate all combinations of PLURAL × GENDER variants

**Tasks**:
- [x] Implement `expand_all_variants(ast: &AstNodeList, target_locale: &str) -> Result<Vec<String>>`
  - Takes source wikitext and target locale
  - Generates Cartesian product of PLURAL and GENDER
  - Returns vector of all plain-text variants with anchor tokens
  - **Limit**: Max 64 variants, returns error if exceeded
- [x] Implement `calculate_variant_count(ast: &AstNodeList, target_locale: &str) -> usize`
  - Predicts number of variants before expansion

**Tests Completed**:
- [x] Simple message: `"Hello, $1"` → 1 variant
- [x] PLURAL only: `{{PLURAL:$1|is|are}}` → 2 variants (English)
- [x] GENDER only: `{{GENDER:$1|He|She|They}}` → 3 variants
- [x] PLURAL × GENDER: Both → 3 × 2 = 6 variants (English)
- [x] Russian expansion: 3 GENDER × 3 PLURAL = 9 variants
- [x] **Limit at max**: 2^6 = 64 variants → succeeds
- [x] **Limit exceeded**: 2^7 = 128 variants → returns error
- [x] Variant count calculation matches actual expansion
- [x] Anchor tokens applied to all variants
- [x] Complex messages with links and placeholders

**Files Created**:
- [x] `src/mt/expansion.rs` - Cartesian product logic (580 LOC)

**Test Results**: 15 new tests, all passing. Total: 133/133 tests passing

---

### Iteration 5: MT Trait & Mock Implementation (Infrastructure) ✅ COMPLETE

**Goal**: Define trait and implement mock MT provider for testing

**Tasks Completed**:
- [x] Define `MachineTranslator` trait (async-based):
  ```rust
  #[async_trait]
  pub trait MachineTranslator: Send + Sync {
      async fn translate(&self, text: &str, source: &str, target: &str) -> MtResult<String>;
      async fn translate_batch(&self, texts: &[String], source: &str, target: &str) -> MtResult<Vec<String>>;
      fn provider_name(&self) -> &str;
  }
  ```
- [x] Implement `MockTranslator` with 5 modes:
  - `Suffix`: Append locale suffix
  - `Mappings`: Use predefined translation map
  - `Reorder`: Reverse word order for SOV languages
  - `Error`: Simulate API failures
  - `NoOp`: Return input unchanged
- [x] Add simulated network delay support
- [x] Implement helper functions:
  - `normalize_locale()`: Convert "en-US" → "en"
  - `validate_locale()`: Validate locale code format
- [x] Add trait exports to lib.rs

**Tests Completed**: 22 async tests
- [x] All 5 MockMode variants tested
- [x] Batch translation support
- [x] Anchor token preservation
- [x] Simulated network delays
- [x] Error handling modes

**Files Created**:
- [x] `src/mt/translator.rs` - Trait definition and helpers (250 LOC)
- [x] `src/mt/mock.rs` - Mock implementation (350 LOC)

**Test Results**: 22 new tests, all passing


---

### Iteration 6: Google Translate Provider (Real MT) ✅ COMPLETE

**Goal**: Implement real Google Translate API integration

**Tasks Completed**:
- [x] Add dependencies:
  - `tokio` (async runtime)
  - `reqwest` (HTTP client)
  - `async-trait` (async trait support)
- [x] Implement `GoogleTranslateProvider` struct:
  - Load API key from `GOOGLE_TRANSLATE_API_KEY` env var
  - Implement `MachineTranslator` trait with async methods
  - Support batch translation (up to 128 items per request)
- [x] Automatic batch chunking:
  - User provides Vec[100] → provider chunks internally → returns Vec[100]
  - Transparent to user
- [x] Error handling:
  - `ConfigError` for missing/invalid API key
  - `NetworkError` for HTTP/connection issues
  - `InvalidLocale` for invalid locale codes
- [x] Validation:
  - Validate locale codes before API call
  - Check text length limits (30,000 chars per string)
  - Check batch size (128 items max per request)
- [x] Debug output with API key masking

**Tests Completed**: 17 tests
- [x] 5 unit tests for initialization and validation (no API required)
- [x] 7 unit tests for chunking logic and error handling
- [x] 5 integration tests marked `#[ignore]` (requires real API key)

**Test Results**: 17 new tests, 12 passing (5 integration tests ignored), 0 failures

**Files Created**:
- [x] `src/mt/google_translate.rs` - Google Translate provider (400 LOC)

**Dependencies Added**:
- [x] `tokio = { version = "1", features = ["rt-multi-thread", "macros"] }`
- [x] `reqwest = { version = "0.12", features = ["json"] }`
- [x] `async-trait = "0.1"`

**How to Run Integration Tests**:
```bash
# Set your API key
export GOOGLE_TRANSLATE_API_KEY="your-key-here"

# Run integration tests
cargo test --lib google_translate -- --ignored --nocapture
```

---

### Iteration 7: Reassembly Engine - Structural Alignment & Scope Widening ✅ COMPLETE

**Goal**: Extract diffs from translated variants and reconstruct wikitext with scope detection

**Tasks Completed**:
- [x] Implement `find_stable_and_variable_parts()` - Character-level comparison to identify unchanging text
- [x] Implement `map_variable_parts_to_ast()` - Map variable parts to source AST magic word positions
- [x] Implement `detect_scope_changes()` - Compare source and translated variants to find unexpected changes
- [x] Implement `reconstruct_wikitext()` - Rebuild wikitext with {{PLURAL|...}} and {{GENDER|...}} syntax
- [x] Implement `calculate_confidence()` - Score reassembly quality based on scope changes
- [x] Implement `generate_warnings()` - Create user-facing messages about scope changes

**Core Functions Created**:
- `reassemble()` - Main entry point orchestrating all steps
- `find_stable_and_variable_parts()` - Character-by-character alignment
- `map_variable_parts_to_ast()` - AST mapping
- `detect_scope_changes()` - Scope expansion detection
- `reconstruct_wikitext()` - Wikitext reconstruction with magic words
- `calculate_confidence()` - Confidence scoring (0.0-1.0)

**Helper Functions Created** (in scope_widening.rs):
- `find_continuous_changes()` - Find continuous change blocks
- `calculate_expanded_scope()` - Calculate minimal scope encompassing changes
- `expand_to_word_boundaries()` - Expand ranges to word boundaries

**Data Structures**:
- `ReassemblyResult` - Complete result with wikitext, forms, scope changes, warnings, confidence
- `ExtractedForms` - Extracted forms for each magic word
- `ScopeChange` - Details of scope expansion
- `Alignment` - Internal alignment information
- `StablePart` / `VariablePart` - Segment types

**Tests Completed**: 17 tests
- [x] find_stable_parts tests (single/identical/empty variants)
- [x] find_change_ranges tests (simple/multiple changes)
- [x] confidence scoring tests (with/without scope changes)
- [x] scope detection tests
- [x] warning generation tests
- [x] scope_widening helper tests (9 additional)

**Test Results**: 17 new tests, all passing. Total: 197/197 tests passing

**Files Created**:
- [x] `src/mt/reassembly.rs` - Core reassembly logic (530 LOC)
- [x] `src/mt/scope_widening.rs` - Scope detection helpers (130 LOC)

**Key Design Decisions**:
1. **All consecutive changes included**: When MT changes multiple words, all consecutive words are included in scope
2. **Fail fast on inconsistency**: If inconsistencies detected, error is returned (not warnings)
3. **Placeholder recovery**: In this iteration, anchor tokens are preserved in output. Full placeholder recovery in Iteration 9
4. **Confidence scoring**: Deduction of 0.1 per scope change (max 1.0, min 0.0)

**Algorithm Notes**:
- Uses character-level comparison for alignment (O(n) where n is max variant length)
- Detects scope changes by comparing source vs translated difference ranges
- Scope expansion includes all consecutive changed characters

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
✅ **Iteration 5**: MT trait defined and MockTranslator enables fast async testing  
✅ **Iteration 6**: Google Translate integration works (with real API testing available)  
✅ **Iteration 7**: Reassembly engine reconstructs wikitext with scope detection  
⏳ **Iteration 8**: Advanced placeholder recovery handles word order reordering  
⏳ **Iteration 9**: Consistency checks detect hallucinations and anomalies  
⏳ **Iteration 10**: End-to-end pipeline produces valid suggestions  
⏳ **Iteration 11**: CLI tool is user-friendly and helpful  
⏳ **Iteration 12**: Full integration and documentation complete  

---

## Future Enhancements (Out of Scope)

- [ ] Support for GRAMMAR magic word
- [ ] Real-time translation preview in UI
- [ ] Multiple MT provider support (AWS Translate, Azure, etc.)
- [ ] Machine learning model to detect scope changes automatically
- [ ] Parallel batch processing for large message sets
- [ ] Web UI for translator collaboration
- [ ] Integration with MediaWiki translation workflow
