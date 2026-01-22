# Problem

I am working on a translation suggestion for localizers. Using an Machine translation system, that is capable of translating only plain text we need to prepare accurate translation suggestion to other languages like French or hindi preserving the syntax.

# Proposed algorithm

For each message, generate all possible plain text expansion by passing placeholder values -dummy values.now the i8n system can give the userfacing string by resolving placeholders and  plural. Translate all these source variants to targets variants. Then assemble back to wikitext

This approach is often called **"Cartesian Expansion"** or **"Template Synthesis via Realization."** It is linguistically the most robust way to handle "agreement" (where a gender or plural choice at the start of a sentence changes the form of a word at the end of the sentence).

However, it is technically challenging to "assemble back." Let’s break down the pros, the pitfalls, and a proposed pipeline for your AST-based implementation.

---

### 1. The Core Challenge: Combinatorial Explosion
In MediaWiki, a single message can have multiple variables.
*   **Example:** `{{GENDER:$1|$1}} sent {{PLURAL:$2|a message| $2 messages}} to {{GENDER:$3|him|her}}.`
*   **Expansion:** (2 Genders for $1) × (2 Plural forms for $2) × (2 Genders for $3) = **8 unique plain-text strings.**
*   **Target Language:** If the target language is Russian or Arabic, the plural categories might increase, leading to even more variants.

### 2. The Implementation Pipeline

#### Step A: Expansion (The "Matrix" Generation)
Using your Tree-sitter AST, you identify every "Choice Node" (`PLURAL`, `GENDER`, `GRAMMAR`).
1.  **Determine Target Categories:** Look up the target language's CLDR plural rules (e.g., Hindi has 2, Arabic has 6).
2.  **Generate Test Values:**
    *   For `PLURAL`: Choose a representative number for each category (e.g., 1 for "one", 2 for "two", 5 for "other").
    *   For `GENDER`: Map to "male", "female", and "unknown".
3.  **Produce Variants:** Generate the full set of plain-text strings.
    *   *Crucial Tip:* Use "Anchor Tokens" for placeholders. Instead of `$1`, use a unique, non-translatable string like `_ID1_` or `777001`. This prevents the MT from translating "1" into "un" or "ek".

#### Step B: The Translation Phase
Send the batch of expanded strings to the MT.
*   **Example (Source):** `777001 sent 1 message.` / `777001 sent 5 messages.`
*   **Example (Target - French):** `777001 a envoyé 1 message.` / `777001 a envoyé 5 messages.`

#### Step C: The Reassembly Logic (The Hard Part)
You now have $N$ translated strings and $N$ source strings. You need to "fold" them back into a single MediaWiki wikitext string.

**The "Diff-and-Capture" Algorithm:**
1.  **Structural Alignment:** Compare the translated variants. Identify the "stable" parts (text that never changes) and the "variable" parts.
2.  **AST Mapping:**
    *   If Variant A and Variant B only differ at the spot where the `{{PLURAL}}` node existed in the original AST, you extract those differing strings.
    *   **Linguistic Check:** In languages like French, the MT might change a verb or adjective *outside* the original plural block to agree with the number.
        *   *English:* `The $1 apple is red` / `The $1 apples are red`.
        *   *French:* `La $1 pomme est rouge` / `Les $1 pommes sont rouges`.
    *   **The "Expansion" Rule:** If the MT changed words *outside* the wikitext bracket, you must **widen the scope** of the wikitext tag in the suggestion so it remains grammatically correct.

---

### 3. Critical Considerations

#### A. Target Plural Mismatch
If translating from English (2 forms) to Arabic (6 forms), your MT won't know how to generate the 4 missing forms unless you explicitly provide 6 source variants.
*   **Solution:** Even if English only has "One" and "Other", generate 6 English variants where the "Other" text is repeated for the Arabic "Dual", "Paucal", etc., categories. The MT will then naturally decline them into the correct Arabic forms.

#### B. Placeholder Recovery
MT often moves placeholders around (Word order changes).
*   **Source:** `777001 sent 777002.`
*   **Japanese MT:** `777002 は 777001 によって送信されました。` (Order swapped).
*   Your AST reassembler must use the "Anchor Tokens" (`777001`) to find where the `$1` should be placed in the target wikitext.

#### C. Hallucination and Consistency
MT can be inconsistent. It might translate "message" as "message" in Variant 1 and "pli" in Variant 2.
*   **Mitigation:** Use the AST to perform a "Post-Translation Consistency Check." If the stable parts of the sentences vary significantly between variants, the suggestion might be corrupt.

### 4. Why this approach is superior
Most i18n tools try to translate "The", then "apple", then "apples". This results in "broken" translations in highly inflected languages.
By translating **full sentence variants**, you allow the MT to:
1.  Handle **Gender Agreement** (Adjectives changing based on user gender).
2.  Handle **Vowel Elision** (e.g., in French, `le` becomes `l'` before a vowel).
3.  Handle **Case Marking** (In Hindi or German, the word for "File" changes depending on its role in the sentence).

### Summary of Recommendation
1.  **Parse** original wikitext to AST.
2.  **Identify** all permutation axes (GENDER, PLURAL).
3.  **Expand** into $N$ plain-text strings using unique IDs for placeholders.
4.  **Translate** the full batch.
5.  **Reassemble** by comparing the translated strings and mapping the differences back into the `{{PLURAL|...}}` and `{{GENDER|...}}` slots in the AST.
6.  **Highlight** to the localizer where the MT changed words *outside* the brackets.
