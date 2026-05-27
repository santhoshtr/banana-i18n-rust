# MT-Assisted Localization Algorithm

This document describes the algorithm used in `banana-i18n-mt` to produce
machine-translation suggestions for MediaWiki-style wikitext messages. It is a
detailed companion to the Python reference (`mediawiki_mt_assisted_localization.py`
at the repository root) and the Rust implementation in `banana-i18n-mt/src/`.

The README in `banana-i18n-mt/` gives a high-level tour. This document goes
deeper: it walks every phase, names the data structures, shows worked
examples in multiple language pairs, and ends with a section listing the
loopholes and bugs that we already know about and intend to revisit.

---

## 1. The problem

MediaWiki messages are *not* plain text. A single message can contain:

| Construct       | Example                                               |
|-----------------|-------------------------------------------------------|
| Placeholder     | `$1`, `$2`, `$10`                                     |
| PLURAL          | `{{PLURAL:$1|one item|$1 items}}`                     |
| GENDER          | `{{GENDER:$2|He|She|They}}`                           |
| Internal link   | `[[Main Page|home]]`                                  |
| External link   | `[https://example.org Example]`                       |

A generic machine-translation engine (Google Translate, NLLB, etc.) only
accepts plain text. Naïve approaches break down quickly:

1. **Translate the whole wikitext as one string** — the MT engine mangles
   `{{...|...|...}}` and `$1`, the structure is lost, output is unusable.
2. **Translate each PLURAL/GENDER option independently** — the MT engine
   sees fragments like `"He"` and `"sent a message"` separately and cannot
   apply grammatical agreement (gender-marked verbs in French, case in
   Russian/German, vowel elision in French, etc.).
3. **Pre-substitute and translate per state** — better, but if you do
   `N` independent calls the MT engine drifts: `"message"` becomes
   `"message"` in one variant and `"pli"` in another, breaking
   reassembly.

The algorithm implemented here solves the problem with a four-phase
pipeline:

```
┌───────────────┐   ┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ 1. EXPANSION  │ → │ 2. TRANSLATE  │ → │ 3. REASSEMBLE │ → │ 4. RECOVER    │
│               │   │  (as block)   │   │               │   │ placeholders  │
└───────────────┘   └───────────────┘   └───────────────┘   └───────────────┘
```

The high-level shape (and most of the prose below) follows the Python
reference. The Rust port keeps the same data flow but uses the project's
real AST (`banana_i18n::ast::AstNode`) and adds ICU plural rules,
anchor-token bookkeeping, and stricter limits.

---

## 2. Terminology

| Term              | Meaning                                                                                   |
|-------------------|-------------------------------------------------------------------------------------------|
| **AST**           | The parsed message; a `Vec<AstNode>` (`AstNodeList`) where each node is `Text`, `Placeholder`, `Transclusion` (PLURAL/GENDER), `InternalLink`, or `ExternalLink`. |
| **Magic word**    | A transclusion whose `name` is `PLURAL` or `GENDER` (case-insensitive).                   |
| **Variant**       | One concrete realisation of the message after picking one option per magic word.          |
| **State**         | Map from variable id (`"$1"`, `"$2"`) to chosen option index. Each variant has one state. |
| **Axis**          | A single variable id treated as a dimension of the cartesian product.                     |
| **Anchor token**  | The decimal integer `777_000 + N` substituted for placeholder `$N` before MT.             |
| **LCP / LCS**     | Longest Common Prefix / Longest Common Suffix across a group of strings.                  |
| **Word boundary snap** | Adjusting LCP/LCS so they end / begin on an ASCII space — never inside a word.       |
| **Fold**          | Collapse a group of variants into one string by extracting LCP, LCS, and a magic word.    |

---

## 3. Data structures

The Rust types live in `banana-i18n-mt/src/data.rs`:

```rust
pub struct TranslationVariant {
    pub state: HashMap<String, usize>,  // {"$1": 0, "$2": 1}
    pub source_text: String,            // "777001 sent 777002 messages"
    pub translated_text: String,        // filled in after MT
}

pub struct MessageContext {
    pub original_key: String,                       // identifier for logging
    pub variable_types: HashMap<String, String>,    // {"$1": "GENDER", "$2": "PLURAL"}
    pub variants: Vec<TranslationVariant>,          // cartesian product
}
```

