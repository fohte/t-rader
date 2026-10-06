use std::ops::Range;

#[derive(Debug, Clone, Copy)]
pub struct NoteToken<'a> {
    pub inner: &'a str,
    pub start: usize,
    pub end: usize,
}

pub fn extract_tokens(body: &str) -> Vec<NoteToken<'_>> {
    let mut tokens = Vec::new();
    let mut search_from = 0;

    while let Some(open_offset) = body[search_from..].find("[[") {
        let start = search_from + open_offset;
        let inner_start = start + 2;
        let Some(close_offset) = body[inner_start..].find(']') else {
            break;
        };
        let close = inner_start + close_offset;
        let has_closing_pair = body.as_bytes().get(close + 1) == Some(&b']');

        if close > inner_start && has_closing_pair {
            tokens.push(NoteToken {
                inner: &body[inner_start..close],
                start,
                end: close + 2,
            });
            search_from = close + 2;
        } else {
            search_from = start + 1;
        }
    }

    tokens
}

fn markdown_code_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = markdown_code_block_ranges(body);
    ranges.extend(inline_code_ranges(body, &ranges));
    ranges
}

fn markdown_code_block_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut fence: Option<(usize, u8, usize)> = None;
    let mut offset = 0;

    for line in body.split_inclusive('\n') {
        let content = line_content(line);
        if let Some((start, marker, minimum_length)) = fence {
            if is_closing_fence(content, marker, minimum_length) {
                ranges.push(start..offset + line.len());
                fence = None;
            }
        } else if let Some((marker, length)) = opening_fence(content) {
            fence = Some((offset, marker, length));
        } else if indentation_columns(content) >= 4 && !content.trim().is_empty() {
            ranges.push(offset..offset + line.len());
        }
        offset += line.len();
    }

    if let Some((start, _, _)) = fence {
        ranges.push(start..body.len());
    }
    ranges
}

fn line_content(line: &str) -> &str {
    let without_newline = line.strip_suffix('\n').unwrap_or(line);
    without_newline
        .strip_suffix('\r')
        .unwrap_or(without_newline)
}

fn opening_fence(line: &str) -> Option<(u8, usize)> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    if indentation > 3 {
        return None;
    }
    let content = &line[indentation..];
    let marker = *content.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = content.bytes().take_while(|byte| *byte == marker).count();
    if length < 3 || (marker == b'`' && content[length..].contains('`')) {
        return None;
    }
    Some((marker, length))
}

fn is_closing_fence(line: &str, marker: u8, minimum_length: usize) -> bool {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    if indentation > 3 {
        return false;
    }
    let content = &line[indentation..];
    let length = content.bytes().take_while(|byte| *byte == marker).count();
    length >= minimum_length && content[length..].trim().is_empty()
}

fn indentation_columns(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .fold(0, |columns, character| {
            if character == '\t' {
                (columns / 4 + 1) * 4
            } else {
                columns + 1
            }
        })
}

fn inline_code_ranges(body: &str, code_blocks: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut search_from = 0;

    while let Some(open_offset) = body[search_from..].find('`') {
        let start = search_from + open_offset;
        if let Some(block) = range_containing(code_blocks, start) {
            search_from = block.end;
            continue;
        }

        let delimiter_length = backtick_run_length(body, start);
        let delimiter_end = start + delimiter_length;
        let mut close_search_from = delimiter_end;
        let mut close = None;

        while let Some(close_offset) = body[close_search_from..].find('`') {
            let close_start = close_search_from + close_offset;
            if has_blank_line(body, start, close_start)
                || code_blocks
                    .iter()
                    .any(|block| block.start > start && block.start < close_start)
            {
                break;
            }
            if range_containing(code_blocks, close_start).is_some() {
                break;
            }
            let close_length = backtick_run_length(body, close_start);
            if close_length == delimiter_length {
                close = Some(close_start + close_length);
                break;
            }
            close_search_from = close_start + close_length;
        }

        if let Some(end) = close {
            ranges.push(start..end);
            search_from = end;
        } else {
            search_from = delimiter_end;
        }
    }

    ranges
}

fn has_blank_line(body: &str, start: usize, end: usize) -> bool {
    body[start..end]
        .split_inclusive('\n')
        .any(|line| line_content(line).trim().is_empty())
}

fn backtick_run_length(body: &str, start: usize) -> usize {
    body.as_bytes()[start..]
        .iter()
        .take_while(|byte| **byte == b'`')
        .count()
}

fn range_containing(ranges: &[Range<usize>], position: usize) -> Option<&Range<usize>> {
    ranges
        .iter()
        .find(|range| position >= range.start && position < range.end)
}

pub(crate) fn tokens_outside_code(body: &str) -> Vec<NoteToken<'_>> {
    let code_ranges = markdown_code_ranges(body);
    extract_tokens(body)
        .into_iter()
        .filter(|token| {
            !code_ranges
                .iter()
                .any(|range| token.start >= range.start && token.end <= range.end)
        })
        .collect()
}

pub(crate) fn prose_segments(body: &str) -> Vec<&str> {
    let mut excluded_ranges = markdown_code_ranges(body);
    excluded_ranges.extend(
        extract_tokens(body)
            .into_iter()
            .map(|token| token.start..token.end),
    );
    excluded_ranges.sort_unstable_by_key(|range| range.start);

    let mut segments = Vec::new();
    let mut cursor = 0;
    for range in excluded_ranges {
        let start = cursor.max(range.start);
        if cursor < start {
            segments.push(&body[cursor..start]);
        }
        cursor = cursor.max(range.end);
    }
    if cursor < body.len() {
        segments.push(&body[cursor..]);
    }

    segments
}
