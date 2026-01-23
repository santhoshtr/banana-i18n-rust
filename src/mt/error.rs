/// Error types for the Machine Translation module
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MtError {
    /// Error during anchor token operations
    AnchorTokenError(String),
    /// Error during expansion phase
    ExpansionError(String),
    /// Error during plural expansion (specific case of expansion)
    PluralExpansionError(String),
    /// Error during translation phase (API failures, invalid responses)
    TranslationError(String),
    /// Error during reassembly phase
    ReassemblyError(String),
    /// Invalid API configuration (missing keys, invalid credentials)
    ConfigError(String),
    /// Network or HTTP error (timeouts, connection failures)
    NetworkError(String),
    /// Invalid locale code or unsupported language
    InvalidLocale(String),
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
            MtError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            MtError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            MtError::InvalidLocale(msg) => write!(f, "Invalid locale: {}", msg),
            MtError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for MtError {}

/// Implement conversion from reqwest::Error to MtError
impl From<reqwest::Error> for MtError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            MtError::NetworkError(format!("Request timeout: {}", err))
        } else if err.is_connect() {
            MtError::NetworkError(format!("Connection failed: {}", err))
        } else if err.status().map_or(false, |s| s.is_client_error()) {
            MtError::ConfigError(format!("HTTP client error: {}", err))
        } else if err.status().map_or(false, |s| s.is_server_error()) {
            MtError::TranslationError(format!("HTTP server error: {}", err))
        } else {
            MtError::NetworkError(format!("HTTP error: {}", err))
        }
    }
}

/// Result type for MT operations
pub type MtResult<T> = Result<T, MtError>;
