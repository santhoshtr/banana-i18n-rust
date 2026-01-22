/// Machine Translation Module
///
/// This module provides machine translation suggestion capabilities for the banana-i18n library.
/// It implements the Cartesian Expansion and Diff-and-Capture algorithm to generate high-quality
/// translation suggestions while preserving complex wikitext features like PLURAL, GENDER, and
/// parameterized placeholders.
///
/// # Overview
///
/// The MT module consists of several components working together:
///
/// 1. **Expansion Engine** - Converts wikitext with magic words into all possible plain-text variants
/// 2. **MT Trait & Providers** - Generic trait for MT systems with Google Translate implementation
/// 3. **Reassembly Engine** - Reconstructs wikitext from translated variants using Diff-and-Capture
/// 4. **Consistency Checker** - Validates translations for hallucinations and anomalies
/// 5. **Suggestion Generator** - Orchestrates the full pipeline
///
/// # Example
///
/// ```ignore
/// use banana_i18n::mt::anchor::{generate_anchor_tokens, replace_placeholders_with_anchors};
///
/// let text = "Hello, $1! You have $2 messages.";
/// let anchors = generate_anchor_tokens(2);
/// let expanded = replace_placeholders_with_anchors(text, &anchors)?;
/// // Result: "Hello, _ID1_ ! You have _ID2_  messages."
/// ```
pub mod anchor;
pub mod error;
pub mod plural_expansion;

pub use anchor::{
    AnchorToken, generate_anchor_tokens, recover_placeholders_from_anchors,
    replace_placeholders_with_anchors,
};
pub use error::{MtError, MtResult};
pub use plural_expansion::{expand_plural_variants, get_plural_forms_for_language};
