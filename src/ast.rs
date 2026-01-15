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
    fn localize(&self, values: &Vec<String>) -> String;
}

impl Localizable for Placeholder {
    fn localize(&self, values: &Vec<String>) -> String {
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
    fn localize(&self, values: &Vec<String>) -> String {
        match self.name.to_uppercase().as_str() {
            "PLURAL" => self.localize_plural(values),
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
    fn localize_plural(&self, values: &Vec<String>) -> String {
        // Extract the count from param (e.g., "$1" -> values[0])
        let count = if self.param.starts_with('$') {
            // It's a placeholder reference
            let index_str = &self.param[1..];
            let index: usize = index_str.parse().unwrap_or(0);
            if index > 0 && index <= values.len() {
                values[index - 1].parse::<i32>().unwrap_or(0)
            } else {
                0
            }
        } else {
            // Direct number
            self.param.parse::<i32>().unwrap_or(0)
        };

        // Simple English plural rule: 1 = singular (index 0), others = plural (index 1 or last)
        let form_index = if count == 1 {
            0
        } else {
            1.min(self.options.len() - 1)
        };

        self.options
            .get(form_index)
            .cloned()
            .unwrap_or_else(|| self.options.last().cloned().unwrap_or_default())
    }
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
        assert_eq!(placeholder.localize(&values), "World");
    }

    #[test]
    fn test_placeholder_missing_value() {
        let placeholder = Placeholder { index: 5 };
        let values = vec!["World".to_string()];
        assert_eq!(placeholder.localize(&values), "$5");
    }

    #[test]
    fn test_plural_singular() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };
        let values = vec!["1".to_string()];
        assert_eq!(transclusion.localize(&values), "item");
    }

    #[test]
    fn test_plural_plural() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };
        let values = vec!["5".to_string()];
        assert_eq!(transclusion.localize(&values), "items");
    }

    #[test]
    fn test_plural_zero() {
        let transclusion = Transclusion {
            name: "PLURAL".to_string(),
            param: "$1".to_string(),
            options: vec!["item".to_string(), "items".to_string()],
        };
        let values = vec!["0".to_string()];
        assert_eq!(transclusion.localize(&values), "items");
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
        assert_eq!(transclusion.localize(&values), "two"); // Falls back to the one-index
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
}
