use tree_sitter::{Node, Parser as TSParser};

use crate::ast::{
    AstNode, AstNodeList, Placeholder, Transclusion, WikiExternalLink, WikiInternalLink,
};

pub struct Parser {
    source: String,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        Parser {
            source: source.to_string(),
        }
    }

    pub fn parse(&mut self) -> AstNodeList {
        // Initialize tree-sitter parser
        let mut ts_parser = TSParser::new();
        match ts_parser.set_language(&tree_sitter_wikitext::LANGUAGE.into()) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error loading wikitext grammar: {}", e);
                // Fallback: return source as plain text
                return vec![AstNode::Text(self.source.clone())];
            }
        }

        // Parse the source
        let tree = match ts_parser.parse(&self.source, None) {
            Some(t) => t,
            None => {
                eprintln!("Warning: Failed to parse wikitext, returning as plain text");
                return vec![AstNode::Text(self.source.clone())];
            }
        };

        let root = tree.root_node();

        // Debug: print s-expression for development
        #[cfg(debug_assertions)]
        eprintln!("Parse tree s-expression: {}", root.to_sexp());

        // Walk the tree and build AST
        self.walk_node(root)
    }

    fn walk_node(&self, node: Node) -> AstNodeList {
        let mut ast_nodes = Vec::new();

        // Process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            ast_nodes.extend(self.process_node(child));
        }

        // If no children, process as leaf node
        if ast_nodes.is_empty() && node.child_count() == 0 {
            ast_nodes.extend(self.process_node(node));
        }

        ast_nodes
    }

    fn process_node(&self, node: Node) -> AstNodeList {
        let node_type = node.kind();

        match node_type {
            "parser_function" => self.parse_parser_function(node),
            "wikilink" => self.parse_wikilink(node),
            "external_link" => self.parse_external_link(node),
            "text" => self.parse_text(node),
            "document" | "paragraph" => self.walk_node(node),
            _ => {
                // Unknown node type - walk children or return text
                if node.child_count() > 0 {
                    self.walk_node(node)
                } else {
                    let text = self.node_text(node);
                    if !text.is_empty() {
                        vec![AstNode::Text(text)]
                    } else {
                        vec![]
                    }
                }
            }
        }
    }

    fn parse_parser_function(&self, node: Node) -> AstNodeList {
        // Parser function format: {{PLURAL:$1|is|are}}
        // Tree structure:
        // parser_function
        //   -> parser_function_colon
        //      -> parser_function_name (contains "PLURAL")
        //      -> function_delimiter (":")
        //      -> param_text ("$1")
        //      -> template_argument (contains "is")
        //      -> template_argument (contains "are")

        // Try to find the parser_function_colon child node
        let mut cursor = node.walk();
        let pf_colon = node
            .children(&mut cursor)
            .find(|child| child.kind() == "parser_function_colon");

        if let Some(pf_colon_node) = pf_colon {
            if let (Some(name), Some(param)) = (
                self.extract_parser_function_name(pf_colon_node),
                self.extract_parser_function_param(pf_colon_node),
            ) {
                let options = self.extract_parser_function_arguments(pf_colon_node);

                return vec![AstNode::Transclusion(Transclusion {
                    name,
                    param,
                    options,
                })];
            }
        }

        // If we can't parse as parser function, fall back to text
        let text = self.node_text(node);
        eprintln!(
            "Warning: Failed to parse parser function, returning as text: {}",
            text
        );
        vec![AstNode::Text(text)]
    }

    fn extract_parser_function_name(&self, pf_colon_node: Node) -> Option<String> {
        let mut cursor = pf_colon_node.walk();
        pf_colon_node
            .children(&mut cursor)
            .find(|child| child.kind() == "parser_function_name")
            .map(|name_node| self.node_text(name_node).trim().to_string())
    }

    fn extract_parser_function_param(&self, pf_colon_node: Node) -> Option<String> {
        let mut cursor = pf_colon_node.walk();
        pf_colon_node
            .children(&mut cursor)
            .find(|child| child.kind() == "param_text")
            .map(|param_node| self.node_text(param_node).trim().to_string())
    }

    fn extract_parser_function_arguments(&self, pf_colon_node: Node) -> Vec<String> {
        let mut arguments = Vec::new();
        let mut cursor = pf_colon_node.walk();

        for arg_node in pf_colon_node.children(&mut cursor) {
            if arg_node.kind() == "template_argument" {
                // template_argument contains template_param_value(s)
                let mut arg_cursor = arg_node.walk();
                let arg_text = arg_node
                    .children(&mut arg_cursor)
                    .find(|child| child.kind() == "template_param_value")
                    .map(|value_node| self.node_text(value_node).trim().to_string())
                    .unwrap_or_else(|| self.node_text(arg_node).trim().to_string());

                if !arg_text.is_empty() {
                    arguments.push(arg_text);
                }
            }
        }

        arguments
    }

    fn parse_wikilink(&self, node: Node) -> AstNodeList {
        let text = self.node_text(node);

        // Parse [[target]] or [[target|display]]
        if let Some(inner) = text.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
            let parts: Vec<&str> = inner.splitn(2, '|').collect();
            let target = parts[0].trim().to_string();
            let display_text = if parts.len() > 1 {
                Some(parts[1].trim().to_string())
            } else {
                None
            };

            return vec![AstNode::InternalLink(WikiInternalLink {
                target,
                display_text,
            })];
        }

        eprintln!("Warning: Failed to parse wikilink: {}", text);
        vec![AstNode::Text(text)]
    }

    fn parse_external_link(&self, node: Node) -> AstNodeList {
        let text = self.node_text(node);

        // Parse [URL text] format
        if let Some(inner) = text.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let parts: Vec<&str> = inner.splitn(2, ' ').collect();
            let url = parts[0].trim().to_string();
            let link_text = parts.get(1).map(|s| s.trim().to_string());

            return vec![AstNode::ExternalLink(WikiExternalLink {
                url,
                text: link_text,
            })];
        }

        eprintln!("Warning: Failed to parse external link: {}", text);
        vec![AstNode::Text(text)]
    }

    fn parse_text(&self, node: Node) -> AstNodeList {
        let text = self.node_text(node);

        // Check for placeholders like $1, $2, etc.
        self.extract_placeholders(&text)
    }

    fn extract_placeholders(&self, text: &str) -> AstNodeList {
        let mut nodes = Vec::new();
        let mut current_text = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' {
                // Check if followed by digits
                let mut digits = String::new();
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_ascii_digit() {
                        digits.push(next_ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                if !digits.is_empty() {
                    // Found a placeholder
                    if !current_text.is_empty() {
                        nodes.push(AstNode::Text(current_text.clone()));
                        current_text.clear();
                    }

                    let index: usize = digits.parse().unwrap_or(0);
                    nodes.push(AstNode::Placeholder(Placeholder { index }));
                } else {
                    // Just a '$' character
                    current_text.push('$');
                }
            } else {
                current_text.push(ch);
            }
        }

        if !current_text.is_empty() {
            nodes.push(AstNode::Text(current_text));
        }

        if nodes.is_empty() && !text.is_empty() {
            nodes.push(AstNode::Text(text.to_string()));
        }

        nodes
    }

    fn node_text(&self, node: Node) -> String {
        node.utf8_text(self.source.as_bytes())
            .unwrap_or("")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_parsing() {
        let mut parser = Parser::new("$1");
        let ast = parser.parse();
        assert!(!ast.is_empty());
        match &ast[0] {
            AstNode::Placeholder(p) => assert_eq!(p.index, 1),
            _ => panic!("Expected placeholder, got {:?}", ast[0]),
        }
    }

    #[test]
    fn test_multiple_placeholders() {
        let mut parser = Parser::new("Hello, $1! Goodbye, $2!");
        let ast = parser.parse();
        assert!(ast.len() >= 4); // At least: "Hello, ", placeholder, "! Goodbye, ", placeholder, "!"
    }

    #[test]
    fn test_simple_template() {
        let mut parser = Parser::new("{{PLURAL:$1|is|are}}");
        let ast = parser.parse();
        assert!(!ast.is_empty());
        match &ast[0] {
            AstNode::Transclusion(t) => {
                assert_eq!(t.name, "PLURAL");
                assert_eq!(t.param, "$1");
                assert_eq!(t.options, vec!["is", "are"]);
            }
            _ => panic!("Expected transclusion, got {:?}", ast[0]),
        }
    }

    #[test]
    fn test_internal_link() {
        let mut parser = Parser::new("[[box]]");
        let ast = parser.parse();
        let link = ast.iter().find_map(|node| match node {
            AstNode::InternalLink(l) => Some(l),
            _ => None,
        });

        if let Some(link) = link {
            assert_eq!(link.target, "box");
            assert_eq!(link.to_html(), "<a href=\"box\">box</a>");
        } else {
            panic!("Expected internal link in AST: {:?}", ast);
        }
    }

    #[test]
    fn test_internal_link_with_display() {
        let mut parser = Parser::new("[[Main Page|home]]");
        let ast = parser.parse();
        let link = ast.iter().find_map(|node| match node {
            AstNode::InternalLink(l) => Some(l),
            _ => None,
        });

        if let Some(link) = link {
            assert_eq!(link.target, "Main Page");
            assert_eq!(link.display_text, Some("home".to_string()));
        } else {
            panic!("Expected internal link in AST: {:?}", ast);
        }
    }

    #[test]
    fn test_external_link() {
        let mut parser = Parser::new("[https://example.com]");
        let ast = parser.parse();
        let link = ast.iter().find_map(|node| match node {
            AstNode::ExternalLink(l) => Some(l),
            _ => None,
        });

        if let Some(link) = link {
            assert_eq!(link.url, "https://example.com");
        } else {
            panic!("Expected external link in AST: {:?}", ast);
        }
    }

    #[test]
    fn test_plain_text() {
        let mut parser = Parser::new("Hello, World!");
        let ast = parser.parse();
        assert!(!ast.is_empty());
        match &ast[0] {
            AstNode::Text(t) => assert_eq!(t, "Hello, World!"),
            _ => panic!("Expected text node, got {:?}", ast[0]),
        }
    }
}
