use std::ops::Range;

use core_domain::markdown_lint::{
    ParsedMarkdown, ParsedMarkdownError, TextRun, TextRunKind, ValidatedTokenSpan,
};
use core_domain::note_reference::{markdown_url_ranges, validated_token_ranges};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

enum OpenTag {
    Other,
    BlockQuote,
    TableCell(usize),
    Formatting,
    Link { auto: bool },
    CodeBlock,
}

pub(super) fn parse_markdown(source: &str) -> Result<ParsedMarkdown<'_>, ParsedMarkdownError> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut open_tags = Vec::new();
    let mut runs = Vec::new();
    let mut next_table_cell = 0;

    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                let open_tag = match &tag {
                    Tag::BlockQuote(_) => OpenTag::BlockQuote,
                    Tag::TableCell => {
                        let cell = next_table_cell;
                        next_table_cell += 1;
                        OpenTag::TableCell(cell)
                    }
                    Tag::Emphasis | Tag::Strong | Tag::Strikethrough => OpenTag::Formatting,
                    Tag::Link { link_type, .. } => OpenTag::Link {
                        auto: matches!(link_type, LinkType::Autolink | LinkType::Email),
                    },
                    Tag::CodeBlock(_) => OpenTag::CodeBlock,
                    _ => OpenTag::Other,
                };
                let is_code_block = matches!(open_tag, OpenTag::CodeBlock);
                open_tags.push(open_tag);
                if is_code_block {
                    push_run(
                        &mut runs,
                        make_run(&range, TextRunKind::CodeBlock, &open_tags),
                    );
                }
            }
            Event::End(_) => {
                open_tags.pop();
            }
            Event::Text(_) if !contains_code_block(&open_tags) => {
                push_run(
                    &mut runs,
                    make_run(&range, text_kind(&open_tags), &open_tags),
                );
            }
            Event::Code(_) => {
                push_run(
                    &mut runs,
                    make_run(&range, TextRunKind::InlineCode, &open_tags),
                );
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                push_run(&mut runs, make_run(&range, TextRunKind::Html, &open_tags));
            }
            _ => {}
        }
    }

    ParsedMarkdown::new(
        source,
        runs,
        validated_token_ranges(source)
            .into_iter()
            .map(|range| ValidatedTokenSpan { range })
            .collect(),
        markdown_url_ranges(source),
    )
}

fn text_kind(open_tags: &[OpenTag]) -> TextRunKind {
    match open_tags.iter().rev().find_map(|tag| match tag {
        OpenTag::Link { auto } => Some(*auto),
        _ => None,
    }) {
        Some(true) => TextRunKind::AutoLink,
        Some(false) => TextRunKind::LinkLabel,
        None => TextRunKind::Text,
    }
}

fn contains_code_block(open_tags: &[OpenTag]) -> bool {
    open_tags
        .iter()
        .any(|tag| matches!(tag, OpenTag::CodeBlock))
}

fn make_run(range: &Range<usize>, kind: TextRunKind, open_tags: &[OpenTag]) -> TextRun {
    TextRun {
        range: range.clone(),
        kind,
        quote_depth: open_tags
            .iter()
            .filter(|tag| matches!(tag, OpenTag::BlockQuote))
            .count(),
        table_cell: open_tags.iter().rev().find_map(|tag| match tag {
            OpenTag::TableCell(cell) => Some(*cell),
            _ => None,
        }),
        formatting_depth: open_tags
            .iter()
            .filter(|tag| matches!(tag, OpenTag::Formatting))
            .count(),
    }
}

fn push_run(runs: &mut Vec<TextRun>, next: TextRun) {
    if let Some(previous) = runs.last_mut()
        && previous.range.end == next.range.start
        && previous.kind == next.kind
        && previous.quote_depth == next.quote_depth
        && previous.table_cell == next.table_cell
        && previous.formatting_depth == next.formatting_depth
    {
        previous.range.end = next.range.end;
    } else {
        runs.push(next);
    }
}

#[cfg(test)]
mod tests {
    use core_domain::markdown_lint::TextRunKind;
    use indoc::indoc;
    use rstest::rstest;

    use super::parse_markdown;

    #[test]
    fn keeps_source_ranges_separate_across_escaped_bytes() {
        let source = r"plain \*escaped\* &amp; tail";
        let document = parse_markdown(source).expect("Markdown source ranges are valid");

        assert_eq!(
            document
                .runs()
                .iter()
                .map(|run| (run.kind, &source[run.range.clone()]))
                .collect::<Vec<_>>(),
            vec![
                (TextRunKind::Text, "plain "),
                (TextRunKind::Text, "*escaped"),
                (TextRunKind::Text, "* &amp; tail"),
            ],
        );
    }

    #[test]
    fn tracks_utf8_source_ranges_for_tokens_and_urls() {
        let source = "注記 [[stock:sample-code]] https://example.invalid/path";
        let token = "[[stock:sample-code]]";
        let url = "https://example.invalid/path";
        let token_start = source.find(token).expect("the token is in the source");
        let url_start = source.find(url).expect("the URL is in the source");
        let document = parse_markdown(source).expect("Markdown source ranges are valid");

        assert_eq!(
            (
                document
                    .validated_tokens()
                    .iter()
                    .map(|token| token.range.clone())
                    .collect::<Vec<_>>(),
                document.url_ranges().to_vec(),
            ),
            (
                std::iter::once(token_start..token_start + token.len()).collect(),
                std::iter::once(url_start..url_start + url.len()).collect(),
            ),
        );
    }

    #[rstest]
    #[case::inline_code(
        "before `sample` after",
        TextRunKind::InlineCode,
        vec!["`sample`"]
    )]
    #[case::link_label(
        "[sample](https://example.invalid/path)",
        TextRunKind::LinkLabel,
        vec!["sample"]
    )]
    #[case::auto_link(
        "<https://example.invalid/path>",
        TextRunKind::AutoLink,
        vec!["https://example.invalid/path"]
    )]
    #[case::html(
        "<span>sample</span>",
        TextRunKind::Html,
        vec!["<span>", "</span>"]
    )]
    fn records_markup_runs(
        #[case] source: &str,
        #[case] expected_kind: TextRunKind,
        #[case] expected_sources: Vec<&str>,
    ) {
        let document = parse_markdown(source).expect("Markdown source ranges are valid");

        assert_eq!(
            document
                .runs()
                .iter()
                .filter(|run| run.kind == expected_kind)
                .map(|run| &source[run.range.clone()])
                .collect::<Vec<_>>(),
            expected_sources,
        );
    }

    #[test]
    fn records_code_block_source_range() {
        let source = indoc! {"
            before
            ```text
            sample
            ```
            after
        "};
        let document = parse_markdown(source).expect("Markdown source ranges are valid");

        assert_eq!(
            document
                .runs()
                .iter()
                .filter(|run| run.kind == TextRunKind::CodeBlock)
                .map(|run| &source[run.range.clone()])
                .collect::<Vec<_>>(),
            vec![indoc! {"
                ```text
                sample
                ```"}],
        );
    }

    #[test]
    fn records_gfm_table_and_quote_contexts() {
        let source = indoc! {"
            > | left | right |
            > | --- | --- |
            > | one | two |
            > ~~done~~
        "};
        let document = parse_markdown(source).expect("Markdown source ranges are valid");

        assert_eq!(
            (
                document
                    .runs()
                    .iter()
                    .any(|run| run.table_cell.is_some() && run.quote_depth > 0),
                document.runs().iter().any(|run| run.formatting_depth > 0),
            ),
            (true, true),
        );
    }
}
