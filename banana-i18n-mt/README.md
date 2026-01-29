# Machine Translation Module for banana-i18n-rust

## Overview

This module implements machine translation suggestions for MediaWiki i18n messages. It intelligently handles complex wikitext features like PLURAL magic words, GENDER selections, and parameterized placeholders.

## Key Features

 **Smart Expansion** - Generates all combinations of PLURAL/GENDER forms
**Placeholder Protection** - Uses anchor tokens to prevent corruption during translation
**MT Provider Agnostic** - Generic trait system with Google Translate implementation
**Advanced Reassembly** - Reconstructs wikitext with grammatical agreement handling
**Consistency Checking** - Validates translations for hallucinations and anomalies
**CLI Tool** - Command-line interface for translator workflows

## Algorithm: MT-Assisted Wikitext Translation

### Overview

This module implements a 4-phase translation pipeline that solves the fundamental challenge of translating structured wikitext using plain-text machine translation APIs. The algorithm handles MediaWiki's PLURAL and GENDER magic words while preserving grammatical correctness in highly inflected languages.

**The Core Problem**: MT systems translate plain text only, but MediaWiki messages contain:
- Magic words: `{{PLURAL:$1|form1|form2}}`, `{{GENDER:$1|He|She|They}}`
- Placeholders: `$1`, `$2`, `$3`
- Links: `[[article]]`, `[http://url text]`

Additionally, inflected languages (French, German, Russian, Arabic) require seeing **complete sentences** to properly handle:
- Grammatical agreement (gender/number affecting verbs and adjectives)
- Vowel elision (French: "le" → "l'" before vowels)
- Case marking (Russian/German: noun forms change by sentence role)

**The Solution**: Expand all variant combinations → Translate in batch → Reassemble using structural alignment.

---

### Architecture Flow

```
┌─────────────────┐
│  Input Wikitext │  "{{GENDER:$1|He|She}} sent {{PLURAL:$2|a|$2}} message"
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Phase 1:       │  Generate all variants (3 GENDER × 2 PLURAL = 6)
│  EXPANSION      │  Replace $1→777001, $2→777002 (anchor tokens)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Phase 2:       │  Send batch to Google Translate API
│  TRANSLATION    │  Receive 6 translated French variants
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Phase 3:       │  Compare variants, extract differences
│  REASSEMBLY     │  Collapse axes: $2 → $1 → final wikitext
│                 │  Apply word boundary snapping
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Phase 4:       │  Restore placeholders: 777001→$1, 777002→$2
│  RECOVERY       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Output Wikitext │  "{{GENDER:$1|Il|Elle}} a envoyé {{PLURAL:$2|un|$2}} message"
└─────────────────┘
```

---

### Phase 1: Expansion - Cartesian Product Generation

**Goal**: Generate all possible sentence variants by expanding magic words into plain text with protected placeholders.

#### Algorithm

1. **Parse AST**: Identify all PLURAL and GENDER transclusions
2. **Determine Forms**: Query ICU CLDR for target language plural categories
   - English: 2 forms (one, other)
   - French: 2 forms (one, other)
   - Russian: 3 forms (one, few, many)
   - Arabic: 6 forms (zero, one, two, few, many, other)
3. **Generate Cartesian Product**: Create all state combinations
4. **Apply Anchor Tokens**: Replace `$N` with `777000+N` to protect from MT corruption

#### Detailed Example: English → French

**Input Message**:
```
"{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}"
```

**Step 1: Identify Magic Words**
```
GENDER:$1 → 3 options (male, female, unknown)
PLURAL:$2 → 2 options (one, other) for French target
```

**Step 2: Calculate Variant Count**
```
Total variants = 3 × 2 = 6
```

**Step 3: Generate State Combinations**
```
State 0: {$1: 0, $2: 0}  → GENDER=male,   PLURAL=one
State 1: {$1: 0, $2: 1}  → GENDER=male,   PLURAL=other
State 2: {$1: 1, $2: 0}  → GENDER=female, PLURAL=one
State 3: {$1: 1, $2: 1}  → GENDER=female, PLURAL=other
State 4: {$1: 2, $2: 0}  → GENDER=unknown, PLURAL=one
State 5: {$1: 2, $2: 1}  → GENDER=unknown, PLURAL=other
```

