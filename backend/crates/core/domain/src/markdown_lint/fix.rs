use std::ops::Range;

use thiserror::Error;

use super::document::{ParsedMarkdownError, validate_range};
use super::types::TextEdit;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum FixError {
    #[error(transparent)]
    InvalidRange(#[from] ParsedMarkdownError),
    #[error("text edits overlap")]
    OverlappingEdits,
    #[error("text edits change the number of source lines")]
    LineCountChanged,
}

pub fn apply_safe_edits(source: &str, edits: &[TextEdit]) -> Result<String, FixError> {
    let mut ordered = edits.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|edit| edit.range.start);

    let mut previous: Option<&Range<usize>> = None;
    for edit in &ordered {
        validate_range(source, &edit.range)?;
        if previous
            .is_some_and(|range| range.end > edit.range.start || range.start == edit.range.start)
        {
            return Err(FixError::OverlappingEdits);
        }
        previous = Some(&edit.range);
    }

    let original_line_breaks = line_break_count(source);
    let mut result = source.to_string();
    for edit in ordered.into_iter().rev() {
        result.replace_range(edit.range.clone(), &edit.replacement);
    }
    if line_break_count(&result) != original_line_breaks {
        return Err(FixError::LineCountChanged);
    }
    Ok(result)
}

fn line_break_count(source: &str) -> usize {
    let mut count = 0;
    let mut previous_was_carriage_return = false;
    for character in source.chars() {
        match character {
            '\r' => {
                count += 1;
                previous_was_carriage_return = true;
            }
            '\n' => {
                if !previous_was_carriage_return {
                    count += 1;
                }
                previous_was_carriage_return = false;
            }
            _ => previous_was_carriage_return = false,
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{FixError, apply_safe_edits};
    use crate::markdown_lint::TextEdit;

    #[rstest]
    #[case::reverse_application("abcdef", vec![edit(1..2, "XX"), edit(4..6, "Y")], "aXXcdY")]
    #[case::preserves_crlf_lines("one\r\ntwo", vec![edit(5..8, "second")], "one\r\nsecond")]
    fn applies_valid_edits(
        #[case] source: &str,
        #[case] edits: Vec<TextEdit>,
        #[case] expected: &str,
    ) {
        assert_eq!(apply_safe_edits(source, &edits), Ok(expected.to_string()));
    }

    #[rstest]
    #[case::out_of_bounds("sample", vec![edit(0..7, "x")], FixError::InvalidRange(crate::markdown_lint::ParsedMarkdownError::OutsideSource))]
    #[case::invalid_utf8_boundary("日本語", vec![edit(1..3, "x")], FixError::InvalidRange(crate::markdown_lint::ParsedMarkdownError::InvalidUtf8Boundary))]
    #[case::overlap("sample", vec![edit(0..3, "x"), edit(2..5, "y")], FixError::OverlappingEdits)]
    #[case::same_point_insertions("sample", vec![edit(0..0, "x"), edit(0..0, "y")], FixError::OverlappingEdits)]
    #[case::line_count("first\nsecond", vec![edit(0..5, "first\nnew")], FixError::LineCountChanged)]
    fn rejects_unsafe_edits(
        #[case] source: &str,
        #[case] edits: Vec<TextEdit>,
        #[case] expected: FixError,
    ) {
        assert_eq!(apply_safe_edits(source, &edits), Err(expected));
    }

    fn edit(range: std::ops::Range<usize>, replacement: &str) -> TextEdit {
        TextEdit {
            range,
            replacement: replacement.into(),
        }
    }
}
