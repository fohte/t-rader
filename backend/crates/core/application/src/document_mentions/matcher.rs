use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use super::{DocumentMention, RegisteredStock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChunkText<'a> {
    pub id: Uuid,
    pub start_offset: i32,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InvalidOffset;

#[derive(Debug, Clone, Copy)]
struct SourceSpan {
    start: usize,
    end: usize,
}

pub(super) fn find_mentions(
    chunks: &[ChunkText<'_>],
    stocks: &[RegisteredStock],
) -> Result<Vec<DocumentMention>, InvalidOffset> {
    let mut mentions = Vec::new();
    for chunk in chunks {
        let (normalized_body, source_spans) = normalize_with_spans(chunk.body);
        for stock in stocks {
            let normalized_name = normalize(&stock.name);
            if normalized_name.is_empty() {
                continue;
            }
            for (start, _) in normalized_body.match_indices(&normalized_name) {
                let first_char = normalized_body[..start].chars().count();
                let matched_chars = normalized_name.chars().count();
                let Some(start_span) = source_spans.get(first_char) else {
                    continue;
                };
                let Some(end_span) = source_spans.get(first_char + matched_chars - 1) else {
                    continue;
                };
                mentions.push(DocumentMention {
                    chunk_id: chunk.id,
                    stock_id: stock.id.clone(),
                    start_offset: add_offset(chunk.start_offset, start_span.start)?,
                    end_offset: add_offset(chunk.start_offset, end_span.end)?,
                });
            }
        }
    }
    mentions.sort_unstable_by(|left, right| {
        (
            left.chunk_id,
            left.start_offset,
            left.end_offset,
            &left.stock_id,
        )
            .cmp(&(
                right.chunk_id,
                right.start_offset,
                right.end_offset,
                &right.stock_id,
            ))
    });
    Ok(mentions)
}

fn normalize(value: &str) -> String {
    value
        .nfkd()
        .flat_map(char::to_lowercase)
        .collect::<String>()
}

fn normalize_with_spans(value: &str) -> (String, Vec<SourceSpan>) {
    let mut normalized = String::new();
    let mut source_spans = Vec::new();
    for (start, character) in value.char_indices() {
        let end = start + character.len_utf8();
        for normalized_character in character.to_string().nfkd().flat_map(char::to_lowercase) {
            normalized.push(normalized_character);
            source_spans.push(SourceSpan { start, end });
        }
    }
    (normalized, source_spans)
}

fn add_offset(base: i32, local_offset: usize) -> Result<i32, InvalidOffset> {
    let local_offset = i32::try_from(local_offset).map_err(|_| InvalidOffset)?;
    base.checked_add(local_offset).ok_or(InvalidOffset)
}