**Step 4: Resolve Each Variant with Anchor Tokens**

| Variant | State | Resolved Text |
|---------|-------|---------------|
| 0 | `{$1:0, $2:0}` | `"He sent a message"` |
| 1 | `{$1:0, $2:1}` | `"He sent 777002 messages"` |
| 2 | `{$1:1, $2:0}` | `"She sent a message"` |
| 3 | `{$1:1, $2:1}` | `"She sent 777002 messages"` |
| 4 | `{$1:2, $2:0}` | `"They sent a message"` |
| 5 | `{$1:2, $2:1}` | `"They sent 777002 messages"` |

**Why Anchor Tokens?**

Without protection:
```
"He sent $1 messages" → MT → "Il a envoyé 1 messages"
                                            ↑
                                    Corrupted: $1 became "1"
```

With anchor tokens:
```
"He sent 777001 messages" → MT → "Il a envoyé 777001 messages"
                                              ↑
                                      Preserved: 777001 unchanged
```

The MT system sees `777001` as a proper noun or identifier and preserves it.

---

### Phase 2: Translation - Batch MT Execution

**Goal**: Translate all 6 variants in a single API call to ensure consistency.

#### Algorithm

1. **Join with Numbering**: Create a single text block with numbered lines
   ```
   1. He sent a message
   2. He sent 777002 messages
   3. She sent a message
   4. She sent 777002 messages
   5. They sent a message
   6. They sent 777002 messages
   ```

2. **Send to MT API**: POST to Google Translate with source=en, target=fr

3. **Parse Response**: Split by numbered prefixes using regex `\d+\.\s`

4. **Validate Count**: Ensure 6 translations returned (match input count)

5. **Clean Anchor Mangling**: Some MT systems add spaces: `777 002` → `777002`

#### Translation Results (English → French)

| Variant | Source (English) | Target (French) |
|---------|------------------|-----------------|
| 0 | `"He sent a message"` | `"Il a envoyé un message"` |
| 1 | `"He sent 777002 messages"` | `"Il a envoyé 777002 messages"` |
| 2 | `"She sent a message"` | `"Elle a envoyé un message"` |
| 3 | `"She sent 777002 messages"` | `"Elle a envoyé 777002 messages"` |
| 4 | `"They sent a message"` | `"Ils ont envoyé un message"` |
| 5 | `"They sent 777002 messages"` | `"Ils ont envoyé 777002 messages"` |

**Consistency Benefit**: By translating in a batch, the MT system:
- Uses consistent vocabulary ("envoyé" in all variants)
- Maintains parallel structure
- Reduces hallucination risk

---

### Phase 3: Reassembly - Axis Collapsing Algorithm

**Goal**: Reconstruct wikitext by systematically identifying differences between translated variants.

This is the most sophisticated phase. It uses an **axis-collapsing** algorithm that processes each variable dimension independently, building up the wikitext structure incrementally.

#### Algorithm Overview

```
1. Consistency Guard: Check all variants have similarity > 70%
2. Determine Axes: Extract all variable IDs from first variant state
3. For each axis ($2, then $1):
   a. Group variants by all OTHER dimensions
   b. Within each group, collapse the current axis:
      - Extract LCP (Longest Common Prefix)
      - Extract LCS (Longest Common Suffix)
      - Snap to word boundaries
      - Wrap middle differences in wikitext syntax
   c. Replace group with single "virtual" variant containing wikitext
4. After all axes collapsed: one variant remains with full wikitext
```

#### Step-by-Step Walkthrough

**Initial State**: 6 fully translated French variants

