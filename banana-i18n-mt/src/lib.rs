//! Machine Translation support for banana-i18n
//!
//! This crate provides MT-assisted translation workflows for MediaWiki-style messages
//! # Workflow Example
//!
//! ```ignore
//! use banana_i18n_mt::{
//!     GoogleTranslateProvider, prepare_for_translation, Reassembler, MachineTranslator
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
//!     Ok(())
//! }
//! ```

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

// Re-export main types for convenient access
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
