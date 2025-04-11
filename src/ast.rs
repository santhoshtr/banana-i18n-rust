use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum AstNode {
    Text(String),
    Placeholder(Placeholder),
    Transclusion(Transclusion),
}

#[derive(Debug, PartialEq)]
pub struct Placeholder {
    pub name: String,
    pub index: i32,
}

pub trait Localizable {
    fn localize(&self, values: &Vec<String>) -> String;
}

impl Placeholder {
    pub fn new(name: String) -> Result<Self, String> {
        let index = name[1..]
            .parse::<i32>()
            .map_err(|_| "Failed to parse index")?;
        Ok(Placeholder {
            name,
            index: index - 1,
        })
    }
}

impl Localizable for Placeholder {
    fn localize(&self, values: &Vec<String>) -> String {
        if let Some(value) = values.get(self.index as usize) {
            value.clone()
        } else {
            format!("{}|{}", self.name, self.index)
        }
    }
}

impl std::fmt::Display for Placeholder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, PartialEq)]
pub struct Transclusion {
    title: String,
    placeholder: Option<Placeholder>,
    parts: HashMap<String, String>, // (name, value) pairs
}

impl Transclusion {
    pub fn new(
        title: String,
        placeholder: Option<Placeholder>,
        parts: HashMap<String, String>,
    ) -> Self {
        Transclusion {
            title,
            placeholder,
            parts,
        }
    }

    pub fn to_string(&self) -> String {
        let mut result = self.title.clone();
        if let Some(placeholder) = &self.placeholder {
            result.push_str(&format!("|{}={}", placeholder.name, placeholder.index));
        }
        for (name, value) in &self.parts {
            result.push_str(&format!("|{}={}", name, value));
        }
        result
    }
}

impl Localizable for Transclusion {
    fn localize(&self, values: &Vec<String>) -> String {
        let mut result = String::new();
        if let Some(placeholder) = &self.placeholder {
            if let Some(value) = values.get(placeholder.index as usize) {
                // If the placeholder index is valid, return the corresponding part
                if let Some(part) = self.parts.get(value) {
                    result.push_str(part);
                } else {
                    // If the part doesn't exist, return the title and the value
                    println!("{:?}", values);
                    result.push_str(&format!("{}={}", value, value));
                }
            } else {
                // If the index is out of bounds, return the title and the placeholder
                result.push_str(&format!("{}|{}", self.title, placeholder.name));
            }
        } else {
            // If there's no placeholder, return the title and all parts
            result.push_str(self.to_string().as_str());
        }

        result
    }
}

impl std::fmt::Display for Transclusion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut result = self.title.clone();
        if let Some(placeholder) = &self.placeholder {
            result.push_str(&format!("|{}={}", placeholder.name, placeholder.index));
        }
        for (name, value) in &self.parts {
            result.push_str(&format!("|{}={}", name, value));
        }
        write!(f, "{}", result)
    }
}

pub struct PluralTransclusion(pub Transclusion);
impl PluralTransclusion {
    pub fn new(transclusion: Transclusion) -> Self {
        PluralTransclusion(transclusion)
    }
}

impl Localizable for PluralTransclusion {
    fn localize(&self, values: &Vec<String>) -> String {
        todo!("Use CLDR Plural rules to localize the transclusion");
    }
}

pub struct GenderTransclusion(pub Transclusion);
impl GenderTransclusion {
    pub fn new(transclusion: Transclusion) -> Self {
        GenderTransclusion(transclusion)
    }
}

impl Localizable for GenderTransclusion {
    fn localize(&self, values: &Vec<String>) -> String {
        todo!("Use Gender rules to localize the transclusion");
    }
}

#[derive(Debug, PartialEq)]
pub struct AstNodeList(pub Vec<AstNode>);

impl AstNodeList {
    pub fn new() -> Self {
        AstNodeList(Vec::new())
    }

    pub fn push(&mut self, node: AstNode) {
        self.0.push(node);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&AstNode> {
        self.0.get(index)
    }
}

impl IntoIterator for AstNodeList {
    type Item = AstNode;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a AstNodeList {
    type Item = &'a AstNode;
    type IntoIter = std::slice::Iter<'a, AstNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut AstNodeList {
    type Item = &'a mut AstNode;
    type IntoIter = std::slice::IterMut<'a, AstNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}
