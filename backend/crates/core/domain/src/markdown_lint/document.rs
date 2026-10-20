use std::ops::Range;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRunKind {
    Text,
    InlineCode,
    LinkLabel,
    AutoLink,
    CodeBlock,
    Html,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRun {
    pub range: Range<usize>,
    pub kind: TextRunKind,
    pub quote_depth: usize,
    pub table_cell: Option<usize>,
    pub formatting_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTokenSpan {
    pub range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMarkdown<'a> {
    source: &'a str,
    runs: Vec<TextRun>,
    tokens: Vec<ValidatedTokenSpan>,
    url_ranges: Vec<Range<usize>>,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ParsedMarkdownError {
    #[error("Markdown source range is outside the source")]
    OutsideSource,
    #[error("Markdown source range is not on UTF-8 boundaries")]
    InvalidUtf8Boundary,
}

impl<'a> ParsedMarkdown<'a> {
    pub fn new(
        source: &'a str,
        runs: Vec<TextRun>,
        tokens: Vec<ValidatedTokenSpan>,
        url_ranges: Vec<Range<usize>>,
    ) -> Result<Self, ParsedMarkdownError> {
        for range in runs
            .iter()
            .map(|run| &run.range)
            .chain(tokens.iter().map(|token| &token.range))
            .chain(url_ranges.iter())
        {
            validate_range(source, range)?;
        }

        Ok(Self {
            source,
            runs,
            tokens,
            url_ranges,
        })
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn runs(&self) -> &[TextRun] {
        &self.runs
    }

    pub fn validated_tokens(&self) -> &[ValidatedTokenSpan] {
        &self.tokens
    }

    pub fn url_ranges(&self) -> &[Range<usize>] {
        &self.url_ranges
    }

    pub fn plain_text_run_containing(&self, range: &Range<usize>) -> Option<&TextRun> {
        let run = self.runs.iter().find(|run| {
            run.kind == TextRunKind::Text
                && run.formatting_depth == 0
                && run.range.start <= range.start
                && range.end <= run.range.end
        })?;
        if self
            .url_ranges
            .iter()
            .any(|url| range.start < url.end && url.start < range.end)
        {
            return None;
        }
        Some(run)
    }
}

pub(crate) fn validate_range(
    source: &str,
    range: &Range<usize>,
) -> Result<(), ParsedMarkdownError> {
    if range.start > range.end || range.end > source.len() {
        return Err(ParsedMarkdownError::OutsideSource);
    }
    if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
        return Err(ParsedMarkdownError::InvalidUtf8Boundary);
    }
    Ok(())
}
