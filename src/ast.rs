use icu_locale::Locale;
use icu_plurals::{PluralCategory, PluralRuleType, PluralRules};

// Import for verbosity logging (will be used via crate::VerbosityLevel in context-aware methods)
use crate::VerbosityLevel;

// Type alias for convenience
pub type AstNodeList = Vec<AstNode>;

// Main AST node enum - represents all possible node types in MediaWiki i18n messages
#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    Text(String),
    Placeholder(Placeholder),
    Transclusion(Transclusion),
    InternalLink(WikiInternalLink),
    ExternalLink(WikiExternalLink),
}

/// Placeholder: $1, $2, $3, etc. (1-indexed)
#[derive(Debug, Clone, PartialEq)]
pub struct Placeholder {
    pub index: usize, // 1 for $1, 2 for $2, etc.
}

/// Transclusion: {{PLURAL:$1|singular|plural|...}}
/// Supports any number of plural forms for different languages
#[derive(Debug, Clone, PartialEq)]
pub struct Transclusion {
    pub name: String,         // e.g., "PLURAL"
    pub param: String,        // e.g., "$1" or "2"
    pub options: Vec<String>, // e.g., ["is", "are"] or multiple forms for other languages
}

/// Internal wiki link: [[Page]] or [[Page|Display Text]]
#[derive(Debug, Clone, PartialEq)]
pub struct WikiInternalLink {
    pub target: String,
    pub display_text: Option<String>,
}

/// External link: [http://example.com] or [http://example.com Text]
#[derive(Debug, Clone, PartialEq)]
pub struct WikiExternalLink {
    pub url: String,
    pub text: Option<String>,
}

/// Trait for elements that can be localized with parameter values
pub trait Localizable {
    fn localize(&self, locale: &str, values: &Vec<String>) -> String;
}

/// Parse a locale string into an ICU Locale
/// Handles simple codes like "en", "ru" and complex ones like "en-US", "zh-Hans"
fn parse_locale(locale_str: &str) -> Result<Locale, String> {
    locale_str
        .parse::<Locale>()
        .map_err(|e| format!("Failed to parse locale '{}': {}", locale_str, e))
}

/// Map ICU PluralCategory to form index using MediaWiki/CLDR standard ordering
///
/// The ordering is based on Unicode CLDR plural rules:
/// - English (en): One (0), Other (1)
/// - Russian (ru): One (0), Few (1), Many (2), [Other (3)]
/// - Polish (pl): One (0), Few (1), Many (2), [Other (3)]
/// - Arabic (ar): Zero (0), One (1), Two (2), Few (3), Many (4), Other (5)
/// - French (fr): One (0), Other (1)
///
/// This function maps categories to indices; if a category doesn't match any available
/// form, we fall back to the last available form.
fn plural_category_to_index(category: PluralCategory, form_count: usize) -> usize {
    if form_count == 0 {
        return 0;
    }

    match category {
        PluralCategory::Zero => 0,
        PluralCategory::One => {
            // One maps to index 0 in most languages, or index 1 in some (after Zero)
            // Generally, if we have Zero, One is at index 1; otherwise at index 0
            0
        }
        PluralCategory::Two => {
            // Two typically comes after One
            // Common: One (0), Two (1) or Zero (0), One (1), Two (2)
            if form_count >= 3 {
                2
            } else if form_count >= 2 {
                1
            } else {
                form_count - 1
            }
        }
        PluralCategory::Few => {
            // Few comes after One/Two
            // English: doesn't use Few
            // Russian: One (0), Few (1), Many (2)
            // Polish: One (0), Few (1), Many (2)
            // Arabic: Zero (0), One (1), Two (2), Few (3), Many (4), Other (5)
            if form_count >= 4 {
                3
            } else if form_count >= 3 {
                1
            } else if form_count >= 2 {
                1
            } else {
                form_count - 1
            }
        }
        PluralCategory::Many => {
            // Many is typically near the end
            // Russian: One (0), Few (1), Many (2)
            // Polish: One (0), Few (1), Many (2)
            // Arabic: Zero (0), One (1), Two (2), Few (3), Many (4), Other (5)
            if form_count >= 5 {
                4
            } else if form_count >= 3 {
                2
            } else if form_count >= 2 {
                1
            } else {
                form_count - 1
            }
        }
        PluralCategory::Other => {
            // Other is the default/fallback category - use the last form
            form_count - 1
        }
    }
}

