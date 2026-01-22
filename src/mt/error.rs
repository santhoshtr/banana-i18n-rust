/// Error types for the Machine Translation module
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MtError {
    /// Error during anchor token operations
    AnchorTokenError(String),
    /// Error during expansion phase
    ExpansionError(String),
    /// Error during plural expansion (specific case of expansion)
    PluralExpansionError(String),
    /// Error during translation phase
    TranslationError(String),
    /// Error during reassembly phase
    ReassemblyError(String),
    /// General error with context
    Other(String),
}

impl std::fmt::Display for MtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MtError::AnchorTokenError(msg) => write!(f, "Anchor token error: {}", msg),
            MtError::ExpansionError(msg) => write!(f, "Expansion error: {}", msg),
            MtError::PluralExpansionError(msg) => write!(f, "Plural expansion error: {}", msg),
            MtError::TranslationError(msg) => write!(f, "Translation error: {}", msg),
            MtError::ReassemblyError(msg) => write!(f, "Reassembly error: {}", msg),
            MtError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for MtError {}

/// Result type for MT operations
pub type MtResult<T> = Result<T, MtError>;