This is intentionally a 1:1 mapping of the Python dataclasses
`TranslationVariant` and `MessageContext`. Keeping them aligned makes it
easy to port fixes between the two implementations.

---

## 4. Phase 1 — Expansion

Source code: `banana-i18n-mt/src/expansion.rs`.

Entry point: `prepare_for_translation(ast, locale, message_key)` →
`MessageContext`.

### 4.1 Steps

1. **Walk the AST** and `collect_choices()`:
   for every `Transclusion` whose `name.to_uppercase()` is `PLURAL` or
   `GENDER`, record `(var_id, magic_type, option_count)`.
   - For `PLURAL`, `option_count` is the *target language's* plural form
     count, looked up via ICU (`get_plural_forms_for_language(locale)`).
   - For `GENDER`, `option_count` is hard-coded to 3 (male / female /
     unknown).
2. **Capacity check**: compute `Π option_count` and reject if it exceeds
   `MAX_VARIANTS = 64` (`MtError::ExpansionError`).
3. **Cartesian product**: generate every combination of indices, one per
   choice, as `HashMap<String, usize>`.
4. **Resolve** each state to a concrete `source_text` by walking the AST:
   - `Text` → emit as-is.
   - `Placeholder { index }` → emit the anchor `(777_000 + index).to_string()`.
   - `Transclusion(PLURAL|GENDER)` → look up `state[var_id]`, clamp to
     `min(idx, options.len()-1)`, emit the chosen option *with $N inside
     it also rewritten to anchors*.
   - `InternalLink` / `ExternalLink` → emit the wiki syntax verbatim so
     the MT engine sees what looks like ordinary punctuation.
5. **Populate** the `MessageContext`: fill in `variable_types` (so
   reassembly knows whether `$1` was PLURAL or GENDER) and the list of
   variants.

### 4.2 Anchor tokens

Why `777_000 + N`?

- Numeric ⇒ MT systems leave it alone (treated like a year or product
  code), unlike `$1` which is frequently dropped or translated as `1`.
- Prefix `777` is rare in real prose.
- Range `777001 … 777999` covers every realistic placeholder count.
- Round-trip is a single regex: `s/777(\d+)/\$\1/`.

Trade-offs and known cracks in this scheme are documented in §9.

### 4.3 Worked example — English source

Input wikitext:

```
{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}
```

Target language: French (2 plural forms).

Choices found:

```
$1 → GENDER, 3 options
$2 → PLURAL, 2 options (French has [one, other])
```

Total variants: `3 × 2 = 6`. Cartesian product (state → resolved text):

| # | `$1` | `$2` | `source_text`                       |
|---|------|------|-------------------------------------|
| 0 | 0    | 0    | `He sent a message`                 |
| 1 | 0    | 1    | `He sent 777002 messages`           |
| 2 | 1    | 0    | `She sent a message`                |
| 3 | 1    | 1    | `She sent 777002 messages`          |
| 4 | 2    | 0    | `They sent a message`               |
| 5 | 2    | 1    | `They sent 777002 messages`         |

Note that `$1` appears *only* as the GENDER selector here, not as a
placeholder, so no `777001` shows up in the text. `$2` is referenced
inside the PLURAL option (`"$2 messages"`) and therefore is anchored.

### 4.4 Worked example — Russian target (3 plural forms)

Same source. Target language: Russian (3 plural forms: *one*, *few*,
*many*).

Choices: `$1 → 3, $2 → 3`. Total variants: `3 × 3 = 9`.

If the source only supplies two PLURAL options (`a message` and
`$2 messages`), the resolver clamps `state["$2"] = 2` to the last
available option, so variants 6–8 reuse the `"$2 messages"` branch:

| # | `$1` | `$2` | `source_text`                  |
|---|------|------|--------------------------------|
| 0 | 0    | 0    | `He sent a message`            |
| 1 | 0    | 1    | `He sent 777002 messages`      |
| 2 | 0    | 2    | `He sent 777002 messages`      |
| 3 | 1    | 0    | `She sent a message`           |
| 4 | 1    | 1    | `She sent 777002 messages`     |
| 5 | 1    | 2    | `She sent 777002 messages`     |
| 6 | 2    | 0    | `They sent a message`          |
| 7 | 2    | 1    | `They sent 777002 messages`    |
| 8 | 2    | 2    | `They sent 777002 messages`    |