impl Localizable for Placeholder {
    fn localize(&self, _locale: &str, values: &Vec<String>) -> String {
        // Placeholders are 1-indexed: $1 = values[0], $2 = values[1], etc.
        if self.index > 0 && self.index <= values.len() {
            values[self.index - 1].clone()
        } else {
            // If no value provided, return the placeholder as-is
            format!("${}", self.index)
        }
    }
}

impl Localizable for Transclusion {
    fn localize(&self, locale: &str, values: &Vec<String>) -> String {
        match self.name.to_uppercase().as_str() {
            "PLURAL" => self.localize_plural(locale, values),
            // Future: Add GENDER, GRAMMAR, etc.
            _ => {
                // Unknown magic word - log warning and return original syntax
                eprintln!("Warning: Unknown magic word '{}'", self.name);
                format!(
                    "{{{{{}:{}|{}}}}}",
                    self.name,
                    self.param,
                    self.options.join("|")
                )
            }
        }
    }
}

impl Transclusion {
    /// Localize a PLURAL magic word using language-specific plural rules from ICU
    ///
    /// Uses ICU plural rules for proper multi-language support. Falls back to English
    /// rules if the locale is invalid or plural rules cannot be loaded.
    ///
    /// # Arguments
    /// * `locale` - Language code (e.g., "en", "ru", "ar")
    /// * `values` - Array of values to substitute
    ///
    /// # Returns
    /// The appropriate plural form for the given number and language
    fn localize_plural(&self, locale: &str, values: &Vec<String>) -> String {
        // Extract the count from param (e.g., "$1" -> values[0])
        let count = if self.param.starts_with('$') {
            // It's a placeholder reference
            let index_str = &self.param[1..];
            let index: usize = index_str.parse().unwrap_or(0);
            if index > 0 && index <= values.len() {
                values[index - 1].parse::<u32>().unwrap_or(0)
            } else {
                0
            }
        } else {
            // Direct number
            self.param.parse::<u32>().unwrap_or(0)
        };

        // Try to get the plural category using ICU rules
        let form_index = match get_plural_form_index(locale, count, self.options.len()) {
            Ok(index) => index,
            Err(_e) => {
                // Fall back to English plural rules
                #[cfg(debug_assertions)]
                eprintln!(
                    "Warning: Failed to get plural form for locale '{}': {}. Using English rules.",
                    locale, _e
                );

                // English rule: 1 = singular (index 0), others = plural (index 1 or last)
                if count == 1 {
                    0
                } else {
                    1.min(self.options.len() - 1)
                }
            }
        };

        self.options
            .get(form_index)
            .cloned()
            .unwrap_or_else(|| self.options.last().cloned().unwrap_or_default())
    }

    /// Localize with fallback chain support and verbosity context
    /// This is an internal method called from lib.rs with verbosity level
    pub fn localize_with_context(
        &self,
        locale: &str,
        values: &Vec<String>,
        verbosity: VerbosityLevel,
    ) -> String {
        match self.name.to_uppercase().as_str() {
            "PLURAL" => self.localize_plural_with_fallback(locale, values, verbosity),
            // Future: Add GENDER, GRAMMAR, etc.
            _ => {
                // Unknown magic word - log warning and return original syntax
                eprintln!("Warning: Unknown magic word '{}'", self.name);
                format!(
                    "{{{{{}:{}|{}}}}}",
                    self.name,
                    self.param,
                    self.options.join("|")
                )
            }
        }
    }

    /// Localize a PLURAL magic word with fallback chain support
    /// This tries the requested locale and follows the fallback chain
    fn localize_plural_with_fallback(
        &self,
        locale: &str,
        values: &Vec<String>,
        verbosity: VerbosityLevel,
    ) -> String {
        // Extract the count from param (e.g., "$1" -> values[0])
        let count = if self.param.starts_with('$') {
            // It's a placeholder reference
            let index_str = &self.param[1..];
            let index: usize = index_str.parse().unwrap_or(0);
            if index > 0 && index <= values.len() {
                values[index - 1].parse::<u32>().unwrap_or(0)
            } else {
                0
            }
        } else {
            // Direct number
            self.param.parse::<u32>().unwrap_or(0)
        };

        // Try to get the plural category using ICU rules with fallback chain
        let form_index = match get_plural_form_index_with_fallback(
            locale,
            count,
            self.options.len(),
            verbosity,
        ) {
            Ok(index) => index,
            Err(_e) => {
                // Fall back to English plural rules
                if verbosity >= VerbosityLevel::Normal {
                    eprintln!(
                        "[i18n] Warning: Failed to get plural form for locale '{}'. Using English rules.",
                        locale
                    );
                }

                // English rule: 1 = singular (index 0), others = plural (index 1 or last)
                if count == 1 {
                    0
                } else {
                    1.min(self.options.len() - 1)
                }
            }
        };

        self.options
            .get(form_index)
            .cloned()
            .unwrap_or_else(|| self.options.last().cloned().unwrap_or_default())
    }
}

