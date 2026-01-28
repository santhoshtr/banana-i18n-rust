//! Machine Translation Module
//!
//! This module provides machine translation capabilities for the banana-i18n library
//! following the design patterns from the Python reference implementation. It implements
//! a simplified and effective approach to MT-assisted localization while preserving
//! complex wikitext features like PLURAL, GENDER, and parameterized placeholders.
//!
//! # Architecture Overview
//!
//! The MT module consists of several focused components:
//!
//! 1. **Data Structures** (`data`) - Core types: `TranslationVariant`, `MessageContext`
//! 2. **Expansion Engine** (`expansion`) - Converts wikitext into variants with cartesian product
//! 3. **Translation Providers** (`translator`, `google_translate`, `mock`) - MT system integrations
//! 4. **Reassembly Engine** (`reassembly`) - Reconstructs wikitext using axis-collapsing algorithm
//! 5. **Error Handling** (`error`) - Comprehensive error types for the MT pipeline
//!
//! # Design Philosophy
//!
//! This implementation prioritizes simplicity and correctness over complexity:
//! - **Data-driven**: Simple structs instead of complex enums
//! - **Algorithm focus**: Implements proven algorithms from Python reference
//! - **Type safety**: Leverages Rust's type system without over-engineering
//! - **Testability**: Clear separation of concerns enables thorough testing
//!
//! # Workflow Example
//!
//! ```ignore
//! use banana_i18n::mt::{
//!     MachineTranslator, GoogleTranslateProvider,
//!     prepare_for_translation, Reassembler
//! };
//! use banana_i18n::parser::Parser;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 1. Parse message
//!     let mut parser = Parser::new("{{GENDER:$1|He|She}} sent {{PLURAL:$2|a message|$2 messages}}");
//!     let ast = parser.parse();
//!
//!     // 2. Prepare for translation (expand to all variants)
//!     let mut context = prepare_for_translation(&ast, "en", "user-message")?;
//!
//!     // 3. Translate using block translation for consistency
//!     let provider = GoogleTranslateProvider::from_env()?;
//!     let source_texts = context.source_texts();
//!     let translated_texts = provider.translate_as_block(&source_texts, "en", "fr").await?;
//!     context.update_translations(translated_texts);
//!
//!     // 4. Reassemble back to wikitext
//!     let reassembler = Reassembler::new(context.variable_types.clone());
//!     let final_wikitext = reassembler.reassemble(context.variants)?;
//!
//!     println!("Result: {}", final_wikitext);
//!     // Output: "{{GENDER:$1|Il|Elle}} a envoyé {{PLURAL:$2|un message|$2 messages}}"
//!     Ok(())
//! }
//! ```
//!
//! # Key Features
//!
//! - **Consistency Checking**: Detects MT hallucinations using similarity thresholds
//! - **Word Boundary Snapping**: Ensures clean wikitext reconstruction
//! - **Anchor Token Protection**: Prevents MT corruption of placeholders ($1 → _ID1_)
//! - **Block Translation**: Translates related variants together for consistency
//! - **ICU Plural Support**: Handles complex plural rules for 50+ languages

// Core module declarations
pub mod data;
pub mod error;
pub mod expansion;
pub mod google_translate;
pub mod mock;
pub mod reassembly;
pub mod translator;

// Integration tests (only available during testing)
#[cfg(test)]
mod integration_tests;

// Public API exports - simplified and focused
pub use data::{MessageContext, TranslationVariant};
pub use error::{MtError, MtResult};
pub use expansion::{
    GenderForm, PluralForm, expand_to_variants, get_gender_forms, get_plural_forms_for_language,
    prepare_for_translation,
};
pub use google_translate::GoogleTranslateProvider;
pub use mock::{MockMode, MockTranslator};
pub use reassembly::{Reassembler, get_similarity, reassemble_from_context};
pub use translator::MachineTranslator;
