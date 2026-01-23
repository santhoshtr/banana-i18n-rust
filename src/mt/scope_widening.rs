//! Scope Widening Detection and Helper Functions
//!
//! This module provides utilities for detecting when machine translation changes
//! words outside the original magic word boundaries and helps determine appropriate
//! scope expansions.

use super::error::MtResult;

/// Detects if a sequence of changes forms a continuous block
///
/// Returns the start and end positions of continuous change ranges
pub fn find_continuous_changes(changes: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if changes.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut sorted_changes = changes.to_vec();
    sorted_changes.sort_by_key(|c| c.0);

    let mut current_start = sorted_changes[0].0;
    let mut current_end = sorted_changes[0].1;

    for &(start, end) in &sorted_changes[1..] {
        if start <= current_end {
            // Overlapping or adjacent - merge
            current_end = current_end.max(end);
        } else {
            // Gap detected - save current range and start new one
            result.push((current_start, current_end));
            current_start = start;
            current_end = end;
        }
    }

    result.push((current_start, current_end));
    result
}

/// Calculate the minimal scope that includes all changed positions
///
/// Given a list of change ranges, returns the minimal continuous range
/// that encompasses all of them
pub fn calculate_expanded_scope(changes: &[(usize, usize)]) -> Option<(usize, usize)> {
    if changes.is_empty() {
        return None;
    }

    let min_start = changes.iter().map(|(s, _)| *s).min()?;
    let max_end = changes.iter().map(|(_, e)| *e).max()?;

    Some((min_start, max_end))
}

/// Expand a range to include nearby word boundaries
///
/// Finds word boundaries (whitespace) around the given range and expands
/// to the nearest word boundaries for more natural results
pub fn expand_to_word_boundaries(text: &str, start: usize, end: usize) -> MtResult<(usize, usize)> {
    let bytes = text.as_bytes();
    let len = bytes.len();

    // Validate inputs
    if start > len || end > len || start > end {
        return Ok((start, end));
    }

    // Expand backward to previous whitespace
    let mut expanded_start = start;
    while expanded_start > 0 && !bytes[expanded_start - 1].is_ascii_whitespace() {
        expanded_start -= 1;
    }

    // Expand forward to next whitespace
    let mut expanded_end = end;
    while expanded_end < len && !bytes[expanded_end].is_ascii_whitespace() {
        expanded_end += 1;
    }

    Ok((expanded_start, expanded_end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_continuous_changes_single() {
        let changes = vec![(0, 5)];
        let result = find_continuous_changes(&changes);
        assert_eq!(result, vec![(0, 5)]);
    }

    #[test]
    fn test_find_continuous_changes_adjacent() {
        let changes = vec![(0, 5), (5, 10)];
        let result = find_continuous_changes(&changes);
        assert_eq!(result, vec![(0, 10)]);
    }

    #[test]
    fn test_find_continuous_changes_gap() {
        let changes = vec![(0, 5), (10, 15)];
        let result = find_continuous_changes(&changes);
        assert_eq!(result, vec![(0, 5), (10, 15)]);
    }

    #[test]
    fn test_find_continuous_changes_overlapping() {
        let changes = vec![(0, 5), (3, 8)];
        let result = find_continuous_changes(&changes);
        assert_eq!(result, vec![(0, 8)]);
    }

    #[test]
    fn test_calculate_expanded_scope_single() {
        let changes = vec![(5, 10)];
        let result = calculate_expanded_scope(&changes);
        assert_eq!(result, Some((5, 10)));
    }

    #[test]
    fn test_calculate_expanded_scope_multiple() {
        let changes = vec![(2, 5), (10, 15)];
        let result = calculate_expanded_scope(&changes);
        assert_eq!(result, Some((2, 15)));
    }

    #[test]
    fn test_calculate_expanded_scope_empty() {
        let changes = vec![];
        let result = calculate_expanded_scope(&changes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_expand_to_word_boundaries() -> MtResult<()> {
        let text = "The apple is red";
        let (start, end) = expand_to_word_boundaries(text, 4, 9)?;

        // Should expand to word boundaries
        assert!(start <= 4);
        assert!(end >= 9);

        Ok(())
    }

    #[test]
    fn test_expand_to_word_boundaries_at_boundaries() -> MtResult<()> {
        let text = "The apple is red";
        let (start, end) = expand_to_word_boundaries(text, 0, 3)?;

        // Already at boundaries
        assert_eq!(start, 0);
        assert!(end >= 3);

        Ok(())
    }
}