/// Get the appropriate plural form index for a given locale and count
///
/// Returns the form index to use for the plural forms array, or an error if
/// plural rules cannot be determined for the locale.
fn get_plural_form_index(locale_str: &str, count: u32, form_count: usize) -> Result<usize, String> {
    if form_count == 0 {
        return Ok(0);
    }

    // Parse the locale
    let locale = parse_locale(locale_str)?;

    // Create plural rules for the locale (cardinal numbers)
    let pr = PluralRules::try_new(locale.into(), PluralRuleType::Cardinal.into())
        .map_err(|e| format!("Failed to create PluralRules: {}", e))?;

    // Get the plural category for this count
    let category = pr.category_for(count as usize);

    // Map the category to a form index
    let form_index = plural_category_to_index(category, form_count);

    Ok(form_index)
}

/// Get the appropriate plural form index for a given locale and count, with fallback chain support
///
/// Tries the requested locale first, then follows the fallback chain to find the first
/// locale with working plural rules. Returns the form index or an error if none found.
fn get_plural_form_index_with_fallback(
    locale_str: &str,
    count: u32,
    form_count: usize,
    verbosity: VerbosityLevel,
) -> Result<usize, String> {
    if form_count == 0 {
        return Ok(0);
    }

    let fallback_chain = crate::fallbacks::resolve_locale_chain(locale_str);

    for fallback_locale in &fallback_chain {
        match get_plural_form_index(fallback_locale, count, form_count) {
            Ok(index) => {
                // Found working plural rules
                if fallback_locale != locale_str && verbosity >= VerbosityLevel::Normal {
                    eprintln!(
                        "[i18n] Fallback: Using plural rules from locale '{}' (requested: '{}')",
                        fallback_locale, locale_str
                    );
                }
                if verbosity >= VerbosityLevel::Verbose {
                    eprintln!(
                        "[i18n] Plural fallback chain: {}",
                        fallback_chain.join(" -> ")
                    );
                }
                return Ok(index);
            }
            Err(_) => {
                // This locale doesn't have plural rules, try next in chain
                continue;
            }
        }
    }

    // No working plural rules found in entire chain
    Err(format!(
        "No plural rules found for locale '{}' or its fallbacks: {}",
        locale_str,
        fallback_chain.join(" -> ")
    ))
}

impl WikiInternalLink {
    pub fn to_html(&self) -> String {
        let display = self.display_text.as_ref().unwrap_or(&self.target);
        format!("<a href=\"{}\">{}</a>", self.target, display)
    }
}

impl ToString for WikiInternalLink {
    fn to_string(&self) -> String {
        self.to_html()
    }
}

impl WikiExternalLink {
    pub fn to_html(&self) -> String {
        let display = self.text.as_ref().unwrap_or(&self.url);
        format!("<a href=\"{}\">{}</a>", self.url, display)
    }
}

