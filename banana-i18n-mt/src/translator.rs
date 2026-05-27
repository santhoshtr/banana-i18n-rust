//! Machine Translation trait and utilities
//!
//! This module defines the `MachineTranslator` trait for provider abstraction,
//! enabling support for different MT backends (Google Translate, mock, etc.)
//! without coupling the library to any specific implementation.
//!
//! # Example
//!
//! ```ignore
//! use banana_i18n::mt::{MachineTranslator, GoogleTranslateProvider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a provider
//!     let provider = GoogleTranslateProvider::from_env()?;
//!
//!     // Translate a single string
//!     let result = provider.translate("Hello, world!", "en", "fr").await?;
//!     println!("{}", result); // "Bonjour, le monde!"
//!
//!     // Translate multiple strings in a batch
//!     let texts = vec!["Hello".to_string(), "Goodbye".to_string()];
//!     let results = provider.translate_batch(&texts, "en", "fr").await?;
//!     println!("{:?}", results);
//!
//!     Ok(())
//! }
//! ```

use crate::error::MtResult;
use async_trait::async_trait;

/// Generic trait for machine translation providers
///
/// Implementations of this trait handle the actual translation work,
/// whether through an API (Google Translate) or deterministic logic (Mock).
///
/// All methods are async to support I/O-bound operations like network requests.
#[async_trait]
pub trait MachineTranslator: Send + Sync {
    /// Translate a single text string from source to target locale
    ///
    /// # Arguments
    ///
    /// * `text` - The text to translate
    /// * `source_locale` - Source language code (e.g., "en", "en-US")
    /// * `target_locale` - Target language code (e.g., "fr", "fr-FR")
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The translated text
    /// * `Err(MtError)` - If translation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = provider.translate("Hello", "en", "fr").await?;
    /// assert_eq!(result, "Bonjour");
    /// ```
    async fn translate(
        &self,
        text: &str,
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<String>;

    /// Translate multiple strings in a single batch operation
    ///
    /// Batch translation is more efficient than individual translations,
    /// especially for providers with per-request overhead (like API calls).
    /// Implementations may chunk large batches internally.
    ///
    /// # Arguments
    ///
    /// * `texts` - Vector of strings to translate
    /// * `source_locale` - Source language code
    /// * `target_locale` - Target language code
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - Translated strings in the same order as input
    /// * `Err(MtError)` - If translation fails
    ///
    /// # Guarantees
    ///
    /// - Output order matches input order
    /// - Output length equals input length
    /// - Each translation is independent
    ///
    /// # Example
    ///
    /// ```ignore
    /// let texts = vec!["Hello".to_string(), "Goodbye".to_string()];
    /// let results = provider.translate_batch(&texts, "en", "fr").await?;
    /// assert_eq!(results.len(), 2);
    /// ```
    async fn translate_batch(
        &self,
        texts: &[String],
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<Vec<String>>;

    /// Get the name of this translation provider
    ///
    /// Used for logging and debugging to identify which provider handled a translation.
    ///
    /// # Returns
    ///
    /// A string identifying the provider (e.g., "Google Translate", "Mock Translator")
    fn provider_name(&self) -> &str;

    /// Translate multiple variants as a single numbered block.
    ///
    /// Joins all variants with numbered prefixes (`1. ...`, `2. ...`) and
    /// translates them in **one** `translate` call, so the MT engine sees the
    /// related variants together and keeps wording consistent across them
    /// (important for PLURAL/GENDER forms). The block is then split back into
    /// individual translations.
    ///
    /// This default implementation is provider-agnostic — it relies only on
    /// [`translate`](Self::translate). Providers may override it if they need
    /// different behaviour.
    ///
    /// # Guarantees
    ///
    /// - Output order matches input order
    /// - Output length equals input length
    async fn translate_as_block(
        &self,
        variants: &[String],
        source_locale: &str,
        target_locale: &str,
    ) -> MtResult<Vec<String>> {
        // Handle empty case
        if variants.is_empty() {
            return Ok(Vec::new());
        }

        // Handle single variant case — no numbering needed
        if variants.len() == 1 {
            let result = self
                .translate(&variants[0], source_locale, target_locale)
                .await?;
            return Ok(vec![normalize_anchors(&result)]);
        }

        // 1. Join with numbered prefixes
        let input_block: String = variants
            .iter()
            .enumerate()
            .map(|(i, variant)| format!("{}. {}", i + 1, variant))
            .collect::<Vec<_>>()
            .join("\n");

        // 2. Translate the entire block in a single call
        let translated_block = self
            .translate(&input_block, source_locale, target_locale)
            .await?;

        // 3. Split back using the numbered prefixes
        use regex::Regex;
        let re = Regex::new(r"\n?\d+\.\s").unwrap();
        let lines: Vec<String> = re
            .split(translated_block.trim())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // 4. Safety check: same count
        if lines.len() != variants.len() {
            return Err(crate::error::MtError::TranslationError(format!(
                "Block translation count mismatch: expected {}, got {}. Block: '{}'",
                variants.len(),
                lines.len(),
                translated_block
            )));
        }

        // 5. De-mangle anchor tokens (foreign numerals, inserted separators, …)
        let cleaned: Vec<String> = lines.iter().map(|line| normalize_anchors(line)).collect();

        Ok(cleaned)
    }
}

/// Decimal-digit block-start codepoints for the scripts MT engines realistically
/// emit. Within each block the ten digits are contiguous (`0..=9`), so the value
/// of a digit char is `codepoint - block_start`.
const DIGIT_BLOCK_STARTS: &[u32] = &[
    0x0030, // ASCII
    0x0660, // Arabic-Indic
    0x06F0, // Extended Arabic-Indic (Persian / Urdu)
    0x0966, // Devanagari
    0x09E6, // Bengali
    0x0A66, // Gurmukhi
    0x0AE6, // Gujarati
    0x0B66, // Oriya
    0x0BE6, // Tamil
    0x0C66, // Telugu
    0x0CE6, // Kannada
    0x0D66, // Malayalam
    0x0E50, // Thai
    0x0ED0, // Lao
    0x0F20, // Tibetan
    0x1040, // Myanmar
    0x17E0, // Khmer
    0xFF10, // Fullwidth
];

/// Return the `0..=9` value of any Unicode decimal digit we recognise, folding
/// foreign numeral systems (Devanagari, Arabic-Indic, …) back to a value.
pub(crate) fn fold_digit(c: char) -> Option<u8> {
    let cp = c as u32;
    for &start in DIGIT_BLOCK_STARTS {
        if cp >= start && cp <= start + 9 {
            return Some((cp - start) as u8);
        }
    }
    None
}

/// Characters MT engines may insert *between* the digits of an anchor token:
/// spaces of various widths, digit-group separators, and bidi controls.
fn is_anchor_separator(c: char) -> bool {
    matches!(c,
        '\u{0020}' | '\u{00A0}' | '\u{202F}' | '\u{2009}' | '\u{2007}' | '\u{2008}'
        | ',' | '.' | '\u{066B}' | '\u{066C}' | '\u{2024}'
        | '\u{200E}' | '\u{200F}' | '\u{061C}'
        | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// If `chars[start..]` begins with a `777`-led anchor — six digits (in any
/// numeral system) where the first three fold to `7`, with optional
/// [`is_anchor_separator`] chars allowed *between* consecutive digits — return
/// the exclusive end index. Returns `None` otherwise.
///
/// Digit boundaries are enforced on both sides so a real number is never
/// mistaken for an anchor: the run must not be immediately preceded or followed
/// by another digit.
fn match_anchor(chars: &[char], start: usize) -> Option<usize> {
    // Left boundary: the char before the run must not be a digit.
    if start > 0 && fold_digit(chars[start - 1]).is_some() {
        return None;
    }

    let mut idx = start;
    let mut digits = [0u8; 6];
    for (slot, d) in digits.iter_mut().enumerate() {
        // Allow separators between digits (but not before the first one).
        if slot > 0 {
            while idx < chars.len() && is_anchor_separator(chars[idx]) {
                idx += 1;
            }
        }
        *d = chars.get(idx).copied().and_then(fold_digit)?;
        idx += 1;
    }

    // Must be the 777-prefixed anchor shape.
    if digits[0] != 7 || digits[1] != 7 || digits[2] != 7 {
        return None;
    }

    // Right boundary: not immediately followed by another digit.
    if idx < chars.len() && fold_digit(chars[idx]).is_some() {
        return None;
    }

    Some(idx)
}

/// De-mangle anchor tokens in a line of MT output.
///
/// MT engines reformat the numeric anchors (`777NNN`) in three ways: converting
/// the digits to another numeral system (Devanagari, Arabic-Indic, …), inserting
/// separators inside the number (ASCII/no-break spaces, grouping commas, the odd
/// `"77 7002"` split), and bracketing them with bidi marks. This scans for
/// `777`-led anchor runs (see [`match_anchor`]) and rewrites each to its
/// canonical ASCII form `777NNN`, leaving everything else — including legitimate
/// localized numerals and the word-separating space *before* an anchor —
/// untouched. See docs/mt_assisted_localization.md §9.5.
pub(crate) fn normalize_anchors(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some(end) = match_anchor(&chars, i) {
            // Emit the folded digits as canonical ASCII (always `777NNN`).
            for c in &chars[i..end] {
                if let Some(d) = fold_digit(*c) {
                    out.push((b'0' + d) as char);
                }
            }
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Normalize a locale code by stripping region information
///
/// Converts locale codes from BCP 47 format to ISO 639-1 format:
/// - `en-US` → `en`
/// - `zh-Hans` → `zh`
/// - `fr-FR` → `fr`
/// - `en` → `en` (unchanged)
///
/// # Arguments
///
/// * `locale` - The locale code to normalize
///
/// # Returns
///
/// The normalized locale code (base language only)
///
/// # Example
///
/// ```ignore
/// assert_eq!(normalize_locale("en-US"), "en");
/// assert_eq!(normalize_locale("zh-Hans"), "zh");
/// ```
pub fn normalize_locale(locale: &str) -> String {
    // Split on hyphen and take the first part (language code)
    locale.split('-').next().unwrap_or(locale).to_lowercase()
}

/// Validate that a locale code is in acceptable format
///
/// Checks that the locale code contains only alphanumeric characters,
/// hyphens, and underscores (following ISO 639 conventions).
///
/// # Arguments
///
/// * `locale` - The locale code to validate
///
/// # Returns
///
/// * `Ok(())` - If the locale is valid
/// * `Err(MtError)` - If the locale is invalid
///
/// # Example
///
/// ```ignore
/// validate_locale("en")?; // OK
/// validate_locale("en-US")?; // OK
/// validate_locale("invalid@code").unwrap_err(); // Error
/// ```
pub fn validate_locale(locale: &str) -> MtResult<()> {
    if locale.is_empty() {
        return Err(crate::error::MtError::InvalidLocale(
            "Locale code is empty".to_string(),
        ));
    }

    // Check that locale contains only valid characters
    if !locale
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(crate::error::MtError::InvalidLocale(format!(
            "Invalid characters in locale code: {}",
            locale
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_locale_with_region() {
        assert_eq!(normalize_locale("en-US"), "en");
        assert_eq!(normalize_locale("en-GB"), "en");
        assert_eq!(normalize_locale("fr-FR"), "fr");
    }

    #[test]
    fn test_normalize_locale_with_script() {
        assert_eq!(normalize_locale("zh-Hans"), "zh");
        assert_eq!(normalize_locale("zh-Hant"), "zh");
        assert_eq!(normalize_locale("sr-Latn"), "sr");
    }

    #[test]
    fn test_normalize_locale_complex() {
        // Even complex codes get normalized to language only
        assert_eq!(normalize_locale("de-AT-1996"), "de");
    }

    #[test]
    fn test_normalize_locale_already_simple() {
        assert_eq!(normalize_locale("en"), "en");
        assert_eq!(normalize_locale("fr"), "fr");
        assert_eq!(normalize_locale("ru"), "ru");
    }

    #[test]
    fn test_normalize_locale_case_insensitive() {
        // Ensures lowercase output
        assert_eq!(normalize_locale("EN"), "en");
        assert_eq!(normalize_locale("EN-US"), "en");
    }

    #[test]
    fn test_validate_locale_valid_codes() {
        assert!(validate_locale("en").is_ok());
        assert!(validate_locale("en-US").is_ok());
        assert!(validate_locale("zh-Hans").is_ok());
        assert!(validate_locale("de_DE").is_ok());
    }

    #[test]
    fn test_validate_locale_invalid_codes() {
        assert!(validate_locale("").is_err());
        assert!(validate_locale("en@invalid").is_err());
        assert!(validate_locale("fr#bad").is_err());
        assert!(validate_locale("es!error").is_err());
    }

    #[test]
    fn test_validate_locale_error_messages() {
        use crate::error::MtError;
        match validate_locale("en@US") {
            Err(MtError::InvalidLocale(msg)) => {
                assert!(msg.contains("Invalid characters"));
            }
            _ => panic!("Expected InvalidLocale error"),
        }
    }

    // ========== fold_digit Tests ==========

    #[test]
    fn test_fold_digit_ascii() {
        assert_eq!(fold_digit('0'), Some(0));
        assert_eq!(fold_digit('7'), Some(7));
        assert_eq!(fold_digit('9'), Some(9));
    }

    #[test]
    fn test_fold_digit_foreign_scripts() {
        assert_eq!(fold_digit('७'), Some(7)); // Devanagari 7 (U+096D)
        assert_eq!(fold_digit('٧'), Some(7)); // Arabic-Indic 7 (U+0667)
        assert_eq!(fold_digit('۷'), Some(7)); // Extended Arabic-Indic 7 (U+06F7)
        assert_eq!(fold_digit('৭'), Some(7)); // Bengali 7 (U+09ED)
        assert_eq!(fold_digit('๗'), Some(7)); // Thai 7 (U+0E57)
        assert_eq!(fold_digit('７'), Some(7)); // Fullwidth 7 (U+FF17)
        assert_eq!(fold_digit('০'), Some(0)); // Bengali 0
    }

    #[test]
    fn test_fold_digit_non_digits() {
        assert_eq!(fold_digit('a'), None);
        assert_eq!(fold_digit(' '), None);
        assert_eq!(fold_digit('।'), None); // Devanagari danda, not a digit
    }

    // ========== normalize_anchors Tests ==========

    #[test]
    fn test_normalize_preserves_space_before_anchor() {
        // Regression (§9.5): the space between निम्नलिखित ("following") and the
        // anchor 777001 ($1) must be preserved — it is a word separator, not
        // intra-anchor mangling.
        let input = "निम्नलिखित 777001 फ़ाइलें वर्तमान श्रेणी में हैं।";
        assert_eq!(normalize_anchors(input), input);
    }

    #[test]
    fn test_normalize_rejoins_ascii_space() {
        assert_eq!(
            normalize_anchors("The following 777 001 files"),
            "The following 777001 files"
        );
    }

    #[test]
    fn test_normalize_rejoins_nbsp() {
        // Google French groups digits with a NO-BREAK SPACE (U+00A0).
        assert_eq!(normalize_anchors("Vous avez 777\u{00A0}002 messages"), "Vous avez 777002 messages");
    }

    #[test]
    fn test_normalize_rejoins_odd_split() {
        // MinT French splits as "77 7002" (space after two digits).
        assert_eq!(normalize_anchors("Vous avez 77 7002 messages"), "Vous avez 777002 messages");
    }

    #[test]
    fn test_normalize_grouping_comma_and_indian() {
        assert_eq!(normalize_anchors("777,002 items"), "777002 items");
        assert_eq!(normalize_anchors("7,77,002 items"), "777002 items"); // Indian grouping
    }

    #[test]
    fn test_normalize_folds_foreign_numerals() {
        assert_eq!(normalize_anchors("आपके पास ७७७००२ संदेश"), "आपके पास 777002 संदेश"); // Devanagari
        assert_eq!(normalize_anchors("لديك ٧٧٧٠٠٢ رسالة"), "لديك 777002 رسالة"); // Arabic-Indic
        assert_eq!(normalize_anchors("৭৭৭০০২টি বার্তা"), "777002টি বার্তা"); // Bengali
    }

    #[test]
    fn test_normalize_strips_bidi_marks() {
        // Bidi marks bracketing/within the digits are stripped.
        assert_eq!(normalize_anchors("x \u{200F}777002\u{200E} y"), "x \u{200F}777002\u{200E} y");
        assert_eq!(normalize_anchors("x 777\u{200E}002 y"), "x 777002 y");
    }

    #[test]
    fn test_normalize_intact_anchor_unchanged() {
        assert_eq!(normalize_anchors("777001 फ़ाइलें"), "777001 फ़ाइलें");
        assert_eq!(
            normalize_anchors("He sent 777001 messages"),
            "He sent 777001 messages"
        );
    }

    #[test]
    fn test_normalize_leaves_real_numbers_alone() {
        // Anchor-scoped: numbers that are not 777-led anchors are untouched.
        assert_eq!(normalize_anchors("Total: 12,345 edits"), "Total: 12,345 edits");
        assert_eq!(normalize_anchors("शीर्ष ५ लेख"), "शीर्ष ५ लेख"); // lone Devanagari 5
    }

    #[test]
    fn test_normalize_real_number_adjacent_to_anchor() {
        // A genuine number next to an anchor must survive (boundary checks).
        assert_eq!(normalize_anchors("5 777001 items"), "5 777001 items");
        assert_eq!(normalize_anchors("777001 then 5"), "777001 then 5");
    }

    #[test]
    fn test_normalize_does_not_eat_longer_number() {
        // 7-digit real number starting 777 is not an anchor (right boundary).
        assert_eq!(normalize_anchors("id 7770015 here"), "id 7770015 here");
    }

    #[test]
    fn test_normalize_multiple_anchors_one_line() {
        assert_eq!(
            normalize_anchors("777001 sent 777 002 to 777003"),
            "777001 sent 777002 to 777003"
        );
    }
}