Variants 1/2, 4/5, 7/8 are duplicates *in the source*. This is the cost
of pre-expanding for the target's plural categories: the source can't
distinguish *few* vs *many*. The reassembler then sees identical pairs
in the MT output too, and the fold step (§6) collapses them.

This expand-then-collapse-duplicates behaviour is by design — it lets
the translator widen `{{PLURAL:$2|...|...}}` into three Russian forms
even though the English source only had two.

### 4.5 No magic words

If `collect_choices` returns empty, expansion produces a single variant
with state `{}` and the AST rendered as anchor-bearing plain text. The
reassembly phase short-circuits on `variants.len() == 1` and just
restores placeholders.

---

## 5. Phase 2 — Block translation

Source code: `banana-i18n-mt/src/google_translate.rs::translate_as_block`,
plus the more general `MachineTranslator` trait in `translator.rs`.

### 5.1 Why a block?

Calling MT once per variant is correct but cheap consistency dies: the
engine picks different vocabulary for the same word in different calls.
The remedy is to send all variants in one request so the engine sees
the related sentences side by side. Two-line transcripts of "the same
sentence with one word swapped" force consistency far more strongly
than independent calls.

### 5.2 Encoding

Variants are joined into one block with one-based numbering:

```
1. He sent a message
2. He sent 777002 messages
3. She sent a message
...
6. They sent 777002 messages
```

After translation, the response is split on the regex `\n?\d+\.\s`,
empty fragments are dropped, and the count is checked against the
input.

Cleanup pass: replace `"777 "` with `"777"` and `" 777"` with `"777"`
to fix MTs that insert a thin space inside long numbers. Other
mangling (Devanagari digits, Arabic-Indic digits, line reordering) is
*not* repaired — see §9.

### 5.3 Mismatched count

If the split produces a different count than the input, the function
returns `MtError::TranslationError`. There is no fallback to
per-variant translation.

### 5.4 Translator backends

The trait `MachineTranslator` (`translator.rs`) is the abstraction:

- `GoogleTranslateProvider` — real Google Translate v2 API, batches
  capped at 128, per-text cap at 30 000 chars.
- `MockTranslator` — used in tests; modes `Suffix`, `Mappings`,
  `Reorder`, `Error`, `NoOp`.

`translate_as_block` is a method on `GoogleTranslateProvider`; the
trait does not require it, so other providers reuse `translate_batch`
unless they want their own block-translation strategy.

---

## 6. Phase 3 — Reassembly (axis collapsing)

Source code: `banana-i18n-mt/src/reassembly.rs`.

Entry point: `Reassembler::new(variable_types).reassemble(variants)`.

### 6.1 The idea

After Phase 2 we have `N` translated variants. We want one wikitext
string back. Reassembly *collapses* the variant grid one axis (one
variable) at a time.

```
6 variants in a 3×2 grid       3 variants                1 variant
($1 × $2)                      ($1 only)                 ()
─────────────────────  ──────────────────────────  ────────────────────
[ a₀,₀ ][ a₀,₁ ]                                          ┌───────────┐
[ a₁,₀ ][ a₁,₁ ]    ─►   [ A₀ ][ A₁ ][ A₂ ]        ─►   │   FINAL   │
[ a₂,₀ ][ a₂,₁ ]                                          └───────────┘
            collapse $2          collapse $1
```

Each collapse step groups by every *other* dimension, then folds the
group into a single string using LCP/LCS extraction. After all axes
are gone, a single string remains; placeholder recovery (Phase 4)
restores `$N`.

### 6.2 `collapse_axis(variants, axis)`

```
1. groups = {}
2. for v in variants:
       key = tuple of (other_dim_id, other_dim_value) sorted lexicographically
       groups[key].push(v)
3. for (key, group) in groups:
       sort group by group[i].state[axis]   # canonical option order
       folded = fold_strings(group, axis)
       emit new TranslationVariant {
           state = key as map,
           source_text = "",                 # not needed downstream
           translated_text = folded,
       }
```

The sort by `state[axis]` matters: the fold preserves the option order
left-to-right inside the resulting `{{TAG:VAR|opt0|opt1|...}}`.

