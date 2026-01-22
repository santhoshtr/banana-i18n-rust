use crate::LocalizedMessages;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Load messages from a single JSON file
///
/// The JSON file should have the following structure:
/// ```json
/// {
///     "@metadata": { ... },  // Ignored
///     "message-key": "message text",
///     "another-key": "another message"
/// }
/// ```
///
/// # Arguments
/// * `path` - Path to the JSON file
///
/// # Returns
/// A `LocalizedMessages` struct containing all non-metadata messages
///
/// # Errors
/// - File not found
/// - Invalid JSON
/// - File read errors
pub fn load_messages_from_file(path: &Path) -> Result<LocalizedMessages, String> {
    // Read the file
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e))?;

    // Parse JSON
    let json: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON from '{}': {}", path.display(), e))?;

    // Ensure it's an object
    let obj = json.as_object().ok_or_else(|| {
        format!(
            "Invalid JSON in '{}': root must be an object",
            path.display()
        )
    })?;

    // Extract messages, skipping @metadata
    let mut messages = LocalizedMessages::new();
    for (key, value) in obj {
        // Skip metadata
        if key.starts_with('@') {
            continue;
        }

        // Extract string value
        if let Some(message) = value.as_str() {
            messages.with_message(key, message);
        } else {
            eprintln!("Warning: Message '{}' is not a string, skipping", key);
        }
    }

    Ok(messages)
}

/// Load all messages from a directory of JSON files
///
/// Scans the directory for all `*.json` files and loads them.
/// The filename (without extension) is used as the locale code.
/// For example: `en.json` -> locale `"en"`, `zh-hans.json` -> locale `"zh-hans"`
///
/// # Arguments
/// * `dir` - Directory path containing JSON files
///
/// # Returns
/// A HashMap mapping locale codes to LocalizedMessages
///
/// # Errors
/// - Directory not found
/// - File read/parse errors
pub fn load_all_messages_from_dir(
    dir: &Path,
) -> Result<HashMap<String, LocalizedMessages>, String> {
    // Check if directory exists
    if !dir.exists() {
        return Err(format!("Directory not found: {}", dir.display()));
    }

    if !dir.is_dir() {
        return Err(format!("Path is not a directory: {}", dir.display()));
    }

    let mut all_messages = HashMap::new();

    // Read directory entries
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {}", dir.display(), e))?;

    // Process each file
    for entry in entries {
        let entry = entry.map_err(|e| format!("Error reading directory entry: {}", e))?;

        let path = entry.path();

        // Only process JSON files
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        // Extract locale from filename (e.g., "en.json" -> "en")
        let locale = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("Invalid filename: {}", path.display()))?
            .to_string();

        // Load messages from file
        let messages = load_messages_from_file(&path)?;

        all_messages.insert(locale, messages);
    }

    if all_messages.is_empty() {
        eprintln!(
            "Warning: No JSON files found in directory {}",
            dir.display()
        );
    }

    Ok(all_messages)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_loader_module_exists() {
        // Loader module exists and can be compiled
    }
}
