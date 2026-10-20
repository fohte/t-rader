use std::ops::Range;

use super::super::{
    MarkdownLintContext, RuleDefinition, RuleFinding, RuleRequirements, Severity, TextRunKind,
};

const UUID_LENGTH: usize = 36;
const MESSAGE: &str =
    "UUID を本文に直接書かず、参照先のタイトル付きリンクを本文に引用してください。";

pub static RULE: RuleDefinition = RuleDefinition::new(
    "no-bare-uuid",
    Severity::Error,
    RuleRequirements::NONE,
    check,
);

fn check(context: &MarkdownLintContext<'_>) -> Vec<RuleFinding> {
    let document = context.document;
    let source = document.source();

    document
        .runs()
        .iter()
        .filter(|run| {
            matches!(
                run.kind,
                TextRunKind::Text | TextRunKind::InlineCode | TextRunKind::LinkLabel
            )
        })
        .flat_map(|run| uuid_ranges(source, &run.range))
        .filter(|range| {
            !document
                .validated_tokens()
                .iter()
                .any(|token| overlaps(range, &token.range))
                && !document.url_ranges().iter().any(|url| overlaps(range, url))
        })
        .map(|range| RuleFinding {
            key: source[range.clone()].to_string(),
            range,
            message: MESSAGE.into(),
            fix: None,
        })
        .collect()
}

fn uuid_ranges(source: &str, range: &Range<usize>) -> Vec<Range<usize>> {
    let bytes = source.as_bytes();
    let mut matches = Vec::new();
    let mut start = range.start;

    while start + UUID_LENGTH <= range.end {
        let end = start + UUID_LENGTH;
        if is_standard_uuid(&bytes[start..end])
            && (start == 0 || !is_identifier_byte(bytes.get(start - 1).copied()))
            && !is_identifier_byte(bytes.get(end).copied())
        {
            matches.push(start..end);
            start = end;
        } else {
            start += 1;
        }
    }

    matches
}

fn is_standard_uuid(bytes: &[u8]) -> bool {
    bytes.len() == UUID_LENGTH
        && bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn is_identifier_byte(byte: Option<u8>) -> bool {
    byte.is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}