### 6.3 `fold_strings(group, var_id)`

This is the heart of the algorithm.

1. **Early exits**
   - `group.len() ≤ 1` → return the only text (no magic word needed).
   - All texts identical → return that text (no magic word needed).

2. **Consistency guard**
   For every `texts[i] (i ≥ 1)` compute `get_similarity(texts[0],
   texts[i])`. If any pair scores below `CONSISTENCY_THRESHOLD = 0.7`
   return `MtError::ConsistencyError`. This is the "hallucination
   detector": when the MT rewrites a sentence entirely for one variant,
   LCS-based similarity collapses below 70%.

3. **LCP / LCS extraction**
   - `raw_prefix = get_lcp(texts)` — character-by-character longest
     common prefix.
   - `raw_suffix = get_lcs(texts)` — reverse the strings, take LCP,
     reverse back.

4. **Word boundary snap**
   - If `raw_prefix` doesn't end with `' '`, snap **back** to the last
     space; if no space found, the prefix is empty.
   - If `raw_suffix` doesn't start with `' '`, snap **forward** to the
     first space; if no space found, the suffix is empty.

   This prevents the classic bug:
   ```
   "He sent…" vs "She sent…"
   raw_prefix = "He s" / "She s"  (matches first three chars by chance)
   without snap: {{GENDER:$1|He s|She s}}ent  ← word "sent" sliced
   with snap:    {{GENDER:$1|He|She}} sent
   ```

5. **Middle extraction**
   For each text, `middle = text[prefix.len() .. text.len() − suffix.len()]`.
   If indices overlap (prefix + suffix > text), middle is empty.

6. **Wikitext synthesis**
   ```
   tag = variable_types.get(var_id).unwrap_or("PLURAL")
   return "{prefix}{{{{{tag}:{var_id}|{middles[0]}|{middles[1]}|...}}}}{suffix}"
   ```

### 6.4 Similarity

`get_similarity(a, b)` returns `2·|LCS(a,b)| / (|a| + |b|)`, mirroring
Python's `difflib.SequenceMatcher.ratio()`. Edge cases:

- `a == b` → 1.0
- both empty → 1.0
- one empty → 0.0

The LCS itself is computed with O(m·n) dynamic programming on
character vectors.

### 6.5 Worked example — collapse $2 then $1, English → French

Input variants (after MT):

| # | state          | translated_text                       |
|---|----------------|---------------------------------------|
| 0 | `{$1:0,$2:0}`  | `Il a envoyé un message`              |
| 1 | `{$1:0,$2:1}`  | `Il a envoyé 777002 messages`         |
| 2 | `{$1:1,$2:0}`  | `Elle a envoyé un message`            |
| 3 | `{$1:1,$2:1}`  | `Elle a envoyé 777002 messages`       |
| 4 | `{$1:2,$2:0}`  | `Ils ont envoyé un message`           |
| 5 | `{$1:2,$2:1}`  | `Ils ont envoyé 777002 messages`      |

**Collapse `$2`**: group by `$1`.

- Group `$1=0` → folds to
  `Il a envoyé {{PLURAL:$2|un message|777002 messages}}`
  (LCP `"Il a envoyé "`, LCS `""`, middles `"un message"` / `"777002 messages"`).
- Group `$1=1` → `Elle a envoyé {{PLURAL:$2|un message|777002 messages}}`.
- Group `$1=2` → `Ils ont envoyé {{PLURAL:$2|un message|777002 messages}}`.

**Collapse `$1`**: one group of three.

- All three end with ` {{PLURAL:$2|un message|777002 messages}}` →
  raw LCS captures the whole tail.
- Raw LCP is empty (`I` vs `E`).
- Snap: prefix stays empty; suffix already starts with `' '`, kept.
- Middles: `"Il a envoyé"`, `"Elle a envoyé"`, `"Ils ont envoyé"`.
- Result:
  `{{GENDER:$1|Il a envoyé|Elle a envoyé|Ils ont envoyé}} {{PLURAL:$2|un message|777002 messages}}`.

This is **scope widening**: the source had `GENDER` selecting only the
pronoun, but French requires the verb to agree, so `envoyé` ended up
inside the GENDER tag — automatically. The LCP/LCS algorithm finds the
maximal stable surroundings; whatever is unstable across genders gets
absorbed into the magic word.

