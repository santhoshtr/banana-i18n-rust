use std::collections::HashMap;

use crate::ast::{AstNode, AstNodeList, Placeholder, Transclusion};

/// A parser for MediaWiki-style transclusions and placeholders.
///
/// This parser is designed to process a string input and convert it into an abstract syntax tree (AST)
pub struct Parser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Parser { input, position: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.input.chars().nth(self.position)
    }

    /// Consumes the current character and advances the position.
    /// Returns the character that was consumed, or None if at the end of input.
    fn consume(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.position += 1;
        }
        c
    }

    fn parse_text(&mut self) -> AstNode {
        let start = self.position;
        while let Some(c) = self.peek() {
            if c == '{' {
                break;
            }
            if c == '$' {
                break;
            }
            self.consume();
        }
        let text = &self.input[start..self.position];
        AstNode::Text(text.to_string())
    }

    fn parse_placeholder(&mut self) -> Option<AstNode> {
        if self.peek() != Some('$') {
            return None;
        }

        let start = self.position;
        self.consume(); // Consume '$'
        while let Some(c) = self.peek() {
            if !c.is_digit(10) {
                break;
            }
            self.consume();
        }
        let name = &self.input[start..self.position];

        match Placeholder::new(name.to_string()) {
            Ok(placeholder) => Some(AstNode::Placeholder(placeholder)),
            Err(_) => None, // Handle the error case appropriately
        }
    }

    fn parse_balanced_text(&mut self) -> String {
        let mut text = String::new();
        let mut brace_count = 0;
        while let Some(c) = self.peek() {
            if c == '{' {
                brace_count += 1;
                text.push(c);
                self.consume();
            } else if c == '}' {
                brace_count -= 1;
                if brace_count < 0 {
                    // End of balanced text
                    break;
                }
                text.push(c);
                self.consume();
            } else if c == '|' {
                break;
            } else if c == ':' {
                break;
            } else {
                text.push(c);
                self.consume();
            }
        }

        // Remove consecutive braces
        let mut result = String::new();
        let mut prev_brace = false;
        for c in text.chars() {
            if c == '{' || c == '}' {
                if !prev_brace {
                    result.push(c);
                    prev_brace = true;
                }
            } else {
                result.push(c);
                prev_brace = false;
            }
        }
        result
    }

    /// Parses a transclusion from the input string.
    ///
    /// A transclusion is a construct enclosed in double or triple braces (`{{...}}` or `{{{...}}}`),
    /// which may contain a title, an optional placeholder, and a set of named or indexed parts.
    ///
    /// # Returns
    ///
    /// - `Some(AstNode::Transclusion)` if a valid transclusion is successfully parsed.
    /// - `None` if the transclusion is invalid or cannot be parsed.
    ///
    /// # Parsing Rules
    ///
    /// 1. The function first determines whether the transclusion uses double or triple braces.
    /// 2. The title of the transclusion is parsed as balanced text.
    /// 3. If a colon (`:`) follows the title, an optional placeholder is parsed.
    /// 4. Named or indexed parts are parsed, separated by `|`. Named parts use the format `name=value`,
    ///    while indexed parts are assigned sequential numeric keys starting from 1.
    /// 5. The function ensures that the transclusion is properly closed with the same number of braces
    ///    as it started with.
    ///
    /// # Error Handling
    ///
    /// - If the transclusion is invalid (e.g., mismatched braces or malformed parts), the function
    ///   backtracks to the starting position and returns `None`.
    ///
    /// # Example
    ///
    /// Given the input `{{Title|1=value1|2=value2}}`, the function parses:
    ///
    /// - Title: `"Title"`
    /// - Parts: `{"1": "value1", "2": "value2"}`
    ///
    /// Given the input `{{{Title:Placeholder|name=value}}}`, the function parses:
    ///
    /// - Title: `"Title"`
    /// - Placeholder: `"Placeholder"`
    /// - Parts: `{"name": "value"}`
    fn parse_transclusion(&mut self) -> Option<AstNode> {
        let triple_braces = self.input.get(self.position..self.position + 3) == Some("{{{");
        let brace_count = if triple_braces { 3 } else { 2 };
        let start_pos = self.position;

        // Consume opening braces
        for _ in 0..brace_count {
            if self.consume() != Some('{') {
                return None; // Should not happen as caller should have checked
            }
        }

        let title = self.parse_balanced_text();
        let mut placeholder: Option<Placeholder> = None;
        if self.peek() == Some(':') {
            self.consume(); // Consume ':'
            placeholder = Placeholder::new(self.parse_balanced_text()).ok();
        }
        let mut parts: HashMap<String, String> = HashMap::new();
        let mut part_index = 1;
        while self.peek() == Some('|') {
            self.consume(); // Consume '|'

            let name = self.parse_balanced_text();
            if self.peek() == Some('=') {
                self.consume(); // Consume '='
                let value = self.parse_balanced_text();
                parts.insert(name.clone(), value);
            } else {
                parts.insert(part_index.to_string(), name);
                part_index += 1;
            };
        }

        // Consume closing braces
        for _ in 0..brace_count {
            if self.consume() != Some('}') {
                // Invalid transclusion, backtrack and treat as plain text.
                self.position = start_pos;
                return None;
            }
        }

        Some(AstNode::Transclusion(Transclusion::new(
            title,
            placeholder,
            parts,
        )))
    }

    pub fn parse(&mut self) -> AstNodeList {
        let mut ast: AstNodeList = AstNodeList::new();
        while self.position < self.input.len() {
            match self.peek() {
                Some('{') => {
                    if self.input.get(self.position..self.position + 2) == Some("{{") {
                        if let Some(node) = self.parse_transclusion() {
                            ast.push(node);
                        } else {
                            // Treat as text if transclusion parsing fails
                            ast.push(self.parse_text());
                        }
                    } else {
                        // Treat as text if not a transclusion
                        ast.push(self.parse_text());
                    }
                }
                Some('$') => {
                    if let Some(node) = self.parse_placeholder() {
                        ast.push(node);
                    } else {
                        ast.push(self.parse_text());
                    }
                }
                _ => ast.push(self.parse_text()),
            }
        }
        ast
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::AstNodeList;

    #[test]
    fn test_parser() {
        let input = "Hello, $1! {{PLURAL:$1|is|are}} $1 {{PLURAL:$1|item|items}} in the box";
        let mut parser = Parser::new(input);
        let ast: AstNodeList = parser.parse();
        assert_eq!(ast.len(), 9);
        let mut parts = HashMap::new();
        parts.insert("2".to_string(), "are".to_string());
        parts.insert("1".to_string(), "is".to_string());
        assert_eq!(
            ast.get(3),
            Some(AstNode::Transclusion(Transclusion::new(
                "PLURAL".to_string(),
                Some(Placeholder {
                    name: "$1".to_string(),
                    index: 0
                }),
                parts
            )))
            .as_ref()
        );
        assert_eq!(
            ast.get(0),
            Some(AstNode::Text("Hello, ".to_string())).as_ref()
        );
    }

    #[test]
    fn test_parse_text() {
        let input = "Hello, World!";
        let mut parser = Parser::new(input);
        let node = parser.parse_text();
        assert_eq!(node, AstNode::Text("Hello, World!".to_string()));
    }

    #[test]
    fn test_parse_placeholder() {
        let input = "$1";
        let mut parser = Parser::new(input);
        let node = parser.parse_placeholder();
        assert_eq!(
            node,
            Some(AstNode::Placeholder(
                Placeholder::new("$1".to_string()).unwrap()
            ))
        );
    }

    #[test]
    fn test_parse_transclusion() {
        let input = "{{{title|param1=value1|param2=value2}}}";
        let mut parser = Parser::new(input);
        let node = parser.parse_transclusion();
        assert!(node.is_some());

        assert_eq!(
            node.unwrap(),
            AstNode::Transclusion(Transclusion::new(
                "title".to_string(),
                None,
                vec![
                    ("param1".to_string(), "value1".to_string()),
                    ("param2".to_string(), "value2".to_string())
                ]
                .into_iter()
                .collect()
            ))
        );
    }

    #[test]
    fn test_parse_balanced_text() {
        let input = "{{{title|param1=value1|param2=value2}}}";
        let mut parser = Parser::new(input);
        let text = parser.parse_balanced_text();
        assert_eq!(text, "{{{title|param1=value1|param2=value2}}}");
    }
}
