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

📋 **Planning Phase Complete** ✅
- Algorithm analyzed
- 12-iteration roadmap created
- Architecture designed
- Test strategy defined

🔨 **Implementation Not Started**
- Ready for Iteration 1: Anchor tokens

## Design Highlights

### Anchor Tokens
Instead of directly translating `"$1 sent $2"`, we use:
```
"_ID1_ sent _ID2_"
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
├── IMPLEMENTATION_SUMMARY.md     # Architecture & examples
├── TODO.md                       # 12-iteration plan
├── README.md                     # This file
├── mod.rs                        # Module definition
└── [To be created in iterations 1-12]
```

## Next Steps

1. Review [TODO.md](./TODO.md) section "Iteration 1: Placeholder Expansion & Anchor Tokens"
2. Implement the anchor token module
3. Write unit tests
4. Move to Iteration 2

## Questions?

Refer to the implementation plan in [TODO.md](./TODO.md) or the architecture overview in [IMPLEMENTATION_SUMMARY.md](./IMPLEMENTATION_SUMMARY.md).

---

**Module Status**: 📋 Ready for Implementation  
**Estimated Total Size**: ~2,200 LOC  
**Estimated Timeline**: 4-6 weeks (12 iterations + testing)