### 6.6 Worked example — identical translations

If the MT happens to produce two identical strings for a PLURAL pair
(e.g. when source and target both use the same form for "1 X"
and "0 X"), `fold_strings` sees `all_same == true` and returns the
single text *without* a PLURAL wrapper. The reassembled wikitext then
has fewer magic words than the source. This is acceptable for
correctness (the rendered message is identical) but means the
round-trip is not always idempotent.

### 6.7 Axis order independence

`reassemble` iterates `variants[0].state.keys()` to pick the axis
order. `HashMap` iteration order is not deterministic, so the order
varies run-to-run. The final string is *almost always* identical
regardless: each fold is a pure function of its inputs. The exception
is when LCP/LCS choices differ between orderings; see §9 for one
such case.

---

## 7. Phase 4 — Placeholder recovery

Source code: `Reassembler::restore_placeholders` in
`banana-i18n-mt/src/reassembly.rs`.

A single regex substitution:

```rust
// 777(\d+)  →  $<int(\1)>
re.replace_all(text, |caps| {
    let n: usize = caps[1].parse().unwrap();
    format!("${}", n)
})
```

Examples:

| Before                 | After          |
|------------------------|----------------|
| `777001`               | `$1`           |
| `777010`               | `$10`          |
| `un message`           | `un message`   |
| `the year 777 BC`      | `the year 777 BC` (no digits after 777, no match) |

The `\d+` is greedy: see §9 for the edge cases this opens up.

---

## 8. End-to-end Rust usage

```rust
use banana_i18n::parser::Parser;
use banana_i18n_mt::{
    prepare_for_translation, Reassembler, GoogleTranslateProvider,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse the wikitext into an AST
    let mut parser = Parser::new(
        "{{GENDER:$1|He|She|They}} sent {{PLURAL:$2|a message|$2 messages}}"
    );
    let ast = parser.parse();

    // 2. Expand to all variants for the target language
    let mut ctx = prepare_for_translation(&ast, "fr", "demo-message")?;

    // 3. Block-translate
    let provider = GoogleTranslateProvider::from_env()?;
    let translated = provider
        .translate_as_block(&ctx.source_texts(), "en", "fr")
        .await?;
    ctx.update_translations(translated);

    // 4. Reassemble and recover placeholders
    let wikitext = Reassembler::new(ctx.variable_types.clone())
        .reassemble(ctx.variants)?;

    println!("{wikitext}");
    Ok(())
}
```

A CLI driver wired up the same way lives in
`banana-i18n-mt/src/bin/banana-mt.rs` (`cargo run --bin banana-mt -- …`).

---

## 9. Known loopholes and bugs (revisit later)

These are the things we know are wrong, brittle, or under-specified.
None of them block the happy path, but each is worth a follow-up
investigation.

### 9.1 LCP/LCS uses byte length as a character iteration bound — FIXED

> **Status:** Fixed. `get_lcp` now operates on `Vec<char>` with
> character-space bounds and indexing. `get_lcs` benefits transitively
> (it reverses characters and delegates to `get_lcp`). Multi-byte
> regression tests live in
> `banana-i18n-mt/src/reassembly.rs::tests` under the
> "Multi-byte UTF-8 LCP/LCS Tests" section, plus the end-to-end
> `test_reassemble_russian_gender_variants`.

**Original bug.** In the old `get_lcp` (`reassembly.rs`):

```rust
let min_len = strings.iter().map(|s| s.len()).min().unwrap_or(0);
// ...
for i in 0..min_len {
    let first_char = strings[0].chars().nth(i);
    ...
}
```

`s.len()` is **bytes**, `chars().nth(i)` is **chars**. For multi-byte
strings the loop ran past the end of each input where `chars().nth(i)`
returned `None`; because `None == None` is true, `prefix_len`
over-counted past the real character LCP. The
`strings[0].chars().take(prefix_len).collect()` masked the symptom by
clamping to whatever characters existed in the first string, so the
returned LCP was *usually* the right substring — but `prefix_len`
itself was unreliable, and the loop did O(n²·k) work via repeated
`chars().nth(i)` calls.