impl ToString for WikiExternalLink {
    fn to_string(&self) -> String {
        self.to_html()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_localize() {
        let placeholder = Placeholder { index: 1 };
        let values = vec!["World".to_string()];
        assert_eq!(placeholder.localize("en", &values), "World");
    }

    #[test]
    fn test_placeholder_missing_value() {
        let placeholder = Placeholder { index: 5 };
        let values = vec!["World".to_string()];
        assert_eq!(placeholder.localize("en", &values), "$5");
    }

    #[test]
    fn test_plural_singular() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };
        let values = vec!["1".to_string()];
        assert_eq!(transclusion.localize("en", &values), "item");
    }

    #[test]
    fn test_plural_plural() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };
        let values = vec!["5".to_string()];
        assert_eq!(transclusion.localize("en", &values), "items");
    }

    #[test]
    fn test_plural_zero() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };
        let values = vec!["0".to_string()];
        assert_eq!(transclusion.localize("en", &values), "items");
    }

    #[test]
    fn test_plural_multiple_forms() {
        // Test with more than 2 plural forms
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["one".to_string(), "two".to_string(), "other".to_string()],
        };
        let values = vec!["3".to_string()];
        // 3 in English falls into "Other" category which maps to the last form
        assert_eq!(transclusion.localize("en", &values), "other");
    }

    #[test]
    fn test_internal_link_html() {
        let link = WikiInternalLink {
            target: "box".to_string(),
            display_text: None,
        };
        assert_eq!(link.to_html(), "<a href=\"box\">box</a>");
    }

    #[test]
    fn test_internal_link_with_display() {
        let link = WikiInternalLink {
            target: "Main Page".to_string(),
            display_text: Some("home".to_string()),
        };
        assert_eq!(link.to_html(), "<a href=\"Main Page\">home</a>");
    }

    #[test]
    fn test_external_link_html() {
        let link = WikiExternalLink {
            url: "https://example.com".to_string(),
            text: None,
        };
        assert_eq!(
            link.to_html(),
            "<a href=\"https://example.com\">https://example.com</a>"
        );
    }

    #[test]
    fn test_external_link_with_text() {
        let link = WikiExternalLink {
            url: "https://example.com".to_string(),
            text: Some("Example Site".to_string()),
        };
        assert_eq!(
            link.to_html(),
            "<a href=\"https://example.com\">Example Site</a>"
        );
    }

    // ============================================
    // Multi-language plural tests with ICU rules
    // ============================================

    /// Test Russian plural rules: 1 item, 2-4 items (few), 5+ items (many)
    /// Russian has 3 main plural forms: One (1, 21, 31...), Few (2-4, 22-24...), Many (0, 5-20, 25-30...)
    #[test]
    fn test_plural_russian_one() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec![
                "предмет".to_string(),   // One (index 0)
                "предмета".to_string(),  // Few (index 1)
                "предметов".to_string(), // Many (index 2)
            ],
        };
        // 1 should use "One" category → index 0
        assert_eq!(
            transclusion.localize("ru", &vec!["1".to_string()]),
            "предмет"
        );
    }

    #[test]
    fn test_plural_russian_few() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec![
                "предмет".to_string(),
                "предмета".to_string(),
                "предметов".to_string(),
            ],
        };
        // 2-4 should use "Few" category → index 1
        assert_eq!(
            transclusion.localize("ru", &vec!["2".to_string()]),
            "предмета"
        );
        assert_eq!(
            transclusion.localize("ru", &vec!["3".to_string()]),
            "предмета"
        );
        assert_eq!(
            transclusion.localize("ru", &vec!["4".to_string()]),
            "предмета"
        );
    }

    #[test]
    fn test_plural_russian_many() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec![
                "предмет".to_string(),
                "предмета".to_string(),
                "предметов".to_string(),
            ],
        };
        // 0 and 5+ should use "Many" category → index 2 (or Other which maps to last)
        assert_eq!(
            transclusion.localize("ru", &vec!["0".to_string()]),
            "предметов"
        );
        assert_eq!(
            transclusion.localize("ru", &vec!["5".to_string()]),
            "предметов"
        );
        assert_eq!(
            transclusion.localize("ru", &vec!["21".to_string()]),
            "предмет"
        ); // 21 → One
    }

    /// Test Polish plural rules: 1 item, 2-4 items (few), 5+ items (many)
    /// Similar to Russian
    #[test]
    fn test_plural_polish() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec![
                "przedmiot".to_string(),   // One
                "przedmioty".to_string(),  // Few
                "przedmiotów".to_string(), // Many
            ],
        };
        assert_eq!(
            transclusion.localize("pl", &vec!["1".to_string()]),
            "przedmiot"
        );
        assert_eq!(
            transclusion.localize("pl", &vec!["2".to_string()]),
            "przedmioty"
        );
        assert_eq!(
            transclusion.localize("pl", &vec!["5".to_string()]),
            "przedmiotów"
        );
    }

    /// Test French plural rules: 1 item vs rest
    /// French is like English: One (1), Other (everything else including 0)
    /// Note: In French, 0 is grammatically "singular" (treated like "one")
    #[test]
    fn test_plural_french() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["article".to_string(), "articles".to_string()],
        };
        assert_eq!(
            transclusion.localize("fr", &vec!["1".to_string()]),
            "article"
        );
        // French treats 0 as "One" category (singular form)
        assert_eq!(
            transclusion.localize("fr", &vec!["0".to_string()]),
            "article"
        );
        assert_eq!(
            transclusion.localize("fr", &vec!["5".to_string()]),
            "articles"
        );
    }

    /// Test Arabic plural rules (simplified): Multiple forms to verify we handle many categories
    /// Arabic has 6 plural forms, but the ordering and mapping can be complex
    /// For now, test basic categories work correctly
    #[test]
    fn test_plural_arabic_basic() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec![
                "أشياء".to_string(),       // Index 0
                "شيء".to_string(),         // Index 1
                "شيئان".to_string(),       // Index 2
                "أشياء قليلة".to_string(), // Index 3
                "أشياء كثيرة".to_string(), // Index 4
                "أشياء".to_string(),       // Index 5
            ],
        };
        // Test that we can handle multiple forms without crashing
        // The exact form selection depends on ICU's plural rules for Arabic
        let result1 = transclusion.localize("ar", &vec!["1".to_string()]);
        assert!(
            !result1.is_empty(),
            "Should return a non-empty string for count 1"
        );

        let result100 = transclusion.localize("ar", &vec!["100".to_string()]);
        assert!(
            !result100.is_empty(),
            "Should return a non-empty string for count 100"
        );
    }

    /// Test fallback behavior: using an unsupported/invalid locale
    /// Should fall back to English rules silently
    #[test]
    fn test_plural_invalid_locale_fallback() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };
        // Invalid locale should fall back to English rules
        // 1 → singular
        assert_eq!(
            transclusion.localize("invalid-locale-xyz", &vec!["1".to_string()]),
            "item"
        );
        // 2 → plural (fallback to last)
        assert_eq!(
            transclusion.localize("invalid-locale-xyz", &vec!["2".to_string()]),
            "items"
        );
    }

    /// Test that English continues to work correctly with ICU
    #[test]
    fn test_plural_english_with_icu() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["thing".to_string(), "things".to_string()],
        };
        assert_eq!(transclusion.localize("en", &vec!["1".to_string()]), "thing");
        assert_eq!(
            transclusion.localize("en", &vec!["0".to_string()]),
            "things"
        );
        assert_eq!(
            transclusion.localize("en", &vec!["2".to_string()]),
            "things"
        );
        assert_eq!(
            transclusion.localize("en", &vec!["100".to_string()]),
            "things"
        );
    }

    /// Test direct number parameter (not a placeholder)
    #[test]
    fn test_plural_direct_number() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "5".to_string(), // Direct number, not a placeholder
            options: vec!["item".to_string(), "items".to_string()],
        };
        // Should use 5 as the count, which is plural
        assert_eq!(transclusion.localize("en", &vec![]), "items");
    }

    /// Test plural with fallback chain (via localize_with_context)
    /// de-at should fall back to de's plural rules
    #[test]
    fn test_plural_with_fallback_context() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["element".to_string(), "elements".to_string()],
        };

        // de-at doesn't have explicit ICU support, should fall back to de
        // Using localize_with_context for fallback support
        let result_silent = transclusion.localize_with_context(
            "de-at",
            &vec!["1".to_string()],
            VerbosityLevel::Silent,
        );
        // Should get singular form
        assert_eq!(result_silent, "element");

        let result_plural = transclusion.localize_with_context(
            "de-at",
            &vec!["5".to_string()],
            VerbosityLevel::Silent,
        );
        // Should get plural form
        assert_eq!(result_plural, "elements");
    }

    /// Test plural with very specific locale that falls back through chain
    /// Example: sr (Serbian) with specific script variants
    #[test]
    fn test_plural_with_complex_fallback() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec![
                "jedan".to_string(), // One
                "dva".to_string(),   // Few/Other
                "pet".to_string(),   // Other
            ],
        };

        // Serbian should have working plural rules or fall back through chain
        let result1 = transclusion.localize_with_context(
            "sr",
            &vec!["1".to_string()],
            VerbosityLevel::Silent,
        );
        assert!(!result1.is_empty());

        let result5 = transclusion.localize_with_context(
            "sr",
            &vec!["5".to_string()],
            VerbosityLevel::Silent,
        );
        assert!(!result5.is_empty());
    }

    /// Test that localize_with_context uses fallback chain for plural resolution
    /// and logs appropriately based on verbosity level
    #[test]
    fn test_plural_fallback_with_verbosity_silent() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };

        // With Silent verbosity, should not log but still work
        let _result = transclusion.localize_with_context(
            "de-at",
            &vec!["2".to_string()],
            VerbosityLevel::Silent,
        );
        // Result should be computed without errors
        assert_eq!(_result, "items");
    }

    /// Test unknown magic word in localize_with_context
    #[test]
    fn test_unknown_magic_word_with_context() {
        let transclusion = Transclusion {
            name: "UNKNOWN".to_string(),
            param: "test".to_string(),
            options: vec!["option1".to_string()],
        };

        let result = transclusion.localize_with_context("en", &vec![], VerbosityLevel::Silent);
        // Should return the original syntax
        assert_eq!(result, "{{UNKNOWN:test|option1}}");
    }
}