```
Variant 0: state={$1:0, $2:0}, text="Il a envoyé un message"
Variant 1: state={$1:0, $2:1}, text="Il a envoyé 777002 messages"
Variant 2: state={$1:1, $2:0}, text="Elle a envoyé un message"
Variant 3: state={$1:1, $2:1}, text="Elle a envoyé 777002 messages"
Variant 4: state={$1:2, $2:0}, text="Ils ont envoyé un message"
Variant 5: state={$1:2, $2:1}, text="Ils ont envoyé 777002 messages"
```

**Axes to Collapse**: `[$2, $1]` (order doesn't matter for final result)

---

#### Collapse Axis $2 (PLURAL) First

**Step 1: Group by OTHER dimensions** (group by $1 values: 0, 1, 2)

**Group A: $1=0** (male gender variants)
```
Variant 0: state={$1:0, $2:0}, text="Il a envoyé un message"
Variant 1: state={$1:0, $2:1}, text="Il a envoyé 777002 messages"
```

**Group B: $1=1** (female gender variants)
```
Variant 2: state={$1:1, $2:0}, text="Elle a envoyé un message"
Variant 3: state={$1:1, $2:1}, text="Elle a envoyé 777002 messages"
```

**Group C: $1=2** (unknown gender variants)
```
Variant 4: state={$1:2, $2:0}, text="Ils ont envoyé un message"
Variant 5: state={$1:2, $2:1}, text="Ils ont envoyé 777002 messages"
```

---

**Step 2: Fold Each Group**

##### Folding Group A ($1=0):

**Input Texts**:
```
Text 0: "Il a envoyé un message"
Text 1: "Il a envoyé 777002 messages"
```

**Consistency Check**:
```
Similarity = high (most of text is identical, only "un" vs "777002 messages" differs)
           > 70% threshold ✓
```

**Extract LCP (Longest Common Prefix)**:
```
Raw LCP = "Il a envoyé "  (character-by-character comparison)
```

**Extract LCS (Longest Common Suffix)**:
```
Raw LCS = ""  (no common suffix: "message" vs "messages" differ)
```

**Word Boundary Snapping**:
```
Prefix check: "Il a envoyé " ends with space → OK, no adjustment needed
Suffix check: empty → no adjustment needed
```

**Extract Middles**:
```
Text 0 middle: "un message"
Text 1 middle: "777002 messages"
```

**Construct Wikitext**:
```
Tag type = variable_types[$2] = "PLURAL"
Result = "Il a envoyé {{PLURAL:$2|un message|777002 messages}}"
```

##### Folding Group B ($1=1):
```
Text 2: "Elle a envoyé un message"
Text 3: "Elle a envoyé 777002 messages"

LCP: "Elle a envoyé "
LCS: ""
Middles: "un message" | "777002 messages"

Result: "Elle a envoyé {{PLURAL:$2|un message|777002 messages}}"
```

##### Folding Group C ($1=2):
```
Text 4: "Ils ont envoyé un message"
Text 5: "Ils ont envoyé 777002 messages"

LCP: "Ils ont envoyé "
LCS: ""
Middles: "un message" | "777002 messages"

Result: "Ils ont envoyé {{PLURAL:$2|un message|777002 messages}}"
```

---

**Step 3: Create Virtual Variants After $2 Collapse**

```
Virtual 0: state={$1:0}, text="Il a envoyé {{PLURAL:$2|un message|777002 messages}}"
Virtual 1: state={$1:1}, text="Elle a envoyé {{PLURAL:$2|un message|777002 messages}}"
Virtual 2: state={$1:2}, text="Ils ont envoyé {{PLURAL:$2|un message|777002 messages}}"
```

Notice: $2 dimension eliminated, now only 3 variants remain.

---

#### Collapse Axis $1 (GENDER) Second

**Step 1: Group by OTHER dimensions** (no other dimensions, so one group)

**Single Group: All variants**
```
Virtual 0: state={$1:0}, text="Il a envoyé {{PLURAL:$2|un message|777002 messages}}"
Virtual 1: state={$1:1}, text="Elle a envoyé {{PLURAL:$2|un message|777002 messages}}"
Virtual 2: state={$1:2}, text="Ils ont envoyé {{PLURAL:$2|un message|777002 messages}}"
```

**Step 2: Fold the Group**

**Consistency Check**:
```
Similarity between all 3 variants > 70% ✓
(Only pronouns and verb forms differ, structure is consistent)
```

**Extract LCP**:
```
Virtual 0: "Il a envoyé {{PLURAL:$2|..."
Virtual 1: "Elle a envoyé {{PLURAL:$2|..."
Virtual 2: "Ils ont envoyé {{PLURAL:$2|..."

Raw LCP = "" (differs at first character: "I" vs "E" vs "I")
```

**Extract LCS**:
```
All three end with: " {{PLURAL:$2|un message|777002 messages}}"
Raw LCS = " {{PLURAL:$2|un message|777002 messages}}"
```

**Word Boundary Snapping**:
```
Prefix: "" → no adjustment needed
Suffix: " {{PLURAL:$2|..." starts with space → OK
```

**Extract Middles**:
```
Virtual 0 middle: "Il a envoyé"
Virtual 1 middle: "Elle a envoyé"
Virtual 2 middle: "Ils ont envoyé"
```

**Construct Wikitext**:
```
Tag type = variable_types[$1] = "GENDER"
Result = "{{GENDER:$1|Il a envoyé|Elle a envoyé|Ils ont envoyé}} {{PLURAL:$2|un message|777002 messages}}"
```

**Step 3: Final Virtual Variant**
```
Virtual Final: state={}, text="{{GENDER:$1|Il a envoyé|Elle a envoyé|Ils ont envoyé}} {{PLURAL:$2|un message|777002 messages}}"
```

---

#### Scope Widening Detection

**Original English Structure**:
```
"{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}"
      └─ Just pronouns        └─ verb + object
```

**Final French Structure**:
```
"{{GENDER:$1|Il a envoyé|Elle a envoyé|Ils ont envoyé}} {{PLURAL:$2|un message|777002 messages}}"
      └─ Pronouns + verb forms                    └─ Just object
```

**Analysis**: The algorithm detected that French requires the verb "envoyé" to agree with gender/number. It automatically **widened the GENDER scope** to include the verb phrase, ensuring grammatical correctness.

This is called **scope widening** and is critical for inflected languages where agreement marks appear outside the original magic word boundaries.

---

### Phase 4: Recovery - Placeholder Restoration

**Goal**: Convert anchor tokens back to MediaWiki placeholders.

#### Algorithm

```rust
Regex: r"777(\d+)"
Replacement: "$\1"

Example transformations:
  "777001" → "$1"
  "777002" → "$2"
  "777010" → "$10"
```

#### Final Output

**Before Recovery**:
```
"{{GENDER:$1|Il a envoyé|Elle a envoyé|Ils ont envoyé}} {{PLURAL:$2|un message|777002 messages}}"
```

**After Recovery**:
```
"{{GENDER:$1|Il a envoyé|Elle a envoyé|Ils ont envoyé}} {{PLURAL:$2|un message|$2 messages}}"
```

This is the final suggested translation for the French translator to review!

---

### Key Design Decisions

#### 1. Anchor Token Design (777xxx)

**Requirements**:
- Must be numeric (easy to pattern match)
- Must be unlikely in natural text
- Must be preserved by MT systems
- Must support sequential placeholders ($1, $2, ..., $10+)

**Choice**: `777000 + N`
- `777` prefix: Rarely appears in text, memorable
- Numeric range: `777001` to `777999` (supports 999 placeholders)
- MT behavior: Treated as identifier/proper noun, preserved

#### 2. Similarity Threshold (70%)

**Tested Values**:
- **50%**: Too permissive, accepts corrupted translations
- **70%**: Balanced, rejects hallucinations while allowing grammatical changes
- **90%**: Too strict, rejects valid inflections in Slavic/Arabic languages

**Implementation**: LCS-based similarity ratio = `2 * LCS / (len_a + len_b)`

#### 3. Word Boundary Snapping

**Problem Without Snapping**:
```
"He sent" vs "She sent"
LCP = "He s" / "She s"  (raw character matching)
Result: "{{GENDER:$1|He s|She s}}ent"  ← Broken!
```

**Solution**:
```
If LCP doesn't end with space: snap back to last space
If LCS doesn't start with space: snap forward to first space

Result: "{{GENDER:$1|He|She}} sent"  ← Correct!
```

#### 4. Cartesian Expansion Limit (64 variants)

**Rationale**:
- Most real messages: 2-8 variants (1-2 magic words)
- Practical limit: 6 binary choices = 2^6 = 64
- MT API limits: Most services handle ~100 strings per batch
- Memory/performance: 64 variants × 200 chars = ~13KB per message

**Error Handling**: If exceeded, return `MtError::ExpansionError` with guidance to simplify message.

#### 5. Batch Translation Strategy

**Why Batch?**
- **Consistency**: MT uses same vocabulary across variants
- **Efficiency**: 1 API call instead of N calls
- **Context**: MT sees related sentences, improves quality

**Implementation**: Join with numbered prefixes, split on return

#### 6. Axis Collapsing Order Independence

**Property**: The order of collapsing axes doesn't affect final result.

**Example**: Collapsing $1 then $2 produces the same wikitext as $2 then $1.

**Why**: Each collapse is a pure function of the variant texts, not dependent on previous collapses.

---

### Error Handling

#### Consistency Errors

**Trigger**: Similarity < 70% between variants in same group

**Cause**: MT hallucination, structural changes, random synonyms

**Example**:
```
English:  "He sent" / "She sent"  (very similar)
Bad MT:   "Il a envoyé" / "Message complètement différent"  (too different)
```

**Response**: Return `MtError::ConsistencyError` with details

#### Expansion Errors

**Trigger**: Variant count > 64

**Cause**: Too many magic words or complex plural forms

**Solution**: Simplify message or split into multiple messages

#### Translation Errors

**Trigger**: Network failure, API rate limit, invalid API key

**Response**: Propagate error from MT provider with context

---

### Why This Approach Works

#### Compared to Naive Word-by-Word Translation

| Feature | Naive Approach | Cartesian Expansion |
|---------|---------------|---------------------|
| **Gender Agreement** | ❌ Translates "He" and "sent" separately | ✅ Sees "He sent" as complete sentence |
| **Vowel Elision** | ❌ "le apple" (incorrect) | ✅ "l'apple" (MT handles naturally) |
| **Case Marking** | ❌ Wrong cases in German/Russian | ✅ MT determines correct case from context |
| **Consistency** | ❌ "message" vs "pli" in different forms | ✅ Batch translation ensures consistency |
| **Structure** | ❌ Often breaks wikitext | ✅ Reconstructs structure reliably |

#### Trade-offs

**Advantages**:
- Linguistically robust for all language families
- Preserves grammatical correctness
- Handles scope widening automatically
- Provides consistency guarantees

**Disadvantages**:
- More complex than direct translation
- Requires multiple API calls (mitigated by batching)
- Assumes MT consistency within batch (generally true)
- Limited to 64 variants (rarely an issue in practice)

---

## Quick Start

### Understanding the Pipeline

Read the **Algorithm** section above to understand how the 4-phase translation pipeline works, with detailed English → French examples showing all 6 variants being expanded, translated, reassembled, and recovered.

### Example Usage

```bash
# Translate a single message with mock translator
cargo run --bin banana-mt -- --mock --verbose "{{GENDER:\$1|He|She}} sent a message" fr

# Translate with Google Translate (requires API key)
export GOOGLE_TRANSLATE_API_KEY="your-api-key-here"
cargo run --bin banana-mt -- --verbose "{{GENDER:\$1|He|She}} sent a message" fr
```

### Set API Key for Google Translate

```bash
export GOOGLE_TRANSLATE_API_KEY="your-api-key-here"
```

---