**Fix.** Convert all inputs to `Vec<char>` up front, take the minimum
char count, and index directly. `prefix_len` is now exactly the
character count of the returned LCP, and each loop iteration is
O(1) per string instead of O(n).

### 9.2 Middle slicing relies on `get_lcp`/`get_lcs` char-alignment

In `fold_strings`:

```rust
let start = prefix.len();              // bytes
let end   = text.len().saturating_sub(suffix.len());
let middle = text[start..end].to_string();
```

This only stays safe if `prefix` and `suffix` are exact char-aligned
slices of `text`. After the §9.1 fix that invariant holds by
construction (`get_lcp` builds the prefix from a `Vec<char>` over
the first input string, and `get_lcp`/`get_lcs` always return
substrings whose byte length lands on a UTF-8 char boundary).

Kept on the list because the invariant is *implicit* — there is no
guard in `fold_strings` itself. If anyone ever changes `get_lcp` or
introduces a different prefix/suffix source, this code would panic
on non-ASCII input. Worth a defensive `debug_assert!` that
`text.is_char_boundary(start)` and `text.is_char_boundary(end)`.

### 9.3 Word-boundary snapping is ASCII-space-only

`rfind(' ')` / `find(' ')` only see U+0020. CJK languages have no
inter-word spaces; Thai, Lao, Khmer, Tibetan don't either. Arabic and
Hebrew use spaces but also use narrow no-break spaces (U+202F) and
right-to-left marks adjacent to punctuation. Result: for those
languages, the prefix/suffix often snaps to empty and we end up with
the whole sentence as a single `{{TAG:VAR|full₁|full₂|...}}` — usually
correct but loses structure, and any verbal agreement is hidden inside
the tag instead of factored out.

### 9.4 `777(\d+)` regex eats legitimate trailing digits

`restore_placeholders` is greedy. Any number in the source whose
decimal expansion *starts* with `777` collides with the anchor space:

- `"7770015"` → `"$15"` (parsed as `int("0015") == 15`)
- `"77712345"` → `"$12345"`
- `"the bus 7779 runs daily"` → `"the bus $9 runs daily"`

The Python reference has the same bug verbatim.

### 9.5 Anchor mangling cleanup is incomplete

Only `"777 "` → `"777"` and `" 777"` → `"777"` are repaired. Real MT
behaviour we've already seen but don't fix:

- **Numeral system conversion**: Hindi MT may turn `777002` into
  `७७७००२` (Devanagari digits); Arabic MT may emit `٧٧٧٠٠٢`.
- **Period insertion**: some engines render `"777,002"` or
  `"777.002"` thinking it's a large number.
- **Bidirectional marks**: RTL languages may bracket the digits with
  `U+200E` / `U+200F`.

None of these are detected, so reassembly silently produces wikitext
with the anchors still corrupted, and recovery leaves the corruption
in the output.

### 9.6 Block-translation reordering / merging

The block protocol assumes the MT preserves both line count *and*
order. Counter-examples observed in the wild:

- Some engines strip the numbering for languages whose ordinal
  formatting differs.
- A few engines collapse adjacent lines whose contents are very
  similar (deduplication).
- Output can reorder lines for RTL or topic-flowed languages.

When the split count mismatches, the algorithm errors out with no
fallback to per-line translation.

### 9.7 Greedy MAX_VARIANTS = 64 is target-language dependent

The cap is on the *target* language's plural count, not the source's.
An innocent `{{PLURAL:$1|...}} ... {{PLURAL:$2|...}}` may fit in
English (2×2 = 4) but blow up to 36 in Arabic (6×6) and refuse to
translate at all. The error message says "simplify your message",
which isn't actionable — the author wrote a simple message; it just
expands differently per language.

### 9.8 Source-vs-target option count mismatch produces duplicate variants

When the source provides fewer PLURAL options than the target language
needs, expansion clamps `idx → options.len()-1`, so multiple states
resolve to the same source text (§4.4). The duplicates flow through
MT and reassembly. Two consequences:

1. Wasted MT quota (we pay for translating duplicates).
2. The reassembler can't distinguish "MT collapsed two forms by
   accident" from "the source intentionally collapsed them"; both
   look like identical strings in the same group.

### 9.9 Identical-MT-output erases structure

`fold_strings` returns the bare text when all variants in a group
match. If the MT happens to produce identical output for two
genuinely-different states (e.g. both `un message` for *one* and
*few*), the resulting wikitext loses that magic word entirely. The
output is still semantically correct for that target language, but
the round-trip is not idempotent — re-running the pipeline on the
*output* would produce a structurally smaller message.

### 9.10 GENDER hard-codes 3 forms

`collect_choices` always uses `option_count = 3` for GENDER. Messages
with 2-option GENDER (the common `{{GENDER:$1|He|She}}` pattern) are
expanded to 3 variants where the third clamps to "She". This works,
but it wastes one MT call per such message and forces the
reassembler to merge two identical female variants.

### 9.11 Variable-type fallback is silent

In `fold_strings`:

```rust
let tag_type = self.variable_types.get(var_id).cloned()
    .unwrap_or_else(|| "PLURAL".to_string());
```

If the var-types map is missing an entry (we forgot to populate it,
or the AST has an axis with no matching transclusion), we silently
emit `{{PLURAL:$N|...}}` — even if the original was GENDER. A
debug-only `assert!` or an `MtError::ReassemblyError` would surface
this earlier.

### 9.12 Numbered-prefix collision in source

`translate_as_block` injects `"1. "`, `"2. "` … into the source.
If the message itself begins with a similar pattern (`"1. Click
here"`), the re-split regex `\d+\.\s` may swallow it after MT, leading
to off-by-one truncation. Unlikely in MediaWiki messages but worth a
guard.

### 9.13 Nested magic words aren't supported

The Rust AST defines `Transclusion.options: Vec<String>` — flat
strings, not sub-ASTs. So `{{PLURAL:$1|{{GENDER:$2|he|she}}|they}}`
won't parse as a nested transclusion; it'll be one PLURAL with three
literal-string options. The Python reference contains scaffolding
(`isinstance(opt, list)` recursion in `collect_choices`) hinting that
nesting was once contemplated, but neither implementation actually
exercises it. Some real MediaWiki messages do nest — these will
silently lose structure.

### 9.14 Axis-order independence isn't actually guaranteed

The README claims axis-collapse order doesn't matter. It usually
doesn't, but consider three variants whose LCP/LCS choices depend on
which axis collapses first — e.g. when one axis introduces a word at
the start that the other introduces at the end. Concrete examples are
hard to construct synthetically but show up with inflected languages
that change both verb and adjective endings. The current code uses
non-deterministic `HashMap` iteration to pick the order, so two runs
on the same input can produce different (both valid) wikitext.

### 9.15 70% threshold is empirical

`CONSISTENCY_THRESHOLD = 0.7` was picked by hand. For Russian,
Polish, Arabic and other heavily-inflected languages the same source
sentence can legitimately diverge to ~60% similarity across number
categories ("письмо" vs "писем" vs "письма" share two characters of a
seven-character word). The threshold will reject valid translations
for those languages while still accepting subtler hallucinations in
analytic languages like Chinese.

### 9.16 No per-variant fallback when consistency fails

When `fold_strings` raises `ConsistencyError`, the whole reassembly
aborts. No partial result is returned, no diagnostic is attached to a
specific axis, no retry is attempted. A natural retry would be to
translate the offending pair individually and re-attempt the fold —
not implemented.

### 9.17 No source-language-side anchoring of PLURAL counts

If the message has `{{PLURAS:$1|one|other}}` where `$1` is actually a
count (e.g. `"5"`), the resolver substitutes `777001` as the count
*and* expands all plural forms. The MT engine then sees a sentence
with `777001` as a noun-like token but no agreement signal — so for
languages where the verb agrees with grammatical number (German,
French in some contexts) the wrong form may come back. Better
behaviour: substitute representative counts (1, 2, 5) per variant.

---

## 10. References

- `mediawiki_mt_assisted_localization.py` — single-file Python reference
  this implementation was ported from.
- `banana-i18n-mt/src/expansion.rs` — Phase 1.
- `banana-i18n-mt/src/google_translate.rs` — Phase 2 (`translate_as_block`).
- `banana-i18n-mt/src/reassembly.rs` — Phases 3 and 4.
- `banana-i18n-mt/README.md` — high-level overview with the
  English → French walkthrough.
- Unicode CLDR plural rules — drive the per-locale PLURAL form counts
  via ICU (`icu_plurals`).
